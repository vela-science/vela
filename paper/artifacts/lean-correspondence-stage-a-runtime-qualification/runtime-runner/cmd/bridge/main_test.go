package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"testing"
)

func validLosslessFrame(t *testing.T) ([]byte, []byte, []byte) {
	t.Helper()
	schema := []byte("{\n  \"type\": \"object\"\n}\n")
	body := append([]byte("{\"schema\":"), schema...)
	body = append(body, []byte(",\"value\":1}\n")...)
	payload := requestPayload{
		Schema: "vela.lossless-provider-request-payload.v1", Encoding: "base64-rfc4648-canonical", ContentType: "application/json",
		Bytes: len(body), SHA256: digestBytes(body), Base64: base64.StdEncoding.EncodeToString(body),
		ProviderSchemaBytes: len(schema), ProviderSchemaSHA256: digestBytes(schema), ProviderSchemaBase64: base64.StdEncoding.EncodeToString(schema), ProviderSchemaOccurrences: 1,
	}
	frame, err := json.Marshal(map[string]any{"type": "provider_request", "adapter": "anthropic-messages-v1", "endpoint": "https://api.anthropic.com/v1/messages", "payload": payload})
	if err != nil {
		t.Fatal(err)
	}
	return frame, body, schema
}

func TestLosslessProviderRequestDecodeAndAdversaries(t *testing.T) {
	frame, body, schema := validLosslessFrame(t)
	adapter, endpointValue, decoded, decodedSchema, custody, err := decodeProviderRequestFrame(frame)
	if err != nil || adapter != "anthropic-messages-v1" || endpointValue != "https://api.anthropic.com/v1/messages" || !bytes.Equal(decoded, body) || !bytes.Equal(decodedSchema, schema) || custody.DecodeCount != 1 || !custody.EndpointWritePrepared {
		t.Fatalf("valid lossless frame failed: %v", err)
	}
	mutations := map[string]func(map[string]any){
		"noncanonical_base64": func(item map[string]any) { item["base64"] = item["base64"].(string) + "\n" },
		"double_encoded": func(item map[string]any) {
			item["base64"] = base64.StdEncoding.EncodeToString([]byte(item["base64"].(string)))
		},
		"padding_drift":     func(item map[string]any) { item["base64"] = item["base64"].(string) + "=" },
		"length":            func(item map[string]any) { item["bytes"] = item["bytes"].(float64) + 1 },
		"root":              func(item map[string]any) { item["sha256"] = digestBytes([]byte("drift")) },
		"schema_occurrence": func(item map[string]any) { item["provider_schema_occurrences"] = 0 },
		"boolean_length":    func(item map[string]any) { item["bytes"] = false },
		"unknown":           func(item map[string]any) { item["raw_body"] = map[string]any{} },
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			var candidate map[string]any
			if err := json.Unmarshal(frame, &candidate); err != nil {
				t.Fatal(err)
			}
			mutate(candidate["payload"].(map[string]any))
			raw, _ := json.Marshal(candidate)
			if _, _, _, _, _, err := decodeProviderRequestFrame(raw); err == nil {
				t.Fatal("accepted lossless payload adversary")
			}
		})
	}
	var fallback map[string]any
	_ = json.Unmarshal(frame, &fallback)
	delete(fallback, "payload")
	fallback["body"] = json.RawMessage(body)
	rawFallback, _ := json.Marshal(fallback)
	if _, _, _, _, _, err := decodeProviderRequestFrame(rawFallback); err == nil {
		t.Fatal("accepted RawMessage semantic-only fallback")
	}
}

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
