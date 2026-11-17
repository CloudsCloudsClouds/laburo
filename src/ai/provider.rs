//! AI configuration and the provider boundary.
//!
//! AI is opt-in. The default configuration is disabled, and an
//! [`OpenAiProvider`] that is disabled refuses to make a network call. Tier 1
//! callers can always use [`NoopProvider`] or the deterministic rankings in
//! [`super::rules`].

use anyhow::{Context, Result, bail};
use std::time::Duration;

/// Deterministic output of [`NoopProvider`]: a valid ranking response with no
/// entries, so callers keep their deterministic order.
pub const NOOP_FALLBACK: &str = r#"{"ranking":[]}"#;

/// Configuration for the optional OpenAI-compatible provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfig {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub timeout_ms: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".to_string(),
            model: "qwen2.5:1.5b".to_string(),
            api_key_env: "LABURO_AI_KEY".to_string(),
            timeout_ms: 20_000,
        }
    }
}

impl AiConfig {
    /// Read configuration from the environment.
    ///
    /// Variables: `LABURO_AI_ENABLED`, `LABURO_AI_BASE_URL`, `LABURO_AI_MODEL`,
    /// `LABURO_AI_TIMEOUT_MS`. The API key is read from the variable named by
    /// `api_key_env`, which defaults to `LABURO_AI_KEY`. Invalid values fall
    /// back to the default rather than breaking Tier 1.
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let mut config = Self::default();
        if let Some(value) = lookup("LABURO_AI_ENABLED") {
            config.enabled = parse_bool(&value);
        }
        if let Some(value) = non_empty(lookup("LABURO_AI_BASE_URL")) {
            config.base_url = value.trim().trim_end_matches('/').to_string();
        }
        if let Some(value) = non_empty(lookup("LABURO_AI_MODEL")) {
            config.model = value.trim().to_string();
        }
        if let Some(value) = non_empty(lookup("LABURO_AI_TIMEOUT_MS"))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|millis| *millis > 0)
        {
            config.timeout_ms = value;
        }
        config
    }

    /// Full chat-completions endpoint derived from `base_url`.
    pub fn endpoint(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        }
    }

    /// Resolve the API key from the environment variable named by
    /// `api_key_env`. Local providers may not need one.
    pub fn api_key(&self) -> Option<String> {
        let name = self.api_key_env.trim();
        if name.is_empty() {
            return None;
        }
        non_empty(std::env::var(name).ok())
    }
}

/// Boundary between deterministic rules and a model.
pub trait AiProvider {
    fn rank(&self, prompt: &str) -> Result<String>;
}

/// A provider that never calls a model and returns a deterministic fallback.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProvider;

impl AiProvider for NoopProvider {
    fn rank(&self, _prompt: &str) -> Result<String> {
        Ok(NOOP_FALLBACK.to_string())
    }
}

/// Minimal OpenAI-compatible provider.
///
/// It POSTs to `<base_url>/chat/completions` and reads
/// `choices[0].message.content`. Construction does not perform network I/O;
/// a disabled provider returns an error from [`AiProvider::rank`] before any
/// request is attempted.
pub struct OpenAiProvider {
    enabled: bool,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    agent: ureq::Agent,
}

impl OpenAiProvider {
    pub fn new(config: &AiConfig) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_millis(config.timeout_ms.max(1))))
            .build()
            .into();
        Self {
            enabled: config.enabled,
            endpoint: config.endpoint(),
            model: config.model.clone(),
            // Do not read a key when the provider is disabled.
            api_key: if config.enabled {
                config.api_key()
            } else {
                None
            },
            agent,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl AiProvider for OpenAiProvider {
    fn rank(&self, prompt: &str) -> Result<String> {
        if !self.enabled {
            bail!("AI is disabled; set LABURO_AI_ENABLED=1 to enable it");
        }

        let payload = serde_json::json!({
            "model": self.model.as_str(),
            "messages": [
                {
                    "role": "system",
                    "content": "You rank deterministic quality findings. Reply with JSON only."
                },
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.0,
            "stream": false
        });

        let mut request = self.agent.post(&self.endpoint);
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let mut response = request
            .send_json(payload)
            .context("calling the AI provider")?;
        let body = response
            .body_mut()
            .read_to_string()
            .context("reading the AI response")?;
        let parsed: serde_json::Value =
            serde_json::from_str(&body).context("AI provider returned invalid JSON")?;
        let content = parsed
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .context("AI provider response is missing choices[0].message.content")?;
        Ok(content.to_string())
    }
}

/// Build the provider that matches the configuration. Disabled configuration
/// yields [`NoopProvider`], so no network path is reachable by default.
pub fn provider_from_config(config: &AiConfig) -> Box<dyn AiProvider> {
    if config.enabled {
        Box::new(OpenAiProvider::new(config))
    } else {
        Box::new(NoopProvider)
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_is_disabled_by_default() {
        let config = AiConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.api_key_env, "LABURO_AI_KEY");

        let looked_up = AiConfig::from_lookup(|_| None);
        assert_eq!(looked_up, config);

        let provider = provider_from_config(&config);
        assert_eq!(provider.rank("prompt").unwrap(), NOOP_FALLBACK);
    }

    #[test]
    fn environment_values_are_parsed_without_breaking_defaults() {
        let config = AiConfig::from_lookup(|key| match key {
            "LABURO_AI_ENABLED" => Some("yes".to_string()),
            "LABURO_AI_BASE_URL" => Some("http://127.0.0.1:8080/v1/".to_string()),
            "LABURO_AI_MODEL" => Some("test-model".to_string()),
            "LABURO_AI_TIMEOUT_MS" => Some("1500".to_string()),
            _ => None,
        });
        assert!(config.enabled);
        assert_eq!(config.base_url, "http://127.0.0.1:8080/v1");
        assert_eq!(config.model, "test-model");
        assert_eq!(config.timeout_ms, 1_500);
        assert_eq!(
            config.endpoint(),
            "http://127.0.0.1:8080/v1/chat/completions"
        );

        let invalid = AiConfig::from_lookup(|key| match key {
            "LABURO_AI_ENABLED" => Some("maybe".to_string()),
            "LABURO_AI_TIMEOUT_MS" => Some("soon".to_string()),
            _ => None,
        });
        assert!(!invalid.enabled);
        assert_eq!(invalid.timeout_ms, 20_000);
    }

    #[test]
    fn endpoint_does_not_double_append() {
        let config = AiConfig {
            base_url: "http://localhost:11434/v1/chat/completions".to_string(),
            ..AiConfig::default()
        };
        assert_eq!(
            config.endpoint(),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn disabled_openai_provider_makes_no_call() {
        let provider = OpenAiProvider::new(&AiConfig::default());
        assert!(!provider.is_enabled());
        let error = provider.rank("prompt").unwrap_err().to_string();
        assert!(error.contains("disabled"));
    }

    #[test]
    fn noop_provider_is_deterministic() {
        let first = NoopProvider.rank("one").unwrap();
        let second = NoopProvider.rank("two").unwrap();
        assert_eq!(first, NOOP_FALLBACK);
        assert_eq!(first, second);
    }
}
