package main

import (
	"bytes"
	"encoding/json"
	"testing"
)

func useAdapter(t *testing.T, adapter string) {
	t.Helper()
	previous := providerAdapter
	providerAdapter = adapter
	t.Cleanup(func() { providerAdapter = previous })
}

func TestExactEndpointsAndClosedClient(t *testing.T) {
	for adapter, expected := range map[string]string{
		"openai-responses-v1":   "https://api.openai.com/v1/responses",
		"anthropic-messages-v1": "https://api.anthropic.com/v1/messages",
	} {
		t.Run(adapter, func(t *testing.T) {
			useAdapter(t, adapter)
			observed, err := endpoint()
			if err != nil || observed != expected {
				t.Fatalf("endpoint drift: %q %v", observed, err)
			}
			if client().CheckRedirect(nil, nil) == nil {
				t.Fatal("redirect accepted")
			}
		})
	}
}

func TestToolBoundaryAdversaries(t *testing.T) {
	for _, item := range []struct {
		name string
		args string
	}{
		{"shell", `{}`},
		{"read_file", `{"operation":"read","path":"/workspace/../etc/passwd"}`},
		{"read_file", `{"operation":"read","path":"/etc/passwd"}`},
		{"shell", `{"argv":["git","status"],"cwd":"/workspace"}`},
	} {
		if _, err := executeTool("/", item.name, json.RawMessage(item.args)); err == nil {
			t.Fatalf("accepted tool adversary: %s %s", item.name, item.args)
		}
	}
}

func TestProviderToolFramesAndContinuations(t *testing.T) {
	useAdapter(t, "openai-responses-v1")
	openAICases := []struct {
		name      string
		tool      string
		arguments string
	}{
		{"shell", "shell", `{"argv":["git","--no-optional-locks","status","--short"],"cwd":"/workspace"}`},
		{"read", "read_file", `{"operation":"read","path":"/workspace/packet.json"}`},
		{"list", "read_file", `{"operation":"list","path":"/workspace"}`},
		{"stat", "read_file", `{"operation":"stat","path":"/workspace/packet.json"}`},
	}
	for _, item := range openAICases {
		t.Run("openai_"+item.name, func(t *testing.T) {
			openai := openAIResponse(t, item.tool, item.arguments)
			_, tools, err := parseResponse(openai)
			if err != nil || len(tools) != 1 || tools[0].CallID != "call-1" || !bytes.Equal(tools[0].Arguments, []byte(item.arguments)) {
				t.Fatalf("OpenAI tool parse failed: %#v %v", tools, err)
			}
			if err := validateArgumentCustody(tools[0].Arguments, tools[0].ArgumentCustody); err != nil {
				t.Fatal(err)
			}
			next, err := continuation(json.RawMessage(`{"model":"held","input":[]}`), openai, json.RawMessage(`{"stdout":""}`), tools[0])
			if err != nil || !json.Valid(next) {
				t.Fatalf("OpenAI continuation failed: %s %v", next, err)
			}
		})
	}

	useAdapter(t, "anthropic-messages-v1")
	anthropic := json.RawMessage(`{"content":[{"type":"tool_use","id":"tool-1","name":"read_file","input":{"operation":"read","path":"/workspace/packet.json"}}]}`)
	_, tools, err := parseResponse(anthropic)
	if err != nil || len(tools) != 1 || tools[0].CallID != "tool-1" {
		t.Fatalf("Anthropic tool parse failed: %#v %v", tools, err)
	}
	next, err := continuation(json.RawMessage(`{"model":"held","messages":[]}`), anthropic, json.RawMessage(`{"content":"x"}`), tools[0])
	if err != nil || !json.Valid(next) {
		t.Fatalf("Anthropic continuation failed: %s %v", next, err)
	}
}

func openAIResponse(t *testing.T, name string, arguments any) json.RawMessage {
	t.Helper()
	raw, err := json.Marshal(map[string]any{
		"id": "resp-1",
		"output": []any{map[string]any{
			"type": "function_call", "call_id": "call-1", "name": name,
			"arguments": arguments,
		}},
	})
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func TestOpenAIArgumentsRejectMalformedNonObjectDoubleEncodedAndUnknown(t *testing.T) {
	useAdapter(t, "openai-responses-v1")
	valid := `{"operation":"read","path":"/workspace/packet.json"}`
	doubleEncoded, err := json.Marshal(valid)
	if err != nil {
		t.Fatal(err)
	}
	for name, arguments := range map[string]any{
		"object_wire_value": map[string]any{"operation": "read", "path": "/workspace/packet.json"},
		"malformed":         `{`,
		"array":             `[]`,
		"null":              `null`,
		"scalar":            `1`,
		"double_encoded":    string(doubleEncoded),
		"unknown_field":     `{"operation":"read","path":"/workspace/packet.json","write":false}`,
		"duplicate_field":   `{"operation":"read","operation":"stat","path":"/workspace/packet.json"}`,
	} {
		t.Run(name, func(t *testing.T) {
			if _, _, err := parseResponse(openAIResponse(t, "read_file", arguments)); err == nil {
				t.Fatalf("accepted OpenAI arguments adversary: %v", arguments)
			}
		})
	}
}

func TestOpenAIRawDecodedCustodyDriftFails(t *testing.T) {
	decoded := json.RawMessage(`{"operation":"read","path":"/workspace/packet.json"}`)
	rawField, err := json.Marshal(string(decoded))
	if err != nil {
		t.Fatal(err)
	}
	_, original, err := decodeOpenAIArguments(rawField)
	if err != nil {
		t.Fatal(err)
	}
	mutations := []struct {
		name      string
		arguments json.RawMessage
		custody   argumentCustody
	}{
		{"decoded", json.RawMessage(`{"operation":"stat","path":"/workspace/packet.json"}`), *original},
		{"raw", decoded, *original},
		{"raw_digest", decoded, *original},
		{"decoded_digest", decoded, *original},
		{"decode_count", decoded, *original},
	}
	mutations[1].custody.RawField = json.RawMessage(`"{\"operation\":\"stat\",\"path\":\"/workspace/packet.json\"}"`)
	mutations[2].custody.RawFieldSHA256 = "sha256:" + string(bytes.Repeat([]byte{'0'}, 64))
	mutations[3].custody.DecodedBytesSHA256 = "sha256:" + string(bytes.Repeat([]byte{'1'}, 64))
	mutations[4].custody.DecodeCount = 2
	for _, mutation := range mutations {
		t.Run(mutation.name, func(t *testing.T) {
			if validateArgumentCustody(mutation.arguments, &mutation.custody) == nil {
				t.Fatal("accepted argument custody drift")
			}
		})
	}
}
