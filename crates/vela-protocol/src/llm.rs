//! LLM backends — Gemini, OpenRouter, Groq, Anthropic.

use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub backend: Backend,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    Gemini,
    OpenRouter,
    Groq,
    Anthropic,
}

impl Backend {
    pub fn label(&self) -> &'static str {
        match self {
            Backend::Gemini => "Gemini 3 Flash",
            Backend::OpenRouter => "OpenRouter (free tier)",
            Backend::Groq => "Groq GPT-OSS 120B",
            Backend::Anthropic => "Anthropic",
        }
    }
}

/// Free models available on OpenRouter, ordered by quality.
pub const FREE_MODELS: &[&str] = &[
    "qwen/qwen3.6-plus:free",
    "meta-llama/llama-4-maverick:free",
    "google/gemma-3-27b-it:free",
    "mistralai/mistral-small-3.1-24b-instruct:free",
    "openai/gpt-oss-120b:free",
];

/// Return the best available free model on OpenRouter.
pub fn best_free_model() -> &'static str {
    FREE_MODELS[0]
}

impl LlmConfig {
    /// Auto-detect backend from environment variables.
    pub fn from_env(backend_override: Option<&str>) -> Result<Self, String> {
        let or_model = best_free_model();
        let backends: [(&str, &str, &str); 4] = [
            ("gemini", "GOOGLE_API_KEY", "gemini-3-flash-preview"),
            ("openrouter", "OPENROUTER_API_KEY", or_model),
            ("groq", "GROQ_API_KEY", "openai/gpt-oss-120b"),
            ("anthropic", "ANTHROPIC_API_KEY", "claude-sonnet-4-20250514"),
        ];

        // If override specified, look for that one
        if let Some(name) = backend_override {
            for (n, env_key, model) in &backends {
                if *n == name {
                    if let Ok(key) = std::env::var(env_key)
                        && !key.trim().is_empty()
                    {
                        return Ok(Self {
                            backend: match *n {
                                "gemini" => Backend::Gemini,
                                "openrouter" => Backend::OpenRouter,
                                "groq" => Backend::Groq,
                                "anthropic" => Backend::Anthropic,
                                _ => unreachable!(),
                            },
                            api_key: key,
                            model: model.to_string(),
                        });
                    }
                    return Err(format!("Set {} for {} backend", env_key, n));
                }
            }
        }

        // Auto-detect
        for (n, env_key, model) in &backends {
            if let Ok(key) = std::env::var(env_key)
                && !key.trim().is_empty()
            {
                return Ok(Self {
                    backend: match *n {
                        "gemini" => Backend::Gemini,
                        "openrouter" => Backend::OpenRouter,
                        "groq" => Backend::Groq,
                        "anthropic" => Backend::Anthropic,
                        _ => unreachable!(),
                    },
                    api_key: key,
                    model: model.to_string(),
                });
            }
        }

        Err("No API key found. Set one of:\n  \
             GOOGLE_API_KEY     (Gemini 3 Flash)\n  \
             OPENROUTER_API_KEY (GPT-OSS 120B — free)\n  \
             GROQ_API_KEY       (GPT-OSS 120B — free)\n  \
             ANTHROPIC_API_KEY  (Anthropic)"
            .to_string())
    }
}

/// Call the configured LLM backend expecting a text/Markdown response (not JSON).
/// Uses text mode for Gemini (no JSON mime constraint) and higher token limit.
/// Retries up to 3 times with exponential backoff on transient failures.
pub async fn call_text(
    client: &Client,
    config: &LlmConfig,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let label = format!("LLM text request ({})", config.backend.label());
    crate::retry::retry_with_backoff(&label, 3, || {
        let client = client.clone();
        let api_key = config.api_key.clone();
        let model = config.model.clone();
        let system = system.to_string();
        let user_msg = user_msg.to_string();
        let backend = config.backend;
        async move {
            match backend {
                Backend::Gemini => {
                    call_gemini_text(&client, &api_key, &model, &system, &user_msg).await
                }
                Backend::OpenRouter => {
                    call_openai_compat(
                        &client,
                        "https://openrouter.ai/api/v1/chat/completions",
                        &api_key,
                        &model,
                        &system,
                        &user_msg,
                    )
                    .await
                }
                Backend::Groq => {
                    call_openai_compat(
                        &client,
                        "https://api.groq.com/openai/v1/chat/completions",
                        &api_key,
                        &model,
                        &system,
                        &user_msg,
                    )
                    .await
                }
                Backend::Anthropic => {
                    call_anthropic(&client, &api_key, &model, &system, &user_msg).await
                }
            }
        }
    })
    .await
}

/// Call the configured LLM backend. Returns raw text response.
/// Retries up to 3 times with exponential backoff on transient failures.
pub async fn call(
    client: &Client,
    config: &LlmConfig,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let label = format!("LLM request ({})", config.backend.label());
    crate::retry::retry_with_backoff(&label, 3, || {
        let client = client.clone();
        let api_key = config.api_key.clone();
        let model = config.model.clone();
        let system = system.to_string();
        let user_msg = user_msg.to_string();
        let backend = config.backend;
        async move {
            match backend {
                Backend::Gemini => call_gemini(&client, &api_key, &model, &system, &user_msg).await,
                Backend::OpenRouter => {
                    call_openai_compat(
                        &client,
                        "https://openrouter.ai/api/v1/chat/completions",
                        &api_key,
                        &model,
                        &system,
                        &user_msg,
                    )
                    .await
                }
                Backend::Groq => {
                    call_openai_compat(
                        &client,
                        "https://api.groq.com/openai/v1/chat/completions",
                        &api_key,
                        &model,
                        &system,
                        &user_msg,
                    )
                    .await
                }
                Backend::Anthropic => {
                    call_anthropic(&client, &api_key, &model, &system, &user_msg).await
                }
            }
        }
    })
    .await
}

async fn call_gemini(
    client: &Client,
    api_key: &str,
    model: &str,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let body = serde_json::json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"parts": [{"text": user_msg}]}],
        "generationConfig": {
            "responseMimeType": "application/json",
            "temperature": 0.2,
            "maxOutputTokens": 8000
        }
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini {status}: {}", &text[..text.len().min(200)]));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Gemini parse: {e}"))?;
    json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Gemini: no candidates in response".to_string())
}

async fn call_gemini_text(
    client: &Client,
    api_key: &str,
    model: &str,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let body = serde_json::json!({
        "systemInstruction": {"parts": [{"text": system}]},
        "contents": [{"parts": [{"text": user_msg}]}],
        "generationConfig": {
            "responseMimeType": "text/plain",
            "temperature": 0.3,
            "maxOutputTokens": 16000
        }
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Gemini request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini {status}: {}", &text[..text.len().min(200)]));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Gemini parse: {e}"))?;
    json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Gemini: no candidates in response".to_string())
}

async fn call_openai_compat(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_msg}
        ],
        "temperature": 0.2,
        "max_tokens": 8000
    });

    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("{status}: {}", &text[..text.len().min(200)]));
    }

    let json: Value = resp.json().await.map_err(|e| format!("Parse: {e}"))?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No content in response".to_string())
}

async fn call_anthropic(
    client: &Client,
    api_key: &str,
    model: &str,
    system: &str,
    user_msg: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 8000,
        "system": system,
        "messages": [{"role": "user", "content": user_msg}]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("Content-Type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Anthropic {status}: {}",
            &text[..text.len().min(200)]
        ));
    }

    let json: Value = resp.json().await.map_err(|e| format!("Parse: {e}"))?;
    let content = json["content"].as_array().ok_or("No content array")?;
    Ok(content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join(""))
}

/// Parse JSON from LLM output, stripping markdown fences if present.
pub fn parse_json(text: &str) -> Result<Value, String> {
    let mut s = text.trim();
    if s.starts_with("```")
        && let Some(pos) = s.find('\n')
    {
        s = &s[pos + 1..];
    }
    if s.ends_with("```") {
        s = &s[..s.len() - 3];
    }
    s = s.trim();
    if s.starts_with("json") {
        s = s[4..].trim();
    }
    serde_json::from_str(s).map_err(|e| format!("JSON parse: {e}"))
}
