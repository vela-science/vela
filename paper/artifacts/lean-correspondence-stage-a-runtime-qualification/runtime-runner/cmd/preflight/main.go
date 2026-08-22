package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"syscall"
)

// preflight gives the exact runner and bridge separate deliberately non-secret
// inherited FD 4 values. Their socketpair on FD 3 exercises the exact lossless
// encode/decode/write-preparation path while the bridge's validation mode has no
// endpoint-contact capability.
func main() {
	sockets, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM, 0)
	if err != nil {
		panic(err)
	}
	runnerSocket := os.NewFile(uintptr(sockets[0]), "runner-bridge")
	bridgeSocket := os.NewFile(uintptr(sockets[1]), "bridge-runner")
	runnerCredential, runnerWriter, err := os.Pipe()
	if err != nil {
		panic(err)
	}
	bridgeCredential, bridgeWriter, err := os.Pipe()
	if err != nil {
		panic(err)
	}
	if _, err := runnerWriter.Write([]byte("offline-validation-dummy-no-secret\n")); err != nil {
		panic(err)
	}
	_ = runnerWriter.Close()
	if _, err := bridgeWriter.Write([]byte("offline-validation-dummy-no-secret\n")); err != nil {
		panic(err)
	}
	_ = bridgeWriter.Close()
	command := exec.Command("/opt/vela/runner", "--validate-input")
	command.ExtraFiles = []*os.File{runnerSocket, runnerCredential}
	command.Stdout = os.Stdout
	var runnerStderr bytes.Buffer
	command.Stderr = &runnerStderr
	bridge := exec.Command("/opt/vela/bridge", "--validate-payload")
	bridge.ExtraFiles = []*os.File{bridgeSocket, bridgeCredential}
	var bridgeStderr bytes.Buffer
	bridge.Stderr = &bridgeStderr
	if err := bridge.Start(); err != nil {
		panic(err)
	}
	_ = bridgeSocket.Close()
	_ = bridgeCredential.Close()
	if err := command.Start(); err != nil {
		_ = bridge.Process.Kill()
		panic(err)
	}
	_ = runnerSocket.Close()
	_ = runnerCredential.Close()
	runnerErr := command.Wait()
	bridgeErr := bridge.Wait()
	if runnerErr != nil || bridgeErr != nil {
		_, _ = os.Stderr.Write(runnerStderr.Bytes())
		_, _ = os.Stderr.Write(bridgeStderr.Bytes())
		fmt.Fprintln(os.Stderr, "offline pre-request validation failed")
		os.Exit(1)
	}
}
