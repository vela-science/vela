package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
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
	Type            string           `json:"type"`
	Adapter         string           `json:"adapter,omitempty"`
	Endpoint        string           `json:"endpoint,omitempty"`
	Body            json.RawMessage  `json:"body,omitempty"`
	Payload         *requestPayload  `json:"payload,omitempty"`
	RequestCustody  *requestCustody  `json:"request_custody,omitempty"`
	Raw             string           `json:"raw,omitempty"`
	Name            string           `json:"name,omitempty"`
	CallID          string           `json:"call_id,omitempty"`
	Arguments       json.RawMessage  `json:"arguments,omitempty"`
	ArgumentCustody *argumentCustody `json:"argument_custody,omitempty"`
	Result          json.RawMessage  `json:"result,omitempty"`
	Error           string           `json:"error,omitempty"`
	ProviderCalls   int              `json:"provider_calls,omitempty"`
}

type requestPayload struct {
	Schema                    string `json:"schema"`
	Encoding                  string `json:"encoding"`
	ContentType               string `json:"content_type"`
	Bytes                     int    `json:"bytes"`
	SHA256                    string `json:"sha256"`
	Base64                    string `json:"base64"`
	ProviderSchemaBytes       int    `json:"provider_schema_bytes"`
	ProviderSchemaSHA256      string `json:"provider_schema_sha256"`
	ProviderSchemaBase64      string `json:"provider_schema_base64"`
	ProviderSchemaOccurrences int    `json:"provider_schema_occurrences"`
}

type requestCustody struct {
	Schema                    string `json:"schema"`
	ContentType               string `json:"content_type"`
	Bytes                     int    `json:"bytes"`
	SHA256                    string `json:"sha256"`
	PayloadEncoding           string `json:"payload_encoding"`
	DecodeCount               int    `json:"decode_count"`
	ProviderSchemaBytes       int    `json:"provider_schema_bytes"`
	ProviderSchemaSHA256      string `json:"provider_schema_sha256"`
	ProviderSchemaOccurrences int    `json:"provider_schema_occurrences"`
	EndpointWritePrepared     bool   `json:"endpoint_write_prepared"`
}

type argumentCustody struct {
	Schema             string          `json:"schema"`
	RawField           json.RawMessage `json:"raw_field"`
	RawFieldSHA256     string          `json:"raw_field_sha256"`
	DecodedBytesSHA256 string          `json:"decoded_bytes_sha256"`
	DecodeCount        int             `json:"decode_count"`
}

func digestBytes(raw []byte) string {
	sum := sha256.Sum256(raw)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func decodeCanonicalBase64(encoded string) ([]byte, error) {
	if encoded == "" || strings.TrimSpace(encoded) != encoded {
		return nil, errors.New("lossless payload base64 is empty or padded by whitespace")
	}
	decoded, err := base64.StdEncoding.Strict().DecodeString(encoded)
	if err != nil || base64.StdEncoding.EncodeToString(decoded) != encoded {
		return nil, errors.New("lossless payload base64 is not canonical RFC 4648")
	}
	return decoded, nil
}

func custodyForBody(body, schema []byte) (*requestCustody, error) {
	if len(body) == 0 || len(schema) == 0 || !json.Valid(body) || bytes.Count(body, schema) != 1 {
		return nil, errors.New("endpoint write bytes lost exact schema binding")
	}
	return &requestCustody{
		Schema:                    "vela.lossless-provider-request-custody.v1",
		ContentType:               "application/json",
		Bytes:                     len(body),
		SHA256:                    digestBytes(body),
		PayloadEncoding:           "base64-rfc4648-canonical",
		DecodeCount:               1,
		ProviderSchemaBytes:       len(schema),
		ProviderSchemaSHA256:      digestBytes(schema),
		ProviderSchemaOccurrences: bytes.Count(body, schema),
		EndpointWritePrepared:     true,
	}, nil
}

func decodeRequestPayload(raw json.RawMessage) ([]byte, []byte, *requestCustody, error) {
	object, err := decodeExactJSONObject(raw)
	if err != nil || len(object) != 10 {
		return nil, nil, nil, errors.New("lossless request payload is not closed")
	}
	for _, key := range []string{"schema", "encoding", "content_type", "bytes", "sha256", "base64", "provider_schema_bytes", "provider_schema_sha256", "provider_schema_base64", "provider_schema_occurrences"} {
		if object[key] == nil {
			return nil, nil, nil, errors.New("lossless request payload field absent")
		}
	}
	var payload requestPayload
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&payload); err != nil {
		return nil, nil, nil, errors.New("lossless request payload types invalid")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return nil, nil, nil, errors.New("lossless request payload trailing JSON")
	}
	body, err := decodeCanonicalBase64(payload.Base64)
	if err != nil {
		return nil, nil, nil, err
	}
	schema, err := decodeCanonicalBase64(payload.ProviderSchemaBase64)
	if err != nil {
		return nil, nil, nil, err
	}
	if payload.Schema != "vela.lossless-provider-request-payload.v1" || payload.Encoding != "base64-rfc4648-canonical" || payload.ContentType != "application/json" || payload.Bytes <= 0 || payload.Bytes != len(body) || payload.SHA256 != digestBytes(body) || payload.ProviderSchemaBytes <= 0 || payload.ProviderSchemaBytes != len(schema) || payload.ProviderSchemaSHA256 != digestBytes(schema) || payload.ProviderSchemaOccurrences != 1 || bytes.Count(body, schema) != 1 || !json.Valid(body) || len(bytes.TrimSpace(body)) < 2 || bytes.TrimSpace(body)[0] != '{' {
		return nil, nil, nil, errors.New("lossless request payload binding mismatch")
	}
	receipt, err := custodyForBody(body, schema)
	if err != nil {
		return nil, nil, nil, err
	}
	return body, schema, receipt, nil
}

func decodeProviderRequestFrame(raw []byte) (string, string, []byte, []byte, *requestCustody, error) {
	object, err := decodeExactJSONObject(raw)
	if err != nil || len(object) != 4 || object["type"] == nil || object["adapter"] == nil || object["endpoint"] == nil || object["payload"] == nil || object["body"] != nil {
		return "", "", nil, nil, nil, errors.New("provider request frame is not closed lossless transport")
	}
	var frameType, adapter, requestEndpoint string
	if json.Unmarshal(object["type"], &frameType) != nil || json.Unmarshal(object["adapter"], &adapter) != nil || json.Unmarshal(object["endpoint"], &requestEndpoint) != nil || frameType != "provider_request" {
		return "", "", nil, nil, nil, errors.New("provider request frame identity invalid")
	}
	body, schema, custody, err := decodeRequestPayload(object["payload"])
	return adapter, requestEndpoint, body, schema, custody, err
}

func decodeExactJSONObject(raw []byte) (map[string]json.RawMessage, error) {
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) < 2 || trimmed[0] != '{' || trimmed[len(trimmed)-1] != '}' {
		return nil, errors.New("decoded tool arguments are not a JSON object")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	opening, err := decoder.Token()
	if err != nil || opening != json.Delim('{') {
		return nil, errors.New("decoded tool arguments are not a JSON object")
	}
	object := make(map[string]json.RawMessage)
	for decoder.More() {
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok {
			return nil, errors.New("decoded tool argument key invalid")
		}
		if _, exists := object[key]; exists {
			return nil, errors.New("decoded tool arguments contain duplicate field")
		}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return nil, errors.New("decoded tool argument value invalid")
		}
		object[key] = value
	}
	closing, err := decoder.Token()
	if err != nil || closing != json.Delim('}') {
		return nil, errors.New("decoded tool arguments object is incomplete")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return nil, errors.New("decoded tool arguments contain trailing JSON")
	}
	return object, nil
}

func decodeOpenAIArguments(raw json.RawMessage) (json.RawMessage, *argumentCustody, error) {
	var encoded string
	decoder := json.NewDecoder(bytes.NewReader(raw))
	if err := decoder.Decode(&encoded); err != nil {
		return nil, nil, errors.New("OpenAI function_call.arguments must be exactly one JSON string")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return nil, nil, errors.New("OpenAI function_call.arguments contains trailing JSON")
	}
	decoded := json.RawMessage([]byte(encoded))
	if _, err := decodeExactJSONObject(decoded); err != nil {
		return nil, nil, err
	}
	return decoded, &argumentCustody{
		Schema:             "vela.openai-function-call-arguments-custody.v1",
		RawField:           append(json.RawMessage(nil), raw...),
		RawFieldSHA256:     digestBytes(raw),
		DecodedBytesSHA256: digestBytes(decoded),
		DecodeCount:        1,
	}, nil
}

func validateArgumentCustody(arguments json.RawMessage, custody *argumentCustody) error {
	if custody == nil {
		return errors.New("OpenAI argument custody absent")
	}
	decoded, expected, err := decodeOpenAIArguments(custody.RawField)
	if err != nil || custody.Schema != expected.Schema || custody.DecodeCount != 1 ||
		custody.RawFieldSHA256 != expected.RawFieldSHA256 ||
		custody.DecodedBytesSHA256 != expected.DecodedBytesSHA256 ||
		!bytes.Equal(arguments, decoded) {
		return errors.New("OpenAI raw-to-decoded argument custody binding mismatch")
	}
	return nil
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
	object, err := validateToolArguments(name, arguments)
	if err != nil {
		return nil, err
	}
	switch name {
	case "shell":
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

func validateToolArguments(name string, arguments json.RawMessage) (map[string]json.RawMessage, error) {
	object, err := decodeExactJSONObject(arguments)
	if err != nil {
		return nil, err
	}
	switch name {
	case "shell":
		if len(object) != 2 || object["argv"] == nil || object["cwd"] == nil {
			return nil, errors.New("shell arguments not closed")
		}
		var argv []string
		var cwd string
		if err := json.Unmarshal(object["argv"], &argv); err != nil ||
			json.Unmarshal(object["cwd"], &cwd) != nil || cwd != "/workspace" ||
			len(argv) != 4 || argv[0] != "git" || argv[1] != "--no-optional-locks" || argv[2] != "status" || argv[3] != "--short" {
			return nil, errors.New("shell command is not exact")
		}
	case "read_file":
		if len(object) != 2 || object["operation"] == nil || object["path"] == nil {
			return nil, errors.New("read_file arguments not closed")
		}
		var operation, logical string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			json.Unmarshal(object["path"], &logical) != nil ||
			(operation != "read" && operation != "list" && operation != "stat") ||
			(logical != "/workspace" && !strings.HasPrefix(logical, "/workspace/")) ||
			filepath.Clean(logical) != logical || strings.ContainsRune(logical, '\x00') {
			return nil, errors.New("read_file arguments invalid")
		}
	default:
		return nil, errors.New("tool is not allowed")
	}
	return object, nil
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
			var custody *argumentCustody
			if kind == "tool_use" {
				arguments = block["input"]
				_ = json.Unmarshal(block["id"], &callID)
			} else if providerAdapter == "openai-responses-v1" {
				decodedArguments, decodedCustody, decodeErr := decodeOpenAIArguments(arguments)
				if decodeErr != nil {
					return nil, nil, decodeErr
				}
				arguments, custody = decodedArguments, decodedCustody
			}
			if callID == "" {
				return nil, nil, errors.New("provider tool call id missing")
			}
			if _, err := validateToolArguments(name, arguments); err != nil {
				return nil, nil, err
			}
			tools = append(tools, frame{Type: "tool_request", Name: name, CallID: callID, Arguments: arguments, ArgumentCustody: custody})
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
	requestAdapter, requestEndpoint, body, schema, initialCustody, err := decodeProviderRequestFrame(scanner.Bytes())
	if err != nil {
		return err
	}
	exactEndpoint, _ := endpoint()
	if requestAdapter != providerAdapter || requestEndpoint != exactEndpoint {
		return errors.New("provider request escaped exact endpoint")
	}
	providerCalls := 0
	for turn := 0; turn < 64; turn++ {
		providerCalls++
		writeCustody, err := custodyForBody(body, schema)
		if err != nil {
			return err
		}
		if providerCalls == 1 && *writeCustody != *initialCustody {
			return errors.New("initial endpoint write custody drift")
		}
		if err := encoder.Encode(frame{Type: "endpoint_attempt", ProviderCalls: providerCalls, RequestCustody: writeCustody}); err != nil {
			return err
		}
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
			return encoder.Encode(frame{Type: "terminal", Body: terminal, ProviderCalls: providerCalls})
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
		if providerAdapter == "openai-responses-v1" {
			if err := validateArgumentCustody(execution.Arguments, execution.ArgumentCustody); err != nil || execution.ArgumentCustody.RawFieldSHA256 != tool.ArgumentCustody.RawFieldSHA256 || execution.ArgumentCustody.DecodedBytesSHA256 != tool.ArgumentCustody.DecodedBytesSHA256 {
				return errors.New("participant OpenAI argument custody mismatch")
			}
		} else if execution.ArgumentCustody != nil {
			return errors.New("Anthropic tool behavior cannot carry OpenAI argument custody")
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

func validatePayloadOffline() error {
	bridge := os.NewFile(3, "offline-participant-bridge")
	credentialFile := os.NewFile(4, "offline-dummy-credential")
	if bridge == nil || credentialFile == nil {
		return errors.New("offline validation descriptors absent")
	}
	defer bridge.Close()
	rawCredential, err := io.ReadAll(io.LimitReader(credentialFile, 128))
	_ = credentialFile.Close()
	defer clear(rawCredential)
	if err != nil || !bytes.Equal(rawCredential, []byte("offline-validation-dummy-no-secret\n")) {
		return errors.New("offline validation dummy credential invalid")
	}
	scanner := bufio.NewScanner(bridge)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	if !scanner.Scan() {
		return errors.New("offline lossless provider request frame absent")
	}
	adapter, requestEndpoint, _, _, custody, err := decodeProviderRequestFrame(scanner.Bytes())
	if err != nil {
		return err
	}
	exactEndpoint, _ := endpoint()
	if adapter != providerAdapter || requestEndpoint != exactEndpoint {
		return errors.New("offline provider request endpoint drift")
	}
	return json.NewEncoder(bridge).Encode(frame{Type: "write_preparation", RequestCustody: custody})
}

func main() {
	var err error
	switch {
	case len(os.Args) == 2 && os.Args[1] == "--self-test":
		err = selfTest()
	case len(os.Args) == 2 && os.Args[1] == "--validate-payload":
		err = validatePayloadOffline()
	case len(os.Args) == 3 && os.Args[1] == "--serve":
		err = serve(os.Args[2])
	default:
		err = errors.New("accepted arguments are exactly --self-test, --validate-payload, or --serve CANONICAL_WORKSPACE")
	}
	if err != nil {
		_, _ = fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
