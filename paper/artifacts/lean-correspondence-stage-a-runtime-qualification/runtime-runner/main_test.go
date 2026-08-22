package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func withAdapter(t *testing.T, adapter string) {
	t.Helper()
	previous := providerAdapter
	providerAdapter = adapter
	t.Cleanup(func() { providerAdapter = previous })
}

func TestPacketFileBindingRejectsRepresentationPathAndFilesystemAdversaries(t *testing.T) {
	directory := t.TempDir()
	valid := []byte("{\"a\":1,\"b\":2}\n")
	path := filepath.Join(directory, "packet.json")
	if err := os.WriteFile(path, valid, 0o600); err != nil {
		t.Fatal(err)
	}
	if observed, err := readPacketFile(path, len(valid), digestBytes(valid)); err != nil || !bytes.Equal(observed, valid) {
		t.Fatalf("valid canonical packet failed: %s %v", observed, err)
	}
	for name, raw := range map[string][]byte{
		"plaintext":            []byte("neutral packet\n"),
		"json_string":          []byte("\"neutral packet\"\n"),
		"whitespace":           []byte("{ \"a\":1,\"b\":2}\n"),
		"key_order":            []byte("{\"b\":2,\"a\":1}\n"),
		"nested_key_order":     []byte("{\"a\":{\"y\":2,\"x\":1}}\n"),
		"nested_duplicate":     []byte("{\"a\":{\"x\":1,\"x\":2}}\n"),
		"deep_key_order":       []byte("{\"a\":[{\"b\":[{\"y\":2,\"x\":1}]}]}\n"),
		"deep_duplicate":       []byte("{\"a\":[{\"b\":[{\"x\":1,\"x\":2}]}]}\n"),
		"number_fraction_zero": []byte("{\"a\":[{\"b\":1.0}]}\n"),
		"number_exponent":      []byte("{\"a\":[{\"b\":1e0}]}\n"),
		"number_negative_zero": []byte("{\"a\":[{\"b\":-0}]}\n"),
		"escaped_string":       []byte("{\"a\":[\"\\u0061\"]}\n"),
	} {
		t.Run(name, func(t *testing.T) {
			candidate := filepath.Join(directory, name+".json")
			if err := os.WriteFile(candidate, raw, 0o600); err != nil {
				t.Fatal(err)
			}
			if _, err := readPacketFile(candidate, len(raw), digestBytes(raw)); err == nil {
				t.Fatal("accepted noncanonical packet representation")
			}
		})
	}
	for name, inner := range map[string]string{
		"deeply_nested_order":     `{"y":2,"x":1}`,
		"deeply_nested_duplicate": `{"x":1,"x":2}`,
	} {
		for range 32 {
			inner = `{"a":[` + inner + `]}`
		}
		raw := []byte(inner + "\n")
		candidate := filepath.Join(directory, name+".json")
		if err := os.WriteFile(candidate, raw, 0o600); err != nil {
			t.Fatal(err)
		}
		if _, err := readPacketFile(candidate, len(raw), digestBytes(raw)); err == nil {
			t.Fatal("accepted deeply nested noncanonical packet")
		}
	}
	canonicalNested := []byte("{\"a\":[{\"b\":1.25,\"c\":[true,false,null,\"x\"]}]}\n")
	canonicalPath := filepath.Join(directory, "canonical-nested.json")
	if err := os.WriteFile(canonicalPath, canonicalNested, 0o600); err != nil {
		t.Fatal(err)
	}
	if observed, err := readPacketFile(canonicalPath, len(canonicalNested), digestBytes(canonicalNested)); err != nil || !bytes.Equal(observed, canonicalNested) {
		t.Fatalf("valid recursively canonical packet failed: %s %v", observed, err)
	}
	frozen, err := os.ReadFile(filepath.Join("..", "neutral-calibration", "packet.json"))
	if err != nil {
		t.Fatal(err)
	}
	frozenPath := filepath.Join(directory, "frozen-packet.json")
	if err := os.WriteFile(frozenPath, frozen, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := readPacketFile(frozenPath, len(frozen), digestBytes(frozen)); err != nil {
		t.Fatalf("committed canonical packet failed: %v", err)
	}
	if _, err := readPacketFile(path, len(valid), digestBytes([]byte("drift"))); err == nil {
		t.Fatal("accepted packet root drift")
	}
	if _, err := readBoundPacket(path, len(valid), digestBytes(valid)); err == nil {
		t.Fatal("accepted wrong logical packet path")
	}
	symlink := filepath.Join(directory, "symlink.json")
	if err := os.Symlink(path, symlink); err != nil {
		t.Fatal(err)
	}
	if _, err := readPacketFile(symlink, len(valid), digestBytes(valid)); err == nil {
		t.Fatal("accepted symlink packet")
	}
	hardlink := filepath.Join(directory, "hardlink.json")
	if err := os.Link(path, hardlink); err != nil {
		t.Fatal(err)
	}
	if _, err := readPacketFile(path, len(valid), digestBytes(valid)); err == nil {
		t.Fatal("accepted multiply linked packet")
	}
}

func TestPacketFileBindingRejectsPathReplacementRace(t *testing.T) {
	directory := t.TempDir()
	valid := []byte("{\"a\":1}\n")
	path := filepath.Join(directory, "packet.json")
	replacement := filepath.Join(directory, "replacement.json")
	if err := os.WriteFile(path, valid, 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(replacement, valid, 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := readPacketFileWithHook(path, len(valid), digestBytes(valid), func() {
		if renameErr := os.Rename(replacement, path); renameErr != nil {
			t.Fatal(renameErr)
		}
	})
	if err == nil {
		t.Fatal("accepted packet path replacement during read")
	}
}

func TestRunInputRejectsInlinePacketReconstruction(t *testing.T) {
	raw := []byte(`{"run_id":"neutral","model":"held","prompt":"p","packet":{},"packet_path":"/input/packet.json","packet_bytes":3,"packet_sha256":"sha256:drift","provider_schema":{},"output_dir":"/evidence"}`)
	var input runInput
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&input) == nil {
		t.Fatal("accepted inline reconstructed packet")
	}
}

func TestPacketRequestCustodyRejectsRawAndReceiptDrift(t *testing.T) {
	packet := []byte("{\"a\":1}\n")
	for _, adapter := range []string{"openai-responses-v1", "anthropic-messages-v1"} {
		t.Run(adapter, func(t *testing.T) {
			withAdapter(t, adapter)
			input := runInput{
				Model: "held-model", Prompt: "neutral prompt", PacketPath: "/input/packet.json",
				PacketBytes: len(packet), PacketSHA256: digestBytes(packet),
				ProviderSchema: json.RawMessage(`{"type":"object"}`),
			}
			body, err := requestBody(input, packet)
			if err != nil {
				t.Fatal(err)
			}
			receipt, err := makePacketCustody(input, packet, body)
			if err != nil {
				t.Fatal(err)
			}
			mutatedBody := bytes.Replace(body, []byte("held-model"), []byte("other-model"), 1)
			if validatePacketRequestBinding(input, packet, mutatedBody, receipt) == nil {
				t.Fatal("accepted request byte drift")
			}
			mutatedPacket := []byte("{\"a\":2}\n")
			if validatePacketRequestBinding(input, mutatedPacket, body, receipt) == nil {
				t.Fatal("accepted packet byte drift")
			}
			mutatedReceipt := receipt
			mutatedReceipt.Injection = "inline_reconstruction"
			if validatePacketRequestBinding(input, packet, body, mutatedReceipt) == nil {
				t.Fatal("accepted injection custody drift")
			}
		})
	}
}

func TestOpenAIArgumentCustodyBinding(t *testing.T) {
	decoded := json.RawMessage(`{"operation":"list","path":"/workspace"}`)
	rawField, err := json.Marshal(string(decoded))
	if err != nil {
		t.Fatal(err)
	}
	custody := argumentCustody{
		Schema:   "vela.openai-function-call-arguments-custody.v1",
		RawField: rawField, RawFieldSHA256: digestBytes(rawField),
		DecodedBytesSHA256: digestBytes(decoded), DecodeCount: 1,
	}
	if err := validateOpenAIArgumentCustody(decoded, &custody); err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*argumentCustody, *json.RawMessage){
		"raw": func(value *argumentCustody, _ *json.RawMessage) {
			value.RawField = json.RawMessage(`"{\"operation\":\"stat\",\"path\":\"/workspace\"}"`)
		},
		"raw_digest": func(value *argumentCustody, _ *json.RawMessage) {
			value.RawFieldSHA256 = "sha256:" + string(bytes.Repeat([]byte{'0'}, 64))
		},
		"decoded_digest": func(value *argumentCustody, _ *json.RawMessage) {
			value.DecodedBytesSHA256 = "sha256:" + string(bytes.Repeat([]byte{'1'}, 64))
		},
		"decoded": func(_ *argumentCustody, value *json.RawMessage) {
			*value = json.RawMessage(`{"operation":"stat","path":"/workspace"}`)
		},
		"decode_count": func(value *argumentCustody, _ *json.RawMessage) { value.DecodeCount = 2 },
	} {
		t.Run(name, func(t *testing.T) {
			candidateCustody := custody
			candidateDecoded := append(json.RawMessage(nil), decoded...)
			mutate(&candidateCustody, &candidateDecoded)
			if validateOpenAIArgumentCustody(candidateDecoded, &candidateCustody) == nil {
				t.Fatal("accepted argument custody drift")
			}
		})
	}
}

func TestProviderSpecificRequestClosure(t *testing.T) {
	for _, adapter := range []string{"openai-responses-v1", "anthropic-messages-v1"} {
		t.Run(adapter, func(t *testing.T) {
			withAdapter(t, adapter)
			raw, err := requestBody(runInput{Model: "held-model", Prompt: "p", ProviderSchema: json.RawMessage(`{"type":"object"}`)}, json.RawMessage("{}\n"))
			if err != nil {
				t.Fatal(err)
			}
			var body map[string]any
			if err := json.Unmarshal(raw, &body); err != nil {
				t.Fatal(err)
			}
			if body["model"] != "held-model" || len(body["tools"].([]any)) != 2 {
				t.Fatalf("request contract drift: %s", raw)
			}
			if err := selfTest(); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestProviderRequestPreservesExactSchemaBytes(t *testing.T) {
	schemas := []json.RawMessage{
		json.RawMessage("{\"z\": 1, \"a\": {\"y\": 2, \"x\": 1}}\n"),
		json.RawMessage("{\n  \"type\": \"object\",\n  \"additionalProperties\": false\n}\n"),
	}
	for _, adapter := range []string{"openai-responses-v1", "anthropic-messages-v1"} {
		for index, schema := range schemas {
			t.Run(fmt.Sprintf("%s-%d", adapter, index), func(t *testing.T) {
				withAdapter(t, adapter)
				raw, err := requestBody(runInput{Model: "held-model", Prompt: "p", ProviderSchema: schema}, json.RawMessage("{}\n"))
				if err != nil {
					t.Fatal(err)
				}
				if bytes.Count(raw, schema) != 1 {
					t.Fatalf("schema was normalized or substituted: %s", raw)
				}
				var value any
				if err := json.Unmarshal(raw, &value); err != nil {
					t.Fatalf("invalid request JSON: %v", err)
				}
			})
		}
	}
}

func TestLosslessRequestPayloadAndCustodyBinding(t *testing.T) {
	schema := []byte("{\n  \"type\": \"object\"\n}\n")
	body := append([]byte("{\"schema\":"), schema...)
	body = append(body, []byte(",\"value\":1}\n")...)
	payload, err := makeRequestPayload(body, schema)
	if err != nil {
		t.Fatal(err)
	}
	if payload.Encoding != "base64-rfc4648-canonical" || payload.Bytes != len(body) || payload.SHA256 != digestBytes(body) || payload.ProviderSchemaOccurrences != 1 {
		t.Fatal("lossless payload metadata drift")
	}
	receipt := expectedRequestCustody(body, schema)
	if err := validateRequestCustody(&receipt, body, schema); err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*requestCustody){
		"boolean_semantics": func(value *requestCustody) { value.EndpointWritePrepared = false },
		"decode_count":      func(value *requestCustody) { value.DecodeCount = 2 },
		"length":            func(value *requestCustody) { value.Bytes++ },
		"root":              func(value *requestCustody) { value.SHA256 = digestBytes([]byte("drift")) },
		"schema_count":      func(value *requestCustody) { value.ProviderSchemaOccurrences = 0 },
	} {
		t.Run(name, func(t *testing.T) {
			candidate := receipt
			mutate(&candidate)
			if validateRequestCustody(&candidate, body, schema) == nil {
				t.Fatal("accepted request custody drift")
			}
		})
	}
	compact := bytes.ReplaceAll(body, []byte(" "), nil)
	compactReceipt := expectedRequestCustody(compact, schema)
	if validateRequestCustody(&compactReceipt, body, schema) == nil {
		t.Fatal("accepted semantic-only reformatted request")
	}
}

func TestStrictRunInputRejectsUnknownAndBooleanNumericFields(t *testing.T) {
	base := `{"run_id":"r","model":"m","prompt":"p","packet_path":"/input/packet.json","packet_bytes":3,"packet_sha256":"sha256:x","provider_schema":{},"provider_schema_path":"/input/provider-schema.json","provider_schema_bytes":3,"provider_schema_sha256":"sha256:y","materialization_receipt_path":"/input/materialization-receipt.json","output_dir":"/evidence"}`
	if _, err := strictRunInput([]byte(base)); err != nil {
		t.Fatal(err)
	}
	for _, raw := range []string{
		strings.Replace(base, `"packet_bytes":3`, `"packet_bytes":false`, 1),
		strings.Replace(base, `"provider_schema_bytes":3`, `"provider_schema_bytes":true`, 1),
		strings.TrimSuffix(base, "}") + `,"inline_schema":{}}`,
		base + `{}`,
	} {
		if _, err := strictRunInput([]byte(raw)); err == nil {
			t.Fatalf("accepted invalid run input: %s", raw)
		}
	}
}

func TestToolBoundaryRejectsPathAndShapeAdversaries(t *testing.T) {
	for _, raw := range []string{
		`{"operation":"read","path":"/workspace/../secret"}`,
		`{"operation":"read","path":"/etc/passwd"}`,
		`{"operation":"read","path":"/workspace/packet.json","write":true}`,
		`{"operation":"write","path":"/workspace/packet.json"}`,
	} {
		if validateTool("read_file", json.RawMessage(raw)) == nil {
			t.Fatalf("accepted read adversary: %s", raw)
		}
	}
	if validateTool("shell", json.RawMessage(`{"command":"cat /etc/passwd"}`)) == nil {
		t.Fatal("accepted unrestricted shell")
	}
	if validateTool("shell", json.RawMessage(`{"argv":["git","status"],"cwd":"/workspace"}`)) == nil {
		t.Fatal("accepted git argv substitution")
	}
}

func validResponse() string {
	return `{
		"schema":"lean-correspondence.review-response.v1",
		"assignment_id":"lc-neutral",
		"relation_validation":"cannot_determine",
		"change_classification":"unprovable",
		"impact_closure":[{"item_id":"item-1","disposition":"blocked_unprovable","evidence_ids":["evidence-1"]}],
		"authority_scientific_inference":{"repository_authority_effect":"none","scientific_status":"not_established"},
		"uncertainty":["neutral fixture"]
	}`
}

func TestStageAResponseClosure(t *testing.T) {
	if err := validateStageA([]byte(validResponse())); err != nil {
		t.Fatal(err)
	}
	mutations := []string{
		strings.Replace(validResponse(), `"uncertainty":["neutral fixture"]`, `"uncertainty":["neutral fixture","neutral fixture"]`, 1),
		strings.Replace(validResponse(), `"evidence_ids":["evidence-1"]`, `"evidence_ids":[]`, 1),
		strings.Replace(validResponse(), `"schema":"lean-correspondence.review-response.v1"`, `"schema":"lean-correspondence.review-response.v1","execution_authorized":true`, 1),
	}
	for _, raw := range mutations {
		if validateStageA([]byte(raw)) == nil {
			t.Fatalf("accepted schema adversary: %s", raw)
		}
	}
}
