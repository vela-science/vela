package main

import (
	"io"
	"os"
	"os/exec"
	"syscall"
)

func main() {
	sockets, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_STREAM, 0)
	if err != nil {
		os.Exit(125)
	}
	runnerSocket := os.NewFile(uintptr(sockets[0]), "runner-bridge")
	relaySocket := os.NewFile(uintptr(sockets[1]), "relay-bridge")
	runner := exec.Command("/opt/vela/runner", "--run")
	runner.ExtraFiles = []*os.File{runnerSocket}
	runner.Stdout = os.Stderr
	runner.Stderr = os.Stderr
	runner.Env = []string{"PATH=/usr/bin:/bin"}
	if err := runner.Start(); err != nil {
		os.Exit(125)
	}
	_ = runnerSocket.Close()
	done := make(chan struct{})
	go func() {
		_, _ = io.Copy(relaySocket, os.Stdin)
		_ = syscall.Shutdown(int(relaySocket.Fd()), syscall.SHUT_WR)
		close(done)
	}()
	_, _ = io.Copy(os.Stdout, relaySocket)
	_ = relaySocket.Close()
	<-done
	if err := runner.Wait(); err != nil {
		os.Exit(1)
	}
}
