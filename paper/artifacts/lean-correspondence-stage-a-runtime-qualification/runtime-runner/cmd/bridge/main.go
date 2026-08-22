package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

var providerAdapter = "unbound"

type frame struct {
	Type      string          `json:"type"`
	Adapter   string          `json:"adapter,omitempty"`
	Endpoint  string          `json:"endpoint,omitempty"`
	Body      json.RawMessage `json:"body,omitempty"`
	Raw       string          `json:"raw,omitempty"`
	Name      string          `json:"name,omitempty"`
	CallID    string          `json:"call_id,omitempty"`
	Arguments json.RawMessage `json:"arguments,omitempty"`
	Result    json.RawMessage `json:"result,omitempty"`
	Error     string          `json:"error,omitempty"`
}

func endpoint() (string, error) {
	switch providerAdapter {
	case "openai-responses-v1":
		return "https://api.openai.com/v1/responses", nil
	case "anthropic-messages-v1":
		return "https://api.anthropic.com/v1/messages", nil
	default:
		return "", errors.New("unsupported provider adapter")
	}
}

func closedPath(workspace, logical string) (string, os.FileInfo, error) {
	if (logical != "/workspace" && !strings.HasPrefix(logical, "/workspace/")) || strings.ContainsRune(logical, '\x00') {
		return "", nil, errors.New("invalid read path")
	}
	relative := strings.TrimPrefix(logical, "/workspace/")
	if logical == "/workspace" {
		relative = ""
	}
	if relative != "" && filepath.Clean(relative) != relative {
		return "", nil, errors.New("read path escapes workspace")
	}
	current := workspace
	for _, component := range strings.Split(relative, "/") {
		if component == "" {
			continue
		}
		current = filepath.Join(current, component)
		info, err := os.Lstat(current)
		if err != nil || info.Mode()&os.ModeSymlink != 0 {
			return "", nil, errors.New("read path contains missing or symbolic component")
		}
	}
	info, err := os.Lstat(current)
	if err != nil || info.Mode()&os.ModeSymlink != 0 {
		return "", nil, errors.New("read path invalid")
	}
	return current, info, nil
}

func executeTool(workspace, name string, arguments json.RawMessage) (json.RawMessage, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(arguments, &object); err != nil {
		return nil, errors.New("tool arguments invalid")
	}
	switch name {
	case "shell":
		if len(object) != 2 {
			return nil, errors.New("shell arguments not closed")
		}
		var argv []string
		var cwd string
		if err := json.Unmarshal(object["argv"], &argv); err != nil ||
			json.Unmarshal(object["cwd"], &cwd) != nil || cwd != "/workspace" ||
			len(argv) != 4 || argv[0] != "git" || argv[1] != "--no-optional-locks" || argv[2] != "status" || argv[3] != "--short" {
			return nil, errors.New("shell command is not exact")
		}
		command := exec.Command("git", "--no-optional-locks", "status", "--short")
		command.Dir = workspace
		command.Env = []string{"PATH=/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM=1", "GIT_TERMINAL_PROMPT=0"}
		stdout, err := command.Output()
		if err != nil {
			return nil, errors.New("git_status failed")
		}
		return json.Marshal(map[string]any{"stdout": string(stdout), "stderr": "", "exit_code": 0})
	case "read_file":
		if len(object) != 2 {
			return nil, errors.New("read_file arguments not closed")
		}
		var operation string
		var logical string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			json.Unmarshal(object["path"], &logical) != nil {
			return nil, errors.New("read_file path invalid")
		}
		path, info, err := closedPath(workspace, logical)
		if err != nil {
			return nil, err
		}
		switch operation {
		case "read":
			if !info.Mode().IsRegular() || info.Size() > 16*1024*1024 {
				return nil, errors.New("read target is not a bounded regular file")
			}
			raw, err := os.ReadFile(path)
			if err != nil {
				return nil, err
			}
			return json.Marshal(map[string]any{"path": logical, "bytes": len(raw), "content": string(raw)})
		case "list":
			if !info.IsDir() {
				return nil, errors.New("list target is not a directory")
			}
			entries, err := os.ReadDir(path)
			if err != nil {
				return nil, err
			}
			names := make([]string, 0, len(entries))
			for _, entry := range entries {
				entryInfo, err := entry.Info()
				if err != nil || entryInfo.Mode()&os.ModeSymlink != 0 {
					return nil, errors.New("list contains symbolic entry")
				}
				names = append(names, entry.Name())
			}
			return json.Marshal(map[string]any{"path": logical, "entries": names})
		case "stat":
			return json.Marshal(map[string]any{"path": logical, "bytes": info.Size(), "directory": info.IsDir(), "regular": info.Mode().IsRegular()})
		default:
			return nil, errors.New("read_file operation invalid")
		}
	default:
		return nil, errors.New("tool is not allowed")
	}
}

func client() *http.Client {
	transport := &http.Transport{
		Proxy:             nil,
		DialContext:       (&net.Dialer{Timeout: 30 * time.Second, KeepAlive: -1}).DialContext,
		DisableKeepAlives: true,
		ForceAttemptHTTP2: true,
	}
	return &http.Client{
		Transport: transport,
		Timeout:   20 * time.Minute,
		CheckRedirect: func(_ *http.Request, _ []*http.Request) error {
			return errors.New("provider redirects are forbidden")
		},
	}
}

func contact(ctx context.Context, url string, credential []byte, body json.RawMessage) ([]byte, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", "application/json")
	if providerAdapter == "openai-responses-v1" {
		request.Header.Set("Authorization", "Bearer "+string(credential))
	} else {
		request.Header.Set("x-api-key", string(credential))
		request.Header.Set("anthropic-version", "2023-06-01")
	}
	response, err := client().Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	raw, err := io.ReadAll(io.LimitReader(response.Body, 64*1024*1024+1))
	if err != nil || len(raw) > 64*1024*1024 {
		return nil, errors.New("provider response exceeds custody bound")
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, fmt.Errorf("provider returned HTTP status %d", response.StatusCode)
	}
	return raw, nil
}

func parseResponse(raw []byte) (json.RawMessage, []frame, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(raw, &object); err != nil {
		return nil, nil, errors.New("provider response is not JSON")
	}
	var blocks []map[string]json.RawMessage
	key := "output"
	if providerAdapter == "anthropic-messages-v1" {
		key = "content"
	}
	if err := json.Unmarshal(object[key], &blocks); err != nil {
		return nil, nil, errors.New("provider terminal blocks missing")
	}
	var text strings.Builder
	var tools []frame
	for _, block := range blocks {
		var kind string
		_ = json.Unmarshal(block["type"], &kind)
		if kind == "function_call" || kind == "tool_use" {
			var name string
			var callID string
			_ = json.Unmarshal(block["name"], &name)
			_ = json.Unmarshal(block["call_id"], &callID)
			arguments := block["arguments"]
			if kind == "tool_use" {
				arguments = block["input"]
				_ = json.Unmarshal(block["id"], &callID)
			}
			if callID == "" {
				return nil, nil, errors.New("provider tool call id missing")
			}
			tools = append(tools, frame{Type: "tool_request", Name: name, CallID: callID, Arguments: arguments})
		}
		if kind == "text" || kind == "output_text" {
			var value string
			_ = json.Unmarshal(block["text"], &value)
			text.WriteString(value)
		}
		if kind == "message" {
			var content []map[string]json.RawMessage
			_ = json.Unmarshal(block["content"], &content)
			for _, item := range content {
				var value string
				_ = json.Unmarshal(item["text"], &value)
				text.WriteString(value)
			}
		}
	}
	if len(tools) == 0 && text.Len() == 0 {
		return nil, nil, errors.New("provider terminal text missing")
	}
	return json.RawMessage(text.String()), tools, nil
}

func continuation(initial, response, result json.RawMessage, tool frame) (json.RawMessage, error) {
	var body map[string]any
	if err := json.Unmarshal(initial, &body); err != nil {
		return nil, err
	}
	if providerAdapter == "openai-responses-v1" {
		var provider map[string]json.RawMessage
		if err := json.Unmarshal(response, &provider); err != nil {
			return nil, err
		}
		var responseID string
		_ = json.Unmarshal(provider["id"], &responseID)
		if responseID == "" {
			return nil, errors.New("OpenAI response id missing")
		}
		body["previous_response_id"] = responseID
		body["input"] = []any{map[string]any{
			"type": "function_call_output", "call_id": tool.CallID, "output": string(result),
		}}
	} else {
		var messages []any
		if current, ok := body["messages"].([]any); ok {
			messages = current
		}
		var provider map[string]json.RawMessage
		if err := json.Unmarshal(response, &provider); err != nil {
			return nil, err
		}
		var content any
		if err := json.Unmarshal(provider["content"], &content); err != nil {
			return nil, err
		}
		messages = append(messages,
			map[string]any{"role": "assistant", "content": content},
			map[string]any{"role": "user", "content": []any{map[string]any{
				"type": "tool_result", "tool_use_id": tool.CallID, "content": string(result),
			}}},
		)
		body["messages"] = messages
	}
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(body); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func serve(workspace string) error {
	workspace, err := filepath.Abs(workspace)
	if err != nil {
		return err
	}
	info, err := os.Stat(workspace)
	if err != nil || !info.IsDir() {
		return errors.New("workspace is not a canonical directory")
	}
	bridge := os.NewFile(3, "participant-bridge")
	credentialFile := os.NewFile(4, "provider-credential")
	if bridge == nil || credentialFile == nil {
		return errors.New("required inherited descriptor absent")
	}
	defer bridge.Close()
	credential, err := io.ReadAll(io.LimitReader(credentialFile, 32*1024))
	_ = credentialFile.Close()
	if err != nil || len(bytes.TrimSpace(credential)) == 0 {
		return errors.New("credential descriptor empty")
	}
	credential = bytes.TrimSpace(credential)
	defer func() { clear(credential) }()
	scanner := bufio.NewScanner(bridge)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	encoder := json.NewEncoder(bridge)
	if !scanner.Scan() {
		return errors.New("provider request frame absent")
	}
	var requestFrame frame
	if err := json.Unmarshal(scanner.Bytes(), &requestFrame); err != nil {
		return err
	}
	exactEndpoint, _ := endpoint()
	if requestFrame.Type != "provider_request" || requestFrame.Adapter != providerAdapter || requestFrame.Endpoint != exactEndpoint {
		return errors.New("provider request escaped exact endpoint")
	}
	body := requestFrame.Body
	for turn := 0; turn < 64; turn++ {
		raw, err := contact(context.Background(), exactEndpoint, credential, body)
		if err != nil {
			return err
		}
		if err := encoder.Encode(frame{Type: "provider_event", Raw: string(raw)}); err != nil {
			return err
		}
		terminal, tools, err := parseResponse(raw)
		if err != nil {
			return err
		}
		if len(tools) == 0 {
			return encoder.Encode(frame{Type: "terminal", Body: terminal})
		}
		if len(tools) != 1 {
			return errors.New("parallel tool calls are forbidden")
		}
		tool := tools[0]
		if err := encoder.Encode(tool); err != nil || !scanner.Scan() {
			return errors.New("participant tool validation frame absent")
		}
		var execution frame
		if err := json.Unmarshal(scanner.Bytes(), &execution); err != nil || execution.Type != "execute_offline_tool" || execution.Name != tool.Name || execution.CallID != tool.CallID || !bytes.Equal(execution.Arguments, tool.Arguments) {
			return errors.New("participant tool validation mismatch")
		}
		result, err := executeTool(workspace, execution.Name, execution.Arguments)
		if err != nil {
			return err
		}
		if err := encoder.Encode(frame{Type: "tool_result", Name: execution.Name, CallID: execution.CallID, Result: result}); err != nil {
			return err
		}
		body, err = continuation(body, raw, result, tool)
		if err != nil {
			return err
		}
	}
	return errors.New("tool turn limit exceeded")
}

func selfTest() error {
	if _, err := endpoint(); err != nil {
		return err
	}
	if _, err := executeTool("/", "read_file", json.RawMessage(`{"operation":"read","path":"/workspace/../etc/passwd"}`)); err == nil {
		return errors.New("path escape adversary accepted")
	}
	if _, err := executeTool("/", "shell", json.RawMessage(`{}`)); err == nil {
		return errors.New("unrestricted tool adversary accepted")
	}
	return nil
}

func main() {
	var err error
	switch {
	case len(os.Args) == 2 && os.Args[1] == "--self-test":
		err = selfTest()
	case len(os.Args) == 3 && os.Args[1] == "--serve":
		err = serve(os.Args[2])
	default:
		err = errors.New("accepted arguments are exactly --self-test or --serve CANONICAL_WORKSPACE")
	}
	if err != nil {
		_, _ = fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
