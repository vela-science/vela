package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
)

// preflight gives the exact runner a deliberately non-secret inherited FD 4.
// FD 3 is a closed placeholder: offline validation must never use the bridge.
func main() {
	placeholder, err := os.Open("/dev/null")
	if err != nil {
		panic(err)
	}
	defer placeholder.Close()
	readEnd, writeEnd, err := os.Pipe()
	if err != nil {
		panic(err)
	}
	if _, err := writeEnd.Write([]byte("offline-validation-dummy-no-secret\n")); err != nil {
		panic(err)
	}
	_ = writeEnd.Close()
	command := exec.Command("/opt/vela/runner", "--validate-input")
	command.ExtraFiles = []*os.File{placeholder, readEnd}
	command.Stdout = os.Stdout
	var stderr bytes.Buffer
	command.Stderr = &stderr
	err = command.Run()
	_ = readEnd.Close()
	if err != nil {
		_, _ = os.Stderr.Write(stderr.Bytes())
		fmt.Fprintln(os.Stderr, "offline pre-request validation failed")
		os.Exit(1)
	}
}
