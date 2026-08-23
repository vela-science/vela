package main

import (
	"bufio"
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"unicode/utf8"
)

// providerAdapter is injected by the reproducible build. The participant
// runner has no network client. Its only provider transport is a framed stream
// on inherited descriptor 3 to the separately reviewed host bridge.
var providerAdapter = "unbound"

const runnerVersion = "neutral-runner/1"

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

type argumentCustodyReceipt struct {
	Schema             string          `json:"schema"`
	Adapter            string          `json:"adapter"`
	Name               string          `json:"name"`
	CallID             string          `json:"call_id"`
	RawField           json.RawMessage `json:"raw_field"`
	DecodedObject      json.RawMessage `json:"decoded_object"`
	RawFieldSHA256     string          `json:"raw_field_sha256"`
	DecodedBytesSHA256 string          `json:"decoded_bytes_sha256"`
	DecodeCount        int             `json:"decode_count"`
}

type runInput struct {
	RunID                      string          `json:"run_id"`
	Model                      string          `json:"model"`
	Prompt                     string          `json:"prompt"`
	PacketPath                 string          `json:"packet_path"`
	PacketBytes                int             `json:"packet_bytes"`
	PacketSHA256               string          `json:"packet_sha256"`
	ProviderSchema             json.RawMessage `json:"provider_schema"`
	ProviderSchemaPath         string          `json:"provider_schema_path"`
	ProviderSchemaBytes        int             `json:"provider_schema_bytes"`
	ProviderSchemaSHA256       string          `json:"provider_schema_sha256"`
	MaterializationReceiptPath string          `json:"materialization_receipt_path"`
	OutputDir                  string          `json:"output_dir"`
}

type preparedRun struct {
	input  runInput
	packet json.RawMessage
	body   []byte
	schema []byte
}

type packetCustodyReceipt struct {
	Schema                         string `json:"schema"`
	Path                           string `json:"path"`
	Bytes                          int    `json:"bytes"`
	SHA256                         string `json:"sha256"`
	OpenMode                       string `json:"open_mode"`
	LinkCount                      int    `json:"link_count"`
	CanonicalJSONObject            bool   `json:"canonical_json_object"`
	RecursiveDuplicateKeysRejected bool   `json:"recursive_duplicate_keys_rejected"`
	RecursiveCanonical             bool   `json:"recursive_objects_arrays_primitives_canonical"`
	NumberLexemesPreserved         bool   `json:"number_lexemes_preserved"`
	InlineReconstruction           bool   `json:"inline_reconstruction"`
	Injection                      string `json:"injection"`
	RequestSHA256                  string `json:"request_sha256"`
}

type materializationReceipt struct {
	Schema                   string `json:"schema"`
	SourcePath               string `json:"source_path"`
	SourceRegular            bool   `json:"source_regular"`
	SourceSingleLink         bool   `json:"source_single_link"`
	SourceNoFollow           bool   `json:"source_no_follow"`
	SourcePrePostSameInode   bool   `json:"source_pre_post_same_inode"`
	SourceBytes              int    `json:"source_bytes"`
	SourceSHA256             string `json:"source_sha256"`
	RawInsertedStart         int    `json:"raw_inserted_start"`
	RawInsertedEnd           int    `json:"raw_inserted_end"`
	RawInsertedSHA256        string `json:"raw_inserted_sha256"`
	RunJSONSHA256            string `json:"run_json_sha256"`
	MountedSchemaRoot        string `json:"mounted_schema_root"`
	RequestSchemaSHA256      string `json:"request_schema_sha256"`
	ParseReserializationUsed bool   `json:"parse_reserialization_used"`
}

type stageAResponse struct {
	Schema                       string             `json:"schema"`
	AssignmentID                 string             `json:"assignment_id"`
	RelationValidation           string             `json:"relation_validation"`
	ChangeClassification         string             `json:"change_classification"`
	ImpactClosure                []impactItem       `json:"impact_closure"`
	AuthorityScientificInference authorityInference `json:"authority_scientific_inference"`
	Uncertainty                  []string           `json:"uncertainty"`
}

type impactItem struct {
	ItemID      string   `json:"item_id"`
	Disposition string   `json:"disposition"`
	EvidenceIDs []string `json:"evidence_ids"`
}

type authorityInference struct {
	RepositoryAuthorityEffect string `json:"repository_authority_effect"`
	ScientificStatus          string `json:"scientific_status"`
}

func endpoint(adapter string) (string, error) {
	switch adapter {
	case "openai-responses-v1":
		return "https://api.openai.com/v1/responses", nil
	case "anthropic-messages-v1":
		return "https://api.anthropic.com/v1/messages", nil
	default:
		return "", fmt.Errorf("unsupported adapter %q", adapter)
	}
}

func canonical(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

func requestBody(input runInput, packet json.RawMessage) ([]byte, error) {
	toolSpecs := []map[string]any{
		{
			"name":        "read_file",
			"description": "Read, list, stat, or literal-search exact UTF-8 evidence below the read-only /workspace assignment tree.",
			"input_schema": map[string]any{
				"$schema": "https://json-schema.org/draft/2020-12/schema",
				"type":    "object", "additionalProperties": false,
				"required": []string{"operation", "path", "query"},
				"properties": map[string]any{
					"operation": map[string]any{"enum": []string{"read", "list", "stat", "search"}, "type": "string"},
					"path":      map[string]any{"minLength": 1, "pattern": "^/", "type": "string"},
					"query":     map[string]any{"maxLength": 256, "type": "string"},
				},
			},
		},
	}
	if providerAdapter == "openai-responses-v1" {
		tools := make([]any, 0, len(toolSpecs))
		for _, tool := range toolSpecs {
			tools = append(tools, map[string]any{
				"type": "function", "name": tool["name"],
				"description": tool["description"], "parameters": tool["input_schema"],
				"strict": true,
			})
		}
		return canonicalWithRawSchema(map[string]any{
			"model": input.Model, "background": false, "store": false,
			"parallel_tool_calls": false, "max_output_tokens": 32768,
			"reasoning": map[string]any{"effort": "high"}, "service_tier": "default",
			"input": []any{map[string]any{"role": "user", "content": []any{
				map[string]any{"type": "input_text", "text": input.Prompt},
				map[string]any{"type": "input_text", "text": string(packet)},
			}}},
			"tools": tools,
			"text": map[string]any{"format": map[string]any{
				"type": "json_schema", "name": "stage_a_response",
				"schema": input.ProviderSchema, "strict": true,
			}},
		})
	}
	tools := make([]any, len(toolSpecs))
	for index, tool := range toolSpecs {
		tools[index] = tool
	}
	return canonicalWithRawSchema(map[string]any{
		"model": input.Model, "max_tokens": 32768,
		"service_tier": "standard_only", "thinking": map[string]any{"type": "adaptive"},
		"output_config": map[string]any{
			"effort": "high",
			"format": map[string]any{"type": "json_schema", "schema": input.ProviderSchema},
		},
		"messages": []any{map[string]any{"role": "user", "content": input.Prompt + "\n" + string(packet)}},
		"tools":    tools,
	})
}

const rawSchemaSentinel = "__VELA_EXACT_PROVIDER_SCHEMA_BYTES__"

// canonicalWithRawSchema canonicalizes every request field except the mounted
// provider schema. That value is inserted as the exact reviewed file bytes.
func canonicalWithRawSchema(value map[string]any) ([]byte, error) {
	var replace func(any) any
	replace = func(current any) any {
		switch typed := current.(type) {
		case map[string]any:
			result := make(map[string]any, len(typed))
			for key, child := range typed {
				if raw, ok := child.(json.RawMessage); ok && bytes.Equal(raw, valueSchema(value)) {
					result[key] = rawSchemaSentinel
				} else {
					result[key] = replace(child)
				}
			}
			return result
		case []any:
			result := make([]any, len(typed))
			for index, child := range typed {
				result[index] = replace(child)
			}
			return result
		default:
			return current
		}
	}
	schema := valueSchema(value)
	if len(schema) == 0 {
		return nil, errors.New("provider schema absent from request")
	}
	template, err := canonical(replace(value))
	if err != nil {
		return nil, err
	}
	needle, _ := json.Marshal(rawSchemaSentinel)
	if bytes.Count(template, needle) != 1 {
		return nil, errors.New("provider schema insertion point is not unique")
	}
	return bytes.Replace(template, needle, schema, 1), nil
}

func valueSchema(value map[string]any) json.RawMessage {
	if text, ok := value["text"].(map[string]any); ok {
		if format, ok := text["format"].(map[string]any); ok {
			if raw, ok := format["schema"].(json.RawMessage); ok {
				return raw
			}
		}
	}
	if output, ok := value["output_config"].(map[string]any); ok {
		if format, ok := output["format"].(map[string]any); ok {
			if raw, ok := format["schema"].(json.RawMessage); ok {
				return raw
			}
		}
	}
	return nil
}

func readBoundPacket(path string, expectedBytes int, expectedSHA256 string) (json.RawMessage, error) {
	if path != "/input/packet.json" || expectedBytes <= 0 || expectedBytes > 16*1024*1024 {
		return nil, errors.New("packet path or size binding invalid")
	}
	return readPacketFile(path, expectedBytes, expectedSHA256)
}

func readBoundSchema(path string, expectedBytes int, expectedSHA256 string) ([]byte, error) {
	if path != "/input/provider-schema.json" || expectedBytes <= 0 || expectedBytes > 16*1024*1024 {
		return nil, errors.New("provider schema path or size binding invalid")
	}
	before, err := os.Lstat(path)
	if err != nil || !before.Mode().IsRegular() || before.Mode()&os.ModeSymlink != 0 || before.Sys().(*syscall.Stat_t).Nlink != 1 {
		return nil, errors.New("provider schema must be one regular non-symlink link")
	}
	fd, err := syscall.Open(path, syscall.O_RDONLY|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return nil, errors.New("provider schema no-follow open failed")
	}
	file := os.NewFile(uintptr(fd), "provider-schema")
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !os.SameFile(before, opened) {
		return nil, errors.New("provider schema open binding changed")
	}
	raw, err := io.ReadAll(io.LimitReader(file, int64(expectedBytes)+1))
	if err != nil || len(raw) != expectedBytes || digestBytes(raw) != expectedSHA256 {
		return nil, errors.New("provider schema byte/root binding invalid")
	}
	after, err := os.Lstat(path)
	if err != nil || !os.SameFile(opened, after) || after.Mode()&os.ModeSymlink != 0 || after.Sys().(*syscall.Stat_t).Nlink != 1 {
		return nil, errors.New("provider schema path changed during read")
	}
	var object map[string]json.RawMessage
	decoder := json.NewDecoder(bytes.NewReader(raw))
	if err := decoder.Decode(&object); err != nil || object == nil {
		return nil, errors.New("provider schema is not a JSON object")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return nil, errors.New("provider schema contains trailing JSON")
	}
	return raw, nil
}

func strictRunInput(raw []byte) (runInput, error) {
	var input runInput
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&input); err != nil {
		return input, errors.New("strict run input invalid")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF || input.OutputDir != "/evidence" || input.RunID == "" || input.Model == "" {
		return input, errors.New("strict run input invalid")
	}
	return input, nil
}

func prepare() (*preparedRun, error) {
	inputRaw, err := os.ReadFile("/input/run.json")
	if err != nil {
		return nil, err
	}
	input, err := strictRunInput(inputRaw)
	if err != nil {
		return nil, err
	}
	schema, err := readBoundSchema(input.ProviderSchemaPath, input.ProviderSchemaBytes, input.ProviderSchemaSHA256)
	if err != nil || !bytes.Equal(bytes.TrimRight(schema, " \t\r\n"), input.ProviderSchema) {
		return nil, errors.New("provider schema mount binding invalid")
	}
	if input.MaterializationReceiptPath != "/input/materialization-receipt.json" {
		return nil, errors.New("materialization receipt path invalid")
	}
	receiptRaw, err := os.ReadFile(input.MaterializationReceiptPath)
	if err != nil {
		return nil, err
	}
	var receipt materializationReceipt
	decoder := json.NewDecoder(bytes.NewReader(receiptRaw))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&receipt) != nil {
		return nil, errors.New("materialization receipt invalid")
	}
	var extra any
	if decoder.Decode(&extra) != io.EOF {
		return nil, errors.New("materialization receipt trailing JSON")
	}
	start, end := receipt.RawInsertedStart, receipt.RawInsertedEnd
	if receipt.Schema != "vela.stage-a-run-input-materialization.v1" || receipt.SourcePath != input.ProviderSchemaPath ||
		!receipt.SourceRegular || !receipt.SourceSingleLink || !receipt.SourceNoFollow || !receipt.SourcePrePostSameInode ||
		receipt.SourceBytes != len(schema) || receipt.SourceSHA256 != digestBytes(schema) ||
		receipt.RawInsertedSHA256 != digestBytes(schema) || receipt.MountedSchemaRoot != digestBytes(schema) ||
		receipt.RequestSchemaSHA256 != digestBytes(schema) || receipt.ParseReserializationUsed ||
		receipt.RunJSONSHA256 != digestBytes(inputRaw) || start < 0 || end != start+len(schema) || end > len(inputRaw) ||
		!bytes.Equal(inputRaw[start:end], schema) {
		return nil, errors.New("materialization custody binding invalid")
	}
	// The JSON decoder necessarily excludes trailing syntax whitespace from a
	// RawMessage. Restore the exact bound file bytes only after the explicit
	// inserted byte range above has been checked against the mounted file.
	input.ProviderSchema = json.RawMessage(schema)
	packet, err := readBoundPacket(input.PacketPath, input.PacketBytes, input.PacketSHA256)
	if err != nil {
		return nil, err
	}
	body, err := requestBody(input, packet)
	if err != nil {
		return nil, err
	}
	if bytes.Count(body, schema) != 1 {
		return nil, errors.New("request structured-output schema byte binding invalid")
	}
	return &preparedRun{input: input, packet: packet, body: body, schema: schema}, nil
}

func readPacketFile(path string, expectedBytes int, expectedSHA256 string) (json.RawMessage, error) {
	return readPacketFileWithHook(path, expectedBytes, expectedSHA256, nil)
}

func readPacketFileWithHook(path string, expectedBytes int, expectedSHA256 string, afterOpen func()) (json.RawMessage, error) {
	before, err := os.Lstat(path)
	if err != nil || !before.Mode().IsRegular() || before.Mode()&os.ModeSymlink != 0 || before.Sys().(*syscall.Stat_t).Nlink != 1 {
		return nil, errors.New("packet must be one regular non-symlink link")
	}
	fd, err := syscall.Open(path, syscall.O_RDONLY|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return nil, errors.New("packet no-follow open failed")
	}
	file := os.NewFile(uintptr(fd), "neutral-calibration-packet")
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !os.SameFile(before, opened) {
		return nil, errors.New("packet open binding changed")
	}
	if afterOpen != nil {
		afterOpen()
	}
	raw, err := io.ReadAll(io.LimitReader(file, int64(expectedBytes)+1))
	if err != nil || len(raw) != expectedBytes || digestBytes(raw) != expectedSHA256 {
		return nil, errors.New("packet byte/root binding invalid")
	}
	after, err := os.Lstat(path)
	if err != nil || !os.SameFile(opened, after) || after.Mode()&os.ModeSymlink != 0 || after.Sys().(*syscall.Stat_t).Nlink != 1 {
		return nil, errors.New("packet path changed during read")
	}
	value, err := decodeCanonicalPacketJSONObject(raw)
	if err != nil {
		return nil, errors.New("packet is not one closed JSON object")
	}
	canonicalRaw, err := canonical(value)
	if err != nil || !bytes.Equal(canonicalRaw, raw) {
		return nil, errors.New("packet bytes are not exact canonical JSON")
	}
	return json.RawMessage(raw), nil
}

func decodeCanonicalPacketJSONObject(raw []byte) (map[string]any, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	value, err := decodeCanonicalPacketValue(decoder, 0)
	if err != nil {
		return nil, err
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, errors.New("packet top level is not an object")
	}
	if _, err := decoder.Token(); err != io.EOF {
		return nil, errors.New("packet contains trailing JSON")
	}
	return object, nil
}

func decodeCanonicalPacketValue(decoder *json.Decoder, depth int) (any, error) {
	if depth > 256 {
		return nil, errors.New("packet JSON nesting limit exceeded")
	}
	token, err := decoder.Token()
	if err != nil {
		return nil, errors.New("packet JSON token invalid")
	}
	switch value := token.(type) {
	case json.Delim:
		switch value {
		case '{':
			object := make(map[string]any)
			for decoder.More() {
				keyToken, err := decoder.Token()
				key, ok := keyToken.(string)
				if err != nil || !ok {
					return nil, errors.New("packet object key invalid")
				}
				if _, duplicate := object[key]; duplicate {
					return nil, errors.New("packet object contains duplicate key")
				}
				child, err := decodeCanonicalPacketValue(decoder, depth+1)
				if err != nil {
					return nil, err
				}
				object[key] = child
			}
			closing, err := decoder.Token()
			if err != nil || closing != json.Delim('}') {
				return nil, errors.New("packet object is incomplete")
			}
			return object, nil
		case '[':
			array := make([]any, 0)
			for decoder.More() {
				child, err := decodeCanonicalPacketValue(decoder, depth+1)
				if err != nil {
					return nil, err
				}
				array = append(array, child)
			}
			closing, err := decoder.Token()
			if err != nil || closing != json.Delim(']') {
				return nil, errors.New("packet array is incomplete")
			}
			return array, nil
		default:
			return nil, errors.New("unexpected packet closing delimiter")
		}
	case json.Number:
		lexeme := value.String()
		if !isCanonicalPacketNumber(lexeme) {
			return nil, errors.New("packet number is not canonical decimal JSON")
		}
		return value, nil
	case string, bool, nil:
		return value, nil
	default:
		return nil, errors.New("unsupported packet JSON primitive")
	}
}

func isCanonicalPacketNumber(lexeme string) bool {
	if lexeme == "" {
		return false
	}
	index := 0
	negative := false
	if lexeme[index] == '-' {
		negative = true
		index++
		if index == len(lexeme) {
			return false
		}
	}
	integerStart := index
	if lexeme[index] == '0' {
		index++
	} else {
		if lexeme[index] < '1' || lexeme[index] > '9' {
			return false
		}
		for index < len(lexeme) && lexeme[index] >= '0' && lexeme[index] <= '9' {
			index++
		}
	}
	if index < len(lexeme) && lexeme[index] == '.' {
		index++
		fractionStart := index
		for index < len(lexeme) && lexeme[index] >= '0' && lexeme[index] <= '9' {
			index++
		}
		if index == fractionStart || lexeme[index-1] == '0' {
			return false
		}
	}
	if index != len(lexeme) {
		return false
	}
	return !(negative && lexeme[integerStart:] == "0")
}

func packetInjection(adapter string) (string, error) {
	switch adapter {
	case "openai-responses-v1":
		return "input[0].content[1].text_exact_packet_bytes", nil
	case "anthropic-messages-v1":
		return "messages[0].content_exact_prompt_newline_packet_bytes", nil
	default:
		return "", errors.New("unsupported packet injection adapter")
	}
}

func makePacketCustody(input runInput, packet, body []byte) (packetCustodyReceipt, error) {
	injection, err := packetInjection(providerAdapter)
	if err != nil {
		return packetCustodyReceipt{}, err
	}
	receipt := packetCustodyReceipt{
		Schema: "vela.stage-a-neutral-packet-custody.v1", Path: input.PacketPath,
		Bytes: input.PacketBytes, SHA256: input.PacketSHA256,
		OpenMode: "read_only_no_follow", LinkCount: 1,
		CanonicalJSONObject: true, RecursiveDuplicateKeysRejected: true,
		RecursiveCanonical: true, NumberLexemesPreserved: true,
		InlineReconstruction: false,
		Injection:            injection, RequestSHA256: digestBytes(body),
	}
	if err := validatePacketRequestBinding(input, packet, body, receipt); err != nil {
		return packetCustodyReceipt{}, err
	}
	return receipt, nil
}

func validatePacketRequestBinding(input runInput, packet, body []byte, receipt packetCustodyReceipt) error {
	expectedInjection, err := packetInjection(providerAdapter)
	if err != nil || receipt.Schema != "vela.stage-a-neutral-packet-custody.v1" ||
		receipt.Path != input.PacketPath || receipt.Bytes != len(packet) ||
		receipt.Bytes != input.PacketBytes || receipt.SHA256 != digestBytes(packet) ||
		receipt.SHA256 != input.PacketSHA256 || receipt.OpenMode != "read_only_no_follow" ||
		receipt.LinkCount != 1 || !receipt.CanonicalJSONObject || receipt.InlineReconstruction ||
		!receipt.RecursiveDuplicateKeysRejected || !receipt.RecursiveCanonical || !receipt.NumberLexemesPreserved ||
		receipt.Injection != expectedInjection || receipt.RequestSHA256 != digestBytes(body) {
		return errors.New("packet custody or request root binding invalid")
	}
	var request map[string]any
	if err := json.Unmarshal(body, &request); err != nil {
		return errors.New("packet request is not JSON")
	}
	if providerAdapter == "openai-responses-v1" {
		inputItems, ok := request["input"].([]any)
		if !ok || len(inputItems) != 1 {
			return errors.New("OpenAI packet request input drift")
		}
		message, ok := inputItems[0].(map[string]any)
		if !ok {
			return errors.New("OpenAI packet request message drift")
		}
		content, ok := message["content"].([]any)
		if !ok || len(content) != 2 {
			return errors.New("OpenAI packet request content drift")
		}
		packetPart, ok := content[1].(map[string]any)
		if !ok || packetPart["type"] != "input_text" || packetPart["text"] != string(packet) {
			return errors.New("OpenAI packet bytes not transmitted exactly")
		}
		return nil
	}
	messages, ok := request["messages"].([]any)
	if !ok || len(messages) != 1 {
		return errors.New("Anthropic packet request message drift")
	}
	message, ok := messages[0].(map[string]any)
	if !ok || message["content"] != input.Prompt+"\n"+string(packet) {
		return errors.New("Anthropic packet bytes not transmitted exactly")
	}
	return nil
}

func exactKeys(object map[string]json.RawMessage, expected ...string) bool {
	if len(object) != len(expected) {
		return false
	}
	for _, key := range expected {
		if _, ok := object[key]; !ok {
			return false
		}
	}
	return true
}

func uniqueNonempty(items []string) bool {
	seen := make(map[string]struct{}, len(items))
	for _, item := range items {
		if item == "" {
			return false
		}
		if _, ok := seen[item]; ok {
			return false
		}
		seen[item] = struct{}{}
	}
	return true
}

func validateStageA(raw []byte) error {
	var top map[string]json.RawMessage
	if err := json.Unmarshal(raw, &top); err != nil || !exactKeys(top,
		"schema", "assignment_id", "relation_validation", "change_classification",
		"impact_closure", "authority_scientific_inference", "uncertainty") {
		return errors.New("registered schema top-level closure failed")
	}
	var response stageAResponse
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&response); err != nil {
		return fmt.Errorf("registered schema decode: %w", err)
	}
	if response.Schema != "lean-correspondence.review-response.v1" ||
		!strings.HasPrefix(response.AssignmentID, "lc-") {
		return errors.New("registered schema identity failed")
	}
	if !member(response.RelationValidation, "valid", "invalid", "cannot_determine") ||
		!member(response.ChangeClassification, "semantic_change", "environment_drift", "both", "neither", "unprovable") ||
		!member(response.AuthorityScientificInference.RepositoryAuthorityEffect, "none", "repository_local_decision_evidenced", "unprovable") ||
		!member(response.AuthorityScientificInference.ScientificStatus, "not_established", "bounded_source_claim_only", "unprovable") ||
		!uniqueNonempty(response.Uncertainty) {
		return errors.New("registered schema enum or uncertainty closure failed")
	}
	seenItems := map[string]struct{}{}
	for _, item := range response.ImpactClosure {
		if item.ItemID == "" || !member(item.Disposition, "recheck", "invalidate_relation", "remains_valid", "blocked_unprovable") ||
			len(item.EvidenceIDs) == 0 || !uniqueNonempty(item.EvidenceIDs) {
			return errors.New("registered schema impact item failed")
		}
		keyBytes, _ := canonical(item)
		key := string(keyBytes)
		if _, ok := seenItems[key]; ok {
			return errors.New("registered schema impact_closure uniqueItems failed")
		}
		seenItems[key] = struct{}{}
	}
	return nil
}

func member(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}

func digestBytes(raw []byte) string {
	sum := sha256.Sum256(raw)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func makeRequestPayload(body, schema []byte) (requestPayload, error) {
	if len(body) == 0 || len(schema) == 0 || !json.Valid(body) || bytes.Count(body, schema) != 1 {
		return requestPayload{}, errors.New("lossless request payload inputs invalid")
	}
	return requestPayload{
		Schema:                    "vela.lossless-provider-request-payload.v1",
		Encoding:                  "base64-rfc4648-canonical",
		ContentType:               "application/json",
		Bytes:                     len(body),
		SHA256:                    digestBytes(body),
		Base64:                    base64.StdEncoding.EncodeToString(body),
		ProviderSchemaBytes:       len(schema),
		ProviderSchemaSHA256:      digestBytes(schema),
		ProviderSchemaBase64:      base64.StdEncoding.EncodeToString(schema),
		ProviderSchemaOccurrences: 1,
	}, nil
}

func expectedRequestCustody(body, schema []byte) requestCustody {
	return requestCustody{
		Schema:                    "vela.lossless-provider-request-custody.v1",
		ContentType:               "application/json",
		Bytes:                     len(body),
		SHA256:                    digestBytes(body),
		PayloadEncoding:           "base64-rfc4648-canonical",
		DecodeCount:               1,
		ProviderSchemaBytes:       len(schema),
		ProviderSchemaSHA256:      digestBytes(schema),
		ProviderSchemaOccurrences: 1,
		EndpointWritePrepared:     true,
	}
}

func validateRequestCustody(observed *requestCustody, body, schema []byte) error {
	if observed == nil || *observed != expectedRequestCustody(body, schema) {
		return errors.New("bridge lossless request custody mismatch")
	}
	return nil
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

func validateOpenAIArgumentCustody(arguments json.RawMessage, custody *argumentCustody) error {
	if custody == nil || custody.Schema != "vela.openai-function-call-arguments-custody.v1" || custody.DecodeCount != 1 {
		return errors.New("OpenAI argument custody receipt absent or invalid")
	}
	var decodedString string
	decoder := json.NewDecoder(bytes.NewReader(custody.RawField))
	if err := decoder.Decode(&decodedString); err != nil {
		return errors.New("OpenAI arguments raw field is not exactly one JSON string")
	}
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		return errors.New("OpenAI arguments raw field contains trailing JSON")
	}
	decoded := []byte(decodedString)
	if _, err := decodeExactJSONObject(decoded); err != nil {
		return err
	}
	if !bytes.Equal(arguments, decoded) || custody.RawFieldSHA256 != digestBytes(custody.RawField) || custody.DecodedBytesSHA256 != digestBytes(decoded) {
		return errors.New("OpenAI raw-to-decoded argument custody binding mismatch")
	}
	return nil
}

func validateTool(name string, arguments json.RawMessage) error {
	object, err := decodeExactJSONObject(arguments)
	if err != nil {
		return err
	}
	switch name {
	case "read_file":
		if !exactKeys(object, "operation", "path", "query") {
			return errors.New("read_file arguments are not closed")
		}
		var operation, path, query string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			!member(operation, "read", "list", "stat", "search") ||
			json.Unmarshal(object["path"], &path) != nil ||
			json.Unmarshal(object["query"], &query) != nil ||
			(operation == "search") != (query != "") || len(query) > 256 || !utf8.ValidString(query) ||
			(path != "/workspace" && !strings.HasPrefix(path, "/workspace/")) {
			return errors.New("read_file path invalid")
		}
		clean := filepath.Clean(path)
		if clean != path || strings.ContainsRune(path, '\x00') {
			return errors.New("read_file path escapes workspace")
		}
	default:
		return errors.New("tool is not allowed")
	}
	return nil
}

func selfTest() error {
	if _, err := endpoint(providerAdapter); err != nil {
		return err
	}
	if err := validateTool("read_file", json.RawMessage(`{"operation":"read","path":"/workspace/packet.json","query":""}`)); err != nil {
		return err
	}
	for _, rejected := range []json.RawMessage{json.RawMessage(`{"operation":"read","path":"/workspace/../secret","query":""}`), json.RawMessage(`{"operation":"read","path":"/etc/passwd","query":""}`), json.RawMessage(`{"argv":["cat","/etc/passwd"]}`)} {
		if validateTool("read_file", rejected) == nil {
			return errors.New("path adversary accepted")
		}
	}
	body, err := requestBody(runInput{Model: "held-model", Prompt: "neutral", ProviderSchema: json.RawMessage(`{"type":"object"}`)}, json.RawMessage("{}\n"))
	if err != nil || len(body) == 0 {
		return errors.New("request construction failed")
	}
	return nil
}

func appendCustody(directory, name string, raw []byte) error {
	if directory != "/evidence" {
		return errors.New("output directory must be exact /evidence mount")
	}
	path := filepath.Join(directory, name)
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	if _, err = file.Write(raw); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

func run() error {
	prepared, err := prepare()
	if err != nil {
		return err
	}
	input, packet, body := prepared.input, prepared.packet, prepared.body
	if err := appendCustody(input.OutputDir, "request.raw.json", body); err != nil {
		return err
	}
	packetCustody, err := makePacketCustody(input, packet, body)
	if err != nil {
		return err
	}
	packetReceipt, err := canonical(packetCustody)
	if err != nil {
		return err
	}
	if err := appendCustody(input.OutputDir, "packet-custody.json", packetReceipt); err != nil {
		return err
	}
	bridge := os.NewFile(3, "vela-provider-bridge")
	if bridge == nil {
		return errors.New("inherited bridge descriptor absent")
	}
	defer bridge.Close()
	url, _ := endpoint(providerAdapter)
	payload, err := makeRequestPayload(body, prepared.schema)
	if err != nil {
		return err
	}
	encoder := json.NewEncoder(bridge)
	if err := encoder.Encode(frame{Type: "provider_request", Adapter: providerAdapter, Endpoint: url, Payload: &payload}); err != nil {
		return err
	}
	scanner := bufio.NewScanner(bridge)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	var rawEvents bytes.Buffer
	var argumentCustodyBytes bytes.Buffer
	argumentCustodyCount := 0
	providerCalls := 0
	transportReceiptWritten := false
	var finalResponse []byte
	for scanner.Scan() {
		line := append([]byte(nil), scanner.Bytes()...)
		rawEvents.Write(line)
		rawEvents.WriteByte('\n')
		var event frame
		if err := json.Unmarshal(line, &event); err != nil {
			return errors.New("bridge frame invalid")
		}
		switch event.Type {
		case "endpoint_attempt":
			if event.ProviderCalls != providerCalls+1 {
				return errors.New("provider call receipt count drift")
			}
			if providerCalls == 0 {
				if err := validateRequestCustody(event.RequestCustody, body, prepared.schema); err != nil {
					return err
				}
				receipt, err := canonical(event.RequestCustody)
				if err != nil {
					return err
				}
				if err := appendCustody(input.OutputDir, "request-transport-custody.json", receipt); err != nil {
					return err
				}
				transportReceiptWritten = true
			} else if event.RequestCustody == nil || event.RequestCustody.DecodeCount != 1 || !event.RequestCustody.EndpointWritePrepared {
				return errors.New("continuation request custody absent")
			}
			providerCalls++
		case "provider_event":
			// Raw provider bytes are retained before any normalization.
		case "tool_request":
			if providerAdapter == "openai-responses-v1" {
				if err := validateOpenAIArgumentCustody(event.Arguments, event.ArgumentCustody); err != nil {
					return err
				}
				receipt, err := canonical(argumentCustodyReceipt{
					Schema:  "vela.openai-function-call-arguments-custody-receipt.v1",
					Adapter: providerAdapter, Name: event.Name, CallID: event.CallID,
					RawField:           event.ArgumentCustody.RawField,
					DecodedObject:      event.Arguments,
					RawFieldSHA256:     event.ArgumentCustody.RawFieldSHA256,
					DecodedBytesSHA256: event.ArgumentCustody.DecodedBytesSHA256,
					DecodeCount:        event.ArgumentCustody.DecodeCount,
				})
				if err != nil {
					return err
				}
				argumentCustodyBytes.Write(receipt)
				argumentCustodyCount++
			} else if event.ArgumentCustody != nil {
				return errors.New("Anthropic tool behavior cannot carry OpenAI argument custody")
			}
			if err := validateTool(event.Name, event.Arguments); err != nil {
				return err
			}
			if event.CallID == "" {
				return errors.New("tool call id absent")
			}
			if err := encoder.Encode(frame{Type: "execute_offline_tool", Name: event.Name, CallID: event.CallID, Arguments: event.Arguments, ArgumentCustody: event.ArgumentCustody}); err != nil {
				return err
			}
		case "tool_result":
			if len(event.Result) == 0 {
				return errors.New("empty tool result")
			}
		case "terminal":
			if event.ProviderCalls != providerCalls {
				return errors.New("bridge terminal provider call count drift")
			}
			if event.Error != "" {
				return errors.New(event.Error)
			}
			finalResponse = append([]byte(nil), event.Body...)
		default:
			return errors.New("unknown bridge frame")
		}
		if event.Type == "terminal" {
			break
		}
	}
	if err := scanner.Err(); err != nil {
		return err
	}
	if !transportReceiptWritten {
		return errors.New("initial lossless request custody absent")
	}
	if err := appendCustody(input.OutputDir, "provider-events.raw.jsonl", rawEvents.Bytes()); err != nil {
		return err
	}
	if err := validateStageA(finalResponse); err != nil {
		return err
	}
	if err := appendCustody(input.OutputDir, "response.raw.json", finalResponse); err != nil {
		return err
	}
	sum := sha256.Sum256(finalResponse)
	receipt, _ := canonical(map[string]any{
		"schema": "vela.stage-a-runner-terminal.v1", "status": "completed",
		"adapter": providerAdapter, "run_id": input.RunID,
		"response_sha256":     "sha256:" + hex.EncodeToString(sum[:]),
		"packet_sha256":       input.PacketSHA256,
		"request_sha256":      digestBytes(body),
		"provider_calls":      providerCalls,
		"credential_retained": false,
	})
	if providerAdapter == "openai-responses-v1" {
		if err := appendCustody(input.OutputDir, "tool-arguments-custody.jsonl", argumentCustodyBytes.Bytes()); err != nil {
			return err
		}
		var terminal map[string]any
		if err := json.Unmarshal(receipt, &terminal); err != nil {
			return err
		}
		terminal["tool_arguments_custody_sha256"] = digestBytes(argumentCustodyBytes.Bytes())
		terminal["tool_arguments_custody_count"] = argumentCustodyCount
		receipt, _ = canonical(terminal)
	}
	return appendCustody(input.OutputDir, "terminal.json", receipt)
}

func validateInputOffline() error {
	credential := os.NewFile(4, "offline-validation-dummy-credential")
	if credential == nil {
		return errors.New("offline validation credential descriptor absent")
	}
	raw, err := io.ReadAll(io.LimitReader(credential, 128))
	_ = credential.Close()
	defer clear(raw)
	if err != nil || !bytes.Equal(raw, []byte("offline-validation-dummy-no-secret\n")) {
		return errors.New("offline validation dummy credential invalid")
	}
	prepared, err := prepare()
	if err != nil {
		return err
	}
	if err := appendCustody(prepared.input.OutputDir, "request.raw.json", prepared.body); err != nil {
		return err
	}
	packetCustody, err := makePacketCustody(prepared.input, prepared.packet, prepared.body)
	if err != nil {
		return err
	}
	packetReceipt, err := canonical(packetCustody)
	if err != nil {
		return err
	}
	if err := appendCustody(prepared.input.OutputDir, "packet-custody.json", packetReceipt); err != nil {
		return err
	}
	bridge := os.NewFile(3, "offline-validation-bridge")
	if bridge == nil {
		return errors.New("offline validation bridge descriptor absent")
	}
	defer bridge.Close()
	payload, err := makeRequestPayload(prepared.body, prepared.schema)
	if err != nil {
		return err
	}
	url, _ := endpoint(providerAdapter)
	encoder := json.NewEncoder(bridge)
	if err := encoder.Encode(frame{Type: "provider_request", Adapter: providerAdapter, Endpoint: url, Payload: &payload}); err != nil {
		return err
	}
	scanner := bufio.NewScanner(bridge)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	if !scanner.Scan() {
		return errors.New("offline bridge write-preparation receipt absent")
	}
	var bridgeReceipt frame
	if err := json.Unmarshal(scanner.Bytes(), &bridgeReceipt); err != nil || bridgeReceipt.Type != "write_preparation" {
		return errors.New("offline bridge write-preparation receipt invalid")
	}
	if err := validateRequestCustody(bridgeReceipt.RequestCustody, prepared.body, prepared.schema); err != nil {
		return err
	}
	if scanner.Scan() || scanner.Err() != nil {
		return errors.New("offline bridge emitted unexpected frame")
	}
	transportReceipt, err := canonical(bridgeReceipt.RequestCustody)
	if err != nil {
		return err
	}
	if err := appendCustody(prepared.input.OutputDir, "request-transport-custody.json", transportReceipt); err != nil {
		return err
	}
	receipt, err := canonical(map[string]any{
		"schema":                        "vela.stage-a-offline-pre-request-validation.v1",
		"status":                        "pass",
		"adapter":                       providerAdapter,
		"run_id":                        prepared.input.RunID,
		"run_json_sha256":               digestFile("/input/run.json"),
		"mounted_schema_root":           digestBytes(prepared.schema),
		"request_schema_sha256":         digestBytes(prepared.schema),
		"request_sha256":                digestBytes(prepared.body),
		"request_bytes":                 len(prepared.body),
		"request_payload_encoding":      payload.Encoding,
		"request_payload_sha256":        payload.SHA256,
		"bridge_decoded_request_sha256": bridgeReceipt.RequestCustody.SHA256,
		"bridge_decoded_request_bytes":  bridgeReceipt.RequestCustody.Bytes,
		"bridge_decode_count":           bridgeReceipt.RequestCustody.DecodeCount,
		"provider_schema_occurrences":   bridgeReceipt.RequestCustody.ProviderSchemaOccurrences,
		"endpoint_write_prepared":       bridgeReceipt.RequestCustody.EndpointWritePrepared,
		"participant_validation_path":   "exact_runner_prepare_lossless_frame_bridge_decode_and_write_preparation",
		"dummy_credential_fd":           true,
		"credential_secret":             false,
		"endpoint_contact_forbidden":    true,
		"endpoint_write_receipts":       0,
		"provider_calls":                0,
	})
	if err != nil {
		return err
	}
	return appendCustody(prepared.input.OutputDir, "offline-pre-request-validation.json", receipt)
}

func digestFile(path string) string {
	raw, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return digestBytes(raw)
}

func main() {
	var err error
	switch {
	case len(os.Args) == 2 && os.Args[1] == "--version":
		fmt.Println(runnerVersion)
		return
	case len(os.Args) == 2 && os.Args[1] == "--self-test":
		err = selfTest()
	case len(os.Args) == 2 && os.Args[1] == "--run":
		err = run()
	case len(os.Args) == 2 && os.Args[1] == "--validate-input":
		err = validateInputOffline()
	default:
		err = errors.New("accepted arguments are exactly --version, --self-test, --validate-input, or --run")
	}
	if err != nil {
		_, _ = io.WriteString(os.Stderr, err.Error()+"\n")
		os.Exit(1)
	}
}
