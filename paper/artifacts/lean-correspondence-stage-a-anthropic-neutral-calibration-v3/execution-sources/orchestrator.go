package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"syscall"
)

const (
	bridgeBinary = "/Users/williamblair/Documents/Codex/2026-08-21/realtime-voice-chat-2/work/stage_a_anthropic_v3/anthropic-bridge-darwin"
	relayBinary  = "/Users/williamblair/Documents/Codex/2026-08-21/realtime-voice-chat-2/work/stage_a_anthropic_v3/runner-relay-linux"
	executionDir = "/private/tmp/vela-stage-a-anthropic-neutral-execution-v3"
	image        = "sha256:d314adbd8b3765d9aada03bd5bd87ec77826cd81fdc8c8aab3982dce3165385d"
)

type childResult struct {
	Name     string `json:"name"`
	ExitCode int    `json:"exit_code"`
}

func exclusive(path string) (*os.File, error) {
	return os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
}

func writeExclusive(path string, raw []byte) error {
	file, err := exclusive(path)
	if err != nil {
		return err
	}
	if _, err = file.Write(raw); err != nil {
		_ = file.Close()
		return err
	}
	if err = file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

func exitCode(err error) int {
	if err == nil {
		return 0
	}
	var exit *exec.ExitError
	if errors.As(err, &exit) {
		return exit.ExitCode()
	}
	return 127
}

func main() {
	credential, err := io.ReadAll(io.LimitReader(os.Stdin, 32*1024+1))
	if err != nil || len(credential) == 0 || len(credential) > 32*1024 {
		fmt.Fprintln(os.Stderr, "credential pipe invalid")
		os.Exit(125)
	}
	defer clear(credential)

	sockets, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM, 0)
	if err != nil {
		fmt.Fprintln(os.Stderr, "bridge socketpair failed")
		os.Exit(125)
	}
	bridgeSocket := os.NewFile(uintptr(sockets[0]), "bridge-participant")
	relaySocket := os.NewFile(uintptr(sockets[1]), "participant-bridge")
	credentialReader, credentialWriter, err := os.Pipe()
	if err != nil {
		fmt.Fprintln(os.Stderr, "credential pipe failed")
		os.Exit(125)
	}

	var bridgeStdout, bridgeStderr, dockerStderr bytes.Buffer
	bridge := exec.Command(bridgeBinary, "--serve", executionDir+"/workspace")
	bridge.ExtraFiles = []*os.File{bridgeSocket, credentialReader}
	bridge.Stdout = &bridgeStdout
	bridge.Stderr = &bridgeStderr
	bridge.Env = []string{"PATH=/usr/bin:/bin", "SSL_CERT_FILE=" + executionDir + "/input/ca-certificates.crt"}
	if err := bridge.Start(); err != nil {
		fmt.Fprintln(os.Stderr, "bridge start failed")
		os.Exit(125)
	}
	_ = bridgeSocket.Close()
	_ = credentialReader.Close()

	docker := exec.Command(
		"docker", "run", "--rm", "-i", "--network", "none", "--read-only",
		"--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--user", "65532:65532",
		"--mount", "type=bind,src="+relayBinary+",dst=/runner-relay,readonly",
		"--mount", "type=bind,src="+executionDir+"/input,dst=/input,readonly",
		"--mount", "type=bind,src="+executionDir+"/evidence,dst=/evidence",
		"--mount", "type=bind,src="+executionDir+"/workspace,dst=/workspace,readonly",
		"--entrypoint", "/runner-relay", image,
	)
	docker.Env = []string{"PATH=/usr/bin:/bin"}
	dockerStdin, err := docker.StdinPipe()
	if err != nil {
		_ = bridge.Process.Kill()
		fmt.Fprintln(os.Stderr, "participant stdin pipe failed")
		os.Exit(125)
	}
	dockerStdout, err := docker.StdoutPipe()
	if err != nil {
		_ = bridge.Process.Kill()
		fmt.Fprintln(os.Stderr, "participant stdout pipe failed")
		os.Exit(125)
	}
	docker.Stderr = &dockerStderr
	if err := docker.Start(); err != nil {
		_ = bridge.Process.Kill()
		_ = bridge.Wait()
		fmt.Fprintln(os.Stderr, "participant container start failed")
		os.Exit(125)
	}

	if _, err := credentialWriter.Write(credential); err != nil {
		_ = credentialWriter.Close()
		clear(credential)
		_ = docker.Process.Kill()
		_ = bridge.Process.Kill()
		fmt.Fprintln(os.Stderr, "credential injection failed")
		os.Exit(125)
	}
	clear(credential)
	_ = credentialWriter.Close()

	evidence := executionDir + "/evidence/"
	bridgeFrames, err := exclusive(evidence + "bridge-to-runner.raw.jsonl")
	if err != nil {
		_ = docker.Process.Kill()
		_ = bridge.Process.Kill()
		fmt.Fprintln(os.Stderr, "bridge transcript custody failed")
		os.Exit(125)
	}
	runnerFrames, err := exclusive(evidence + "runner-to-bridge.raw.jsonl")
	if err != nil {
		_ = bridgeFrames.Close()
		_ = docker.Process.Kill()
		_ = bridge.Process.Kill()
		fmt.Fprintln(os.Stderr, "runner transcript custody failed")
		os.Exit(125)
	}

	var relay sync.WaitGroup
	relay.Add(2)
	go func() {
		defer relay.Done()
		_, _ = io.Copy(io.MultiWriter(dockerStdin, bridgeFrames), relaySocket)
		_ = dockerStdin.Close()
		_ = bridgeFrames.Sync()
		_ = bridgeFrames.Close()
	}()
	go func() {
		defer relay.Done()
		_, _ = io.Copy(io.MultiWriter(relaySocket, runnerFrames), dockerStdout)
		_ = syscall.Shutdown(int(relaySocket.Fd()), syscall.SHUT_WR)
		_ = runnerFrames.Sync()
		_ = runnerFrames.Close()
	}()

	dockerErr := docker.Wait()
	_ = relaySocket.Close()
	bridgeErr := bridge.Wait()
	relay.Wait()

	if err := writeExclusive(evidence+"bridge.stdout", bridgeStdout.Bytes()); err != nil {
		fmt.Fprintln(os.Stderr, "bridge stdout custody failed")
		os.Exit(125)
	}
	if err := writeExclusive(evidence+"bridge.stderr", bridgeStderr.Bytes()); err != nil {
		fmt.Fprintln(os.Stderr, "bridge stderr custody failed")
		os.Exit(125)
	}
	if err := writeExclusive(evidence+"container.stderr", dockerStderr.Bytes()); err != nil {
		fmt.Fprintln(os.Stderr, "container stderr custody failed")
		os.Exit(125)
	}

	status := "completed"
	if dockerErr != nil || bridgeErr != nil {
		status = "failed_terminal"
	}
	receipt, _ := json.Marshal(map[string]any{
		"schema":               "vela.stage-a-anthropic-neutral-process-teardown.v2",
		"status":               status,
		"credential_fd_closed": true,
		"credential_retained":  false,
		"bridge_fd_closed":     true,
		"participant_network":  "none",
		"children": []childResult{
			{Name: "participant_runner_container", ExitCode: exitCode(dockerErr)},
			{Name: "anthropic_host_bridge", ExitCode: exitCode(bridgeErr)},
		},
	})
	receipt = append(receipt, '\n')
	if err := writeExclusive(evidence+"process-teardown.json", receipt); err != nil {
		fmt.Fprintln(os.Stderr, "teardown custody failed")
		os.Exit(125)
	}
	if dockerErr != nil || bridgeErr != nil {
		os.Exit(1)
	}
}
