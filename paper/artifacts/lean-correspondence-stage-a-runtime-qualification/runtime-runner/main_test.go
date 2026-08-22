package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func withAdapter(t *testing.T, adapter string) {
	t.Helper()
	previous := providerAdapter
	providerAdapter = adapter
	t.Cleanup(func() { providerAdapter = previous })
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
			raw, err := requestBody(runInput{Model: "held-model", Prompt: "p", Packet: json.RawMessage(`{}`), ProviderSchema: json.RawMessage(`{"type":"object"}`)})
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
