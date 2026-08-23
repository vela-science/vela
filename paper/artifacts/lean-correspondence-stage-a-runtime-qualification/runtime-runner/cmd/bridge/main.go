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
	"path/filepath"
	"sort"
	"strings"
	"syscall"
	"time"
	"unicode/utf8"
)

var providerAdapter = "unbound"

type frame struct {
	Type            string           `json:"type"`
	Adapter         string           `json:"adapter,omitempty"`
	Endpoint        string           `json:"endpoint,omitempty"`
	Body            json.RawMessage  `json:"body,omitempty"`
	Payload         *requestPayload  `json:"payload,omitempty"`
	Response        *responsePayload `json:"response,omitempty"`
	RequestCustody  *requestCustody  `json:"request_custody,omitempty"`
	Raw             string           `json:"raw,omitempty"`
	Name            string           `json:"name,omitempty"`
	CallID          string           `json:"call_id,omitempty"`
	Arguments       json.RawMessage  `json:"arguments,omitempty"`
	ArgumentCustody *argumentCustody `json:"argument_custody,omitempty"`
	Result          json.RawMessage  `json:"result,omitempty"`
	Error           string           `json:"error,omitempty"`
	ProviderCalls   int              `json:"provider_calls,omitempty"`
	StopReason      string           `json:"stop_reason,omitempty"`
}

type responsePayload struct {
	Schema     string `json:"schema"`
	Encoding   string `json:"encoding"`
	Bytes      int    `json:"bytes"`
	SHA256     string `json:"sha256"`
	Base64     string `json:"base64"`
	HTTPStatus int    `json:"http_status"`
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

const maxToolOutputBytes = 65536

func workspaceRelative(logical string) (string, error) {
	if (logical != "/workspace" && !strings.HasPrefix(logical, "/workspace/")) || strings.ContainsRune(logical, '\x00') {
		return "", errors.New("invalid read path")
	}
	relative := strings.TrimPrefix(logical, "/workspace/")
	if logical == "/workspace" {
		relative = "."
	}
	if filepath.Clean(relative) != relative || filepath.IsAbs(relative) {
		return "", errors.New("read path escapes workspace")
	}
	return relative, nil
}

func linkCount(info os.FileInfo) (uint64, bool) {
	value, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return 0, false
	}
	return uint64(value.Nlink), true
}

func validateComponents(root *os.Root, relative string) (os.FileInfo, error) {
	current := ""
	var target os.FileInfo
	for _, component := range strings.Split(relative, "/") {
		if component == "." {
			component = ""
		}
		if component != "" {
			current = filepath.Join(current, component)
		}
		name := current
		if name == "" {
			name = "."
		}
		info, err := root.Lstat(name)
		if err != nil || info.Mode()&os.ModeSymlink != 0 {
			return nil, errors.New("read path contains missing or symbolic component")
		}
		target = info
	}
	if target == nil {
		return nil, errors.New("read path invalid")
	}
	return target, nil
}

type boundOpener func(*os.Root, string) (*os.File, error)

func openBoundWithOpener(workspace, logical string, opener boundOpener) (*os.File, *os.Root, string, os.FileInfo, error) {
	if !filepath.IsAbs(workspace) || filepath.Clean(workspace) != workspace {
		return nil, nil, "", nil, errors.New("workspace path is not canonical absolute")
	}
	rootInfo, err := os.Lstat(workspace)
	if err != nil || rootInfo.Mode()&os.ModeSymlink != 0 || !rootInfo.IsDir() {
		return nil, nil, "", nil, errors.New("workspace root is unsafe")
	}
	root, err := os.OpenRoot(workspace)
	if err != nil {
		return nil, nil, "", nil, errors.New("workspace descriptor unavailable")
	}
	relative, err := workspaceRelative(logical)
	if err != nil {
		root.Close()
		return nil, nil, "", nil, err
	}
	pre, err := validateComponents(root, relative)
	if err != nil {
		root.Close()
		return nil, nil, "", nil, err
	}
	file, err := opener(root, relative)
	if err != nil {
		root.Close()
		return nil, nil, "", nil, errors.New("descriptor-relative no-follow open failed")
	}
	opened, err := file.Stat()
	if err != nil || !os.SameFile(pre, opened) || opened.Mode()&os.ModeSymlink != 0 {
		file.Close()
		root.Close()
		return nil, nil, "", nil, errors.New("read path changed before descriptor open")
	}
	if opened.Mode().IsRegular() {
		if count, ok := linkCount(opened); !ok || count != 1 {
			file.Close()
			root.Close()
			return nil, nil, "", nil, errors.New("read target hardlink forbidden")
		}
	}
	return file, root, relative, opened, nil
}

func openBound(workspace, logical string) (*os.File, *os.Root, string, os.FileInfo, error) {
	return openBoundWithOpener(workspace, logical, func(root *os.Root, relative string) (*os.File, error) {
		return root.OpenFile(relative, os.O_RDONLY|syscall.O_NOFOLLOW, 0)
	})
}

func closeValidated(file *os.File, root *os.Root, relative string, opened os.FileInfo) error {
	afterFD, statErr := file.Stat()
	afterPath, pathErr := validateComponents(root, relative)
	closeErr := file.Close()
	rootErr := root.Close()
	if statErr != nil || pathErr != nil || closeErr != nil || rootErr != nil ||
		!os.SameFile(opened, afterFD) || !os.SameFile(opened, afterPath) ||
		afterFD.Mode().Type() != opened.Mode().Type() || afterFD.Size() != opened.Size() {
		return errors.New("read target custody drift")
	}
	if opened.Mode().IsRegular() {
		if count, ok := linkCount(afterFD); !ok || count != 1 {
			return errors.New("read target link-count drift")
		}
	}
	return nil
}

func marshalBounded(value any) (json.RawMessage, error) {
	raw, err := json.Marshal(value)
	if err != nil || len(raw) > maxToolOutputBytes {
		return nil, errors.New("tool output exceeds closed byte bound")
	}
	return raw, nil
}

func executeTool(workspace, name string, arguments json.RawMessage) (json.RawMessage, error) {
	object, err := validateToolArguments(name, arguments)
	if err != nil {
		return nil, err
	}
	switch name {
	case "read_file":
		var operation, logical, query string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			json.Unmarshal(object["path"], &logical) != nil ||
			json.Unmarshal(object["query"], &query) != nil {
			return nil, errors.New("read_file path invalid")
		}
		file, root, relative, info, err := openBound(workspace, logical)
		if err != nil {
			return nil, err
		}
		closed := false
		closeFile := func() error {
			if closed {
				return nil
			}
			closed = true
			return closeValidated(file, root, relative, info)
		}
		defer func() { _ = closeFile() }()
		started := time.Now()
		finish := func(value any) (json.RawMessage, error) {
			if time.Since(started) > 30*time.Second {
				return nil, errors.New("tool call timeout exceeded")
			}
			if err := closeFile(); err != nil {
				return nil, err
			}
			return marshalBounded(value)
		}
		switch operation {
		case "read":
			if !info.Mode().IsRegular() || info.Size() > maxToolOutputBytes/2 {
				return nil, errors.New("read target is not a bounded regular file")
			}
			raw, err := io.ReadAll(io.LimitReader(file, maxToolOutputBytes/2+1))
			if err != nil || int64(len(raw)) != info.Size() || !utf8.Valid(raw) {
				return nil, errors.New("read target bytes invalid")
			}
			return finish(map[string]any{"path": logical, "bytes": len(raw), "sha256": digestBytes(raw), "content": string(raw)})
		case "list":
			if !info.IsDir() {
				return nil, errors.New("list target is not a directory")
			}
			entries, err := file.Readdir(-1)
			if err != nil {
				return nil, err
			}
			names := make([]string, 0, len(entries))
			for _, entryInfo := range entries {
				if entryInfo.Mode()&os.ModeSymlink != 0 {
					return nil, errors.New("list contains symbolic entry")
				}
				if entryInfo.Mode().IsRegular() {
					if count, ok := linkCount(entryInfo); !ok || count != 1 {
						return nil, errors.New("list contains hardlinked entry")
					}
				}
				names = append(names, entryInfo.Name())
			}
			sort.Strings(names)
			return finish(map[string]any{"path": logical, "entries": names})
		case "stat":
			count, ok := linkCount(info)
			if !ok {
				return nil, errors.New("stat link count unavailable")
			}
			return finish(map[string]any{"path": logical, "bytes": info.Size(), "directory": info.IsDir(), "regular": info.Mode().IsRegular(), "links": count})
		case "search":
			if !info.Mode().IsRegular() || info.Size() > maxToolOutputBytes/2 || query == "" || len(query) > 256 || !utf8.ValidString(query) {
				return nil, errors.New("search target or query invalid")
			}
			raw, err := io.ReadAll(io.LimitReader(file, maxToolOutputBytes/2+1))
			if err != nil || int64(len(raw)) != info.Size() || !utf8.Valid(raw) {
				return nil, errors.New("search target bytes invalid")
			}
			matches := make([]map[string]any, 0)
			for index, line := range strings.Split(string(raw), "\n") {
				if strings.Contains(line, query) {
					matches = append(matches, map[string]any{"line": index + 1, "text": line})
				}
			}
			return finish(map[string]any{"path": logical, "query": query, "matches": matches})
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
	case "read_file":
		if len(object) != 3 || object["operation"] == nil || object["path"] == nil || object["query"] == nil {
			return nil, errors.New("read_file arguments not closed")
		}
		var operation, logical, query string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			json.Unmarshal(object["path"], &logical) != nil ||
			json.Unmarshal(object["query"], &query) != nil ||
			(operation != "read" && operation != "list" && operation != "stat" && operation != "search") ||
			(operation == "search") != (query != "") || len(query) > 256 || !utf8.ValidString(query) ||
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

func contact(ctx context.Context, url string, credential []byte, body json.RawMessage) ([]byte, int, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, 0, err
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
		return nil, 0, err
	}
	defer response.Body.Close()
	raw, err := io.ReadAll(io.LimitReader(response.Body, 64*1024*1024+1))
	if err != nil || len(raw) > 64*1024*1024 {
		return nil, response.StatusCode, errors.New("provider response exceeds custody bound")
	}
	return raw, response.StatusCode, nil
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
	var anthropicStopReason string
	if providerAdapter == "anthropic-messages-v1" {
		if err := json.Unmarshal(object["stop_reason"], &anthropicStopReason); err != nil {
			return nil, nil, errors.New("Anthropic stop_reason missing")
		}
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
	if len(tools) > 1 {
		return nil, nil, errors.New("parallel tool calls are forbidden")
	}
	if providerAdapter == "anthropic-messages-v1" {
		if len(tools) > 0 && (anthropicStopReason != "tool_use" || text.Len() != 0) {
			return nil, nil, errors.New("Anthropic tool turn lifecycle invalid")
		}
		if len(tools) == 0 && anthropicStopReason != "end_turn" {
			return nil, nil, errors.New("Anthropic terminal lifecycle invalid")
		}
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
	seenToolCallIDs := make(map[string]bool)
	for turn := 0; turn < 64; turn++ {
		providerCalls++
		writeCustody, err := custodyForBody(body, schema)
		if err != nil {
			return err
		}
		if providerCalls == 1 && *writeCustody != *initialCustody {
			return errors.New("initial endpoint write custody drift")
		}
		requestPayload := requestPayload{
			Schema: "vela.lossless-provider-request-payload.v1", Encoding: "base64-rfc4648-canonical",
			ContentType: "application/json", Bytes: len(body), SHA256: digestBytes(body), Base64: base64.StdEncoding.EncodeToString(body),
			ProviderSchemaBytes: len(schema), ProviderSchemaSHA256: digestBytes(schema), ProviderSchemaBase64: base64.StdEncoding.EncodeToString(schema),
			ProviderSchemaOccurrences: bytes.Count(body, schema),
		}
		// Every initial and continuation body crosses the bridge boundary as an
		// explicit lossless payload before its endpoint-attempt receipt. RawMessage
		// is intentionally not used because outer JSON encoding may compact it.
		if err := encoder.Encode(frame{Type: "request_body", Payload: &requestPayload, RequestCustody: writeCustody}); err != nil {
			return err
		}
		if err := encoder.Encode(frame{Type: "endpoint_attempt", ProviderCalls: providerCalls, RequestCustody: writeCustody}); err != nil {
			return err
		}
		raw, httpStatus, err := contact(context.Background(), exactEndpoint, credential, body)
		if err != nil {
			return err
		}
		responsePayload := responsePayload{
			Schema: "vela.lossless-provider-response-payload.v1", Encoding: "base64-rfc4648-canonical",
			Bytes: len(raw), SHA256: digestBytes(raw), Base64: base64.StdEncoding.EncodeToString(raw), HTTPStatus: httpStatus,
		}
		if err := encoder.Encode(frame{Type: "provider_event", Response: &responsePayload}); err != nil {
			return err
		}
		if httpStatus < 200 || httpStatus >= 300 {
			return encoder.Encode(frame{Type: "terminal", Error: fmt.Sprintf("provider returned HTTP status %d", httpStatus), ProviderCalls: providerCalls, StopReason: "http_error"})
		}
		terminal, tools, err := parseResponse(raw)
		if err != nil {
			return err
		}
		if len(tools) == 0 {
			return encoder.Encode(frame{Type: "terminal", Body: terminal, ProviderCalls: providerCalls, StopReason: "end_turn"})
		}
		if len(tools) != 1 {
			return errors.New("parallel tool calls are forbidden")
		}
		tool := tools[0]
		if seenToolCallIDs[tool.CallID] {
			return errors.New("tool call id reused across turns")
		}
		seenToolCallIDs[tool.CallID] = true
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
	if _, err := executeTool("/", "read_file", json.RawMessage(`{"operation":"read","path":"/workspace/../etc/passwd","query":""}`)); err == nil {
		return errors.New("path escape adversary accepted")
	}
	if _, err := executeTool("/", "shell", json.RawMessage(`{}`)); err == nil {
		return errors.New("unrestricted tool adversary accepted")
	}
	return nil
}

type workspaceBinding struct {
	Bytes       int            `json:"bytes"`
	Kind        string         `json:"kind"`
	LogicalPath string         `json:"logical_path"`
	MountedPath string         `json:"mounted_path"`
	SHA256      string         `json:"sha256"`
	Source      map[string]any `json:"source"`
}

type workspaceManifest struct {
	AssignmentID         string             `json:"assignment_id"`
	Bindings             []workspaceBinding `json:"bindings"`
	CaseID               string             `json:"case_id"`
	CellID               string             `json:"cell_id"`
	EvidenceManifestRoot string             `json:"evidence_manifest_root"`
	EvidenceTreeRoot     string             `json:"evidence_tree_root"`
	PacketBytes          int                `json:"packet_bytes"`
	PacketSHA256         string             `json:"packet_sha256"`
	Schema               string             `json:"schema"`
	WorkspaceMount       string             `json:"workspace_mount"`
}

func validateWorkspace(workspace string) error {
	workspace, err := filepath.Abs(workspace)
	if err != nil || filepath.Clean(workspace) != workspace {
		return errors.New("workspace validation path invalid")
	}
	operations := []json.RawMessage{
		json.RawMessage(`{"operation":"list","path":"/workspace","query":""}`),
		json.RawMessage(`{"operation":"stat","path":"/workspace/assignment-manifest.json","query":""}`),
		json.RawMessage(`{"operation":"read","path":"/workspace/assignment-manifest.json","query":""}`),
		json.RawMessage(`{"operation":"search","path":"/workspace/assignment-manifest.json","query":"vela.lean-correspondence-assignment-evidence-manifest.v1"}`),
	}
	var manifestRaw []byte
	for index, arguments := range operations {
		result, err := executeTool(workspace, "read_file", arguments)
		if err != nil {
			return fmt.Errorf("workspace bridge operation %d failed: %w", index, err)
		}
		if index == 2 {
			var receipt struct {
				Bytes   int    `json:"bytes"`
				Content string `json:"content"`
				Path    string `json:"path"`
				SHA256  string `json:"sha256"`
			}
			if json.Unmarshal(result, &receipt) != nil || receipt.Path != "/workspace/assignment-manifest.json" || receipt.Bytes != len([]byte(receipt.Content)) || receipt.SHA256 != digestBytes([]byte(receipt.Content)) {
				return errors.New("workspace manifest bridge receipt invalid")
			}
			manifestRaw = []byte(receipt.Content)
		}
	}
	object, err := decodeExactJSONObject(manifestRaw)
	if err != nil || len(object) != 10 {
		return errors.New("workspace manifest is not closed")
	}
	var manifest workspaceManifest
	decoder := json.NewDecoder(bytes.NewReader(manifestRaw))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&manifest) != nil || manifest.Schema != "vela.lean-correspondence-assignment-evidence-manifest.v1" || manifest.WorkspaceMount != "/workspace" || manifest.AssignmentID == "" || manifest.CellID == "" || manifest.CaseID == "" || len(manifest.Bindings) == 0 {
		return errors.New("workspace manifest contract invalid")
	}
	seen := make(map[string]bool)
	for _, binding := range manifest.Bindings {
		if binding.LogicalPath == "" || seen[binding.LogicalPath] || binding.MountedPath != "/workspace/"+binding.LogicalPath || binding.Bytes < 0 || binding.SHA256 == "" {
			return errors.New("workspace binding identity invalid")
		}
		seen[binding.LogicalPath] = true
		arguments, _ := json.Marshal(map[string]any{"operation": "read", "path": binding.MountedPath, "query": ""})
		result, err := executeTool(workspace, "read_file", arguments)
		if err != nil {
			return fmt.Errorf("workspace evidence unreachable: %s: %w", binding.LogicalPath, err)
		}
		var receipt struct {
			Bytes  int    `json:"bytes"`
			Path   string `json:"path"`
			SHA256 string `json:"sha256"`
		}
		if json.Unmarshal(result, &receipt) != nil || receipt.Path != binding.MountedPath || receipt.Bytes != binding.Bytes || receipt.SHA256 != binding.SHA256 {
			return errors.New("workspace evidence bridge binding drift")
		}
	}
	return json.NewEncoder(os.Stdout).Encode(map[string]any{
		"schema": "vela.anthropic-offline-workspace-bridge-preflight.v1",
		"status": "pass", "workspace_manifest_sha256": digestBytes(manifestRaw),
		"evidence_manifest_root": manifest.EvidenceManifestRoot,
		"evidence_tree_root":     manifest.EvidenceTreeRoot,
		"reachable_file_count":   len(manifest.Bindings),
		"operations":             []string{"read", "list", "stat", "search"},
		"network_contact":        false, "writes": false,
	})
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
	case len(os.Args) == 3 && os.Args[1] == "--validate-workspace":
		err = validateWorkspace(os.Args[2])
	case len(os.Args) == 3 && os.Args[1] == "--serve":
		err = serve(os.Args[2])
	default:
		err = errors.New("accepted arguments are exactly --self-test, --validate-payload, --validate-workspace CANONICAL_WORKSPACE, or --serve CANONICAL_WORKSPACE")
	}
	if err != nil {
		_, _ = fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
