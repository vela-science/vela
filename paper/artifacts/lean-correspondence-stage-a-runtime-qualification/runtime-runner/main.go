package main

import (
	"bufio"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// providerAdapter is injected by the reproducible build. The participant
// runner has no network client. Its only provider transport is a framed stream
// on inherited descriptor 3 to the separately reviewed host bridge.
var providerAdapter = "unbound"

const runnerVersion = "neutral-runner/1"

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

type runInput struct {
	RunID          string          `json:"run_id"`
	Model          string          `json:"model"`
	Prompt         string          `json:"prompt"`
	Packet         json.RawMessage `json:"packet"`
	ProviderSchema json.RawMessage `json:"provider_schema"`
	OutputDir      string          `json:"output_dir"`
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

func requestBody(input runInput) ([]byte, error) {
	toolSpecs := []map[string]any{
		map[string]any{
			"name":        "shell",
			"description": "Return git --no-optional-locks status --short for the read-only workspace.",
			"input_schema": map[string]any{
				"type": "object", "additionalProperties": false,
				"required": []string{"argv", "cwd"},
				"properties": map[string]any{
					"argv": map[string]any{"const": []string{"git", "--no-optional-locks", "status", "--short"}},
					"cwd":  map[string]any{"const": "/workspace"},
				},
			},
		},
		map[string]any{
			"name":        "read_file",
			"description": "Read one regular non-symlink file below the read-only workspace.",
			"input_schema": map[string]any{
				"type": "object", "additionalProperties": false,
				"required": []string{"operation", "path"},
				"properties": map[string]any{
					"operation": map[string]any{"enum": []string{"read", "list", "stat"}},
					"path":      map[string]any{"type": "string", "pattern": "^/workspace(?:/|$)"},
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
		return canonical(map[string]any{
			"model": input.Model, "background": false, "store": false,
			"parallel_tool_calls": false, "max_output_tokens": 32768,
			"reasoning": map[string]any{"effort": "high"}, "service_tier": "default",
			"input": []any{map[string]any{"role": "user", "content": []any{
				map[string]any{"type": "input_text", "text": input.Prompt},
				map[string]any{"type": "input_text", "text": string(input.Packet)},
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
	return canonical(map[string]any{
		"model": input.Model, "max_tokens": 32768,
		"service_tier": "standard_only", "thinking": map[string]any{"type": "adaptive"},
		"output_config": map[string]any{
			"effort": "high",
			"format": map[string]any{"type": "json_schema", "schema": input.ProviderSchema},
		},
		"messages": []any{map[string]any{"role": "user", "content": input.Prompt + "\n" + string(input.Packet)}},
		"tools":    tools,
	})
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

func validateTool(name string, arguments json.RawMessage) error {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(arguments, &object); err != nil {
		return errors.New("tool arguments are not an object")
	}
	switch name {
	case "shell":
		if !exactKeys(object, "argv", "cwd") {
			return errors.New("shell arguments are not closed")
		}
		var argv []string
		var cwd string
		if err := json.Unmarshal(object["argv"], &argv); err != nil ||
			json.Unmarshal(object["cwd"], &cwd) != nil || cwd != "/workspace" ||
			len(argv) != 4 || argv[0] != "git" || argv[1] != "--no-optional-locks" || argv[2] != "status" || argv[3] != "--short" {
			return errors.New("shell command is not the exact read-only git status argv")
		}
	case "read_file":
		if !exactKeys(object, "operation", "path") {
			return errors.New("read_file arguments are not closed")
		}
		var operation string
		var path string
		if err := json.Unmarshal(object["operation"], &operation); err != nil ||
			!member(operation, "read", "list", "stat") ||
			json.Unmarshal(object["path"], &path) != nil ||
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
	if err := validateTool("shell", json.RawMessage(`{"argv":["git","--no-optional-locks","status","--short"],"cwd":"/workspace"}`)); err != nil {
		return err
	}
	if err := validateTool("read_file", json.RawMessage(`{"operation":"read","path":"/workspace/packet.json"}`)); err != nil {
		return err
	}
	for _, rejected := range []json.RawMessage{json.RawMessage(`{"operation":"read","path":"/workspace/../secret"}`), json.RawMessage(`{"operation":"read","path":"/etc/passwd"}`)} {
		if validateTool("read_file", rejected) == nil {
			return errors.New("path adversary accepted")
		}
	}
	body, err := requestBody(runInput{Model: "held-model", Prompt: "neutral", Packet: json.RawMessage(`{}`), ProviderSchema: json.RawMessage(`{"type":"object"}`)})
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
	inputRaw, err := os.ReadFile("/input/run.json")
	if err != nil {
		return err
	}
	var input runInput
	decoder := json.NewDecoder(bytes.NewReader(inputRaw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&input); err != nil || input.OutputDir != "/evidence" || input.RunID == "" || input.Model == "" {
		return errors.New("strict run input invalid")
	}
	providerSchema, err := os.ReadFile("/input/provider-schema.json")
	if err != nil || !bytes.Equal(bytes.TrimSpace(providerSchema), bytes.TrimSpace(input.ProviderSchema)) {
		return errors.New("provider schema mount binding invalid")
	}
	body, err := requestBody(input)
	if err != nil {
		return err
	}
	if err := appendCustody(input.OutputDir, "request.raw.json", body); err != nil {
		return err
	}
	bridge := os.NewFile(3, "vela-provider-bridge")
	if bridge == nil {
		return errors.New("inherited bridge descriptor absent")
	}
	defer bridge.Close()
	url, _ := endpoint(providerAdapter)
	encoder := json.NewEncoder(bridge)
	if err := encoder.Encode(frame{Type: "provider_request", Adapter: providerAdapter, Endpoint: url, Body: body}); err != nil {
		return err
	}
	scanner := bufio.NewScanner(bridge)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	var rawEvents bytes.Buffer
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
		case "provider_event":
			// Raw provider bytes are retained before any normalization.
		case "tool_request":
			if err := validateTool(event.Name, event.Arguments); err != nil {
				return err
			}
			if event.CallID == "" {
				return errors.New("tool call id absent")
			}
			if err := encoder.Encode(frame{Type: "execute_offline_tool", Name: event.Name, CallID: event.CallID, Arguments: event.Arguments}); err != nil {
				return err
			}
		case "tool_result":
			if len(event.Result) == 0 {
				return errors.New("empty tool result")
			}
		case "terminal":
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
		"credential_retained": false,
	})
	return appendCustody(input.OutputDir, "terminal.json", receipt)
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
	default:
		err = errors.New("accepted arguments are exactly --version, --self-test, or --run")
	}
	if err != nil {
		_, _ = io.WriteString(os.Stderr, err.Error()+"\n")
		os.Exit(1)
	}
}
