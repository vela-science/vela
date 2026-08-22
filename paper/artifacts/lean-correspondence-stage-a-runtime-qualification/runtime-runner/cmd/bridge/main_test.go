package main

import (
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
	openai := json.RawMessage(`{"id":"resp-1","output":[{"type":"function_call","call_id":"call-1","name":"shell","arguments":{"argv":["git","--no-optional-locks","status","--short"],"cwd":"/workspace"}}]}`)
	_, tools, err := parseResponse(openai)
	if err != nil || len(tools) != 1 || tools[0].CallID != "call-1" {
		t.Fatalf("OpenAI tool parse failed: %#v %v", tools, err)
	}
	next, err := continuation(json.RawMessage(`{"model":"held","input":[]}`), openai, json.RawMessage(`{"stdout":""}`), tools[0])
	if err != nil || !json.Valid(next) {
		t.Fatalf("OpenAI continuation failed: %s %v", next, err)
	}

	useAdapter(t, "anthropic-messages-v1")
	anthropic := json.RawMessage(`{"content":[{"type":"tool_use","id":"tool-1","name":"read_file","input":{"operation":"read","path":"/workspace/packet.json"}}]}`)
	_, tools, err = parseResponse(anthropic)
	if err != nil || len(tools) != 1 || tools[0].CallID != "tool-1" {
		t.Fatalf("Anthropic tool parse failed: %#v %v", tools, err)
	}
	next, err = continuation(json.RawMessage(`{"model":"held","messages":[]}`), anthropic, json.RawMessage(`{"content":"x"}`), tools[0])
	if err != nil || !json.Valid(next) {
		t.Fatalf("Anthropic continuation failed: %s %v", next, err)
	}
}
