//! Decision providers (spec §72-§75, §153).
//!
//! The core always speaks the Decision Schema. JEV is optional: a malformed
//! or unreachable response never becomes an internal decision — the local
//! provider is used instead.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::Value;

use crate::config::schema::DecisionConfig;
use crate::core::decision::{decide, DecisionInput, DecisionOutput};
use crate::error::{NodkrayError, NodkrayResult};

/// A source of workflow/effort/review-depth decisions.
pub trait DecisionProvider {
    fn id(&self) -> &'static str;
    fn decide(&self, input: &DecisionInput, force_st: bool) -> NodkrayResult<DecisionOutput>;
}

/// Built-in local heuristic provider (spec §74).
#[derive(Debug, Clone)]
pub struct LocalProvider {
    pub config: DecisionConfig,
}

impl DecisionProvider for LocalProvider {
    fn id(&self) -> &'static str {
        "local"
    }

    fn decide(&self, input: &DecisionInput, force_st: bool) -> NodkrayResult<DecisionOutput> {
        decide(input, &self.config.thresholds, force_st)
    }
}

/// HTTP (or injected) transport used by [`JevProvider`].
pub trait JevTransport {
    fn request(&self, payload: &Value) -> NodkrayResult<Value>;
}

/// Minimal `http://` POST client. HTTPS is out of V1 (no extra TLS crate).
#[derive(Debug, Clone)]
pub struct HttpJevTransport {
    pub url: String,
    pub api_key: Option<String>,
}

impl JevTransport for HttpJevTransport {
    fn request(&self, payload: &Value) -> NodkrayResult<Value> {
        let body = serde_json::to_string(payload).map_err(|err| {
            NodkrayError::internal("JEV_SERIALIZE_FAILED", err.to_string())
        })?;
        let raw = http_post_json(&self.url, &body, self.api_key.as_deref())?;
        serde_json::from_str(&raw).map_err(|err| {
            NodkrayError::network(
                "JEV_INVALID_JSON",
                format!("JEV returned non-JSON: {err}"),
            )
        })
    }
}

/// JEV adapter: validate schema, fall back to local on any failure (spec §73).
pub struct JevProvider<T: JevTransport> {
    pub transport: T,
    pub fallback: LocalProvider,
}

impl<T: JevTransport> DecisionProvider for JevProvider<T> {
    fn id(&self) -> &'static str {
        "jev"
    }

    fn decide(&self, input: &DecisionInput, force_st: bool) -> NodkrayResult<DecisionOutput> {
        match self.try_jev(input) {
            Ok(mut output) => {
                if force_st {
                    output.workflow = "ST".to_string();
                    output.review_depth = "fast".to_string();
                }
                Ok(output)
            }
            Err(error) => {
                tracing::warn!(
                    code = error.code(),
                    message = error.message(),
                    "JEV decision failed; falling back to local provider"
                );
                let mut local = self.fallback.decide(input, force_st)?;
                local.reasons.insert(
                    0,
                    format!("jev fallback: {}: {}", error.code(), error.message()),
                );
                Ok(local)
            }
        }
    }
}

impl<T: JevTransport> JevProvider<T> {
    fn try_jev(&self, input: &DecisionInput) -> NodkrayResult<DecisionOutput> {
        let payload = serde_json::json!({
            "title": input.title,
            "description": input.description,
        });
        let response = self.transport.request(&payload)?;
        parse_decision_schema(&response)
    }
}

/// Registry of decision providers (spec §153).
pub struct DecisionProviderRegistry {
    provider: Box<dyn DecisionProvider>,
}

impl DecisionProviderRegistry {
    /// Build the configured provider. Unknown ids fall back to local.
    pub fn from_config(config: &DecisionConfig) -> Self {
        Self {
            provider: resolve_provider(config),
        }
    }

    pub fn id(&self) -> &'static str {
        self.provider.id()
    }

    pub fn decide(
        &self,
        input: &DecisionInput,
        force_st: bool,
    ) -> NodkrayResult<DecisionOutput> {
        self.provider.decide(input, force_st)
    }
}

/// Classify using the configured provider (local by default).
pub fn classify(
    config: &DecisionConfig,
    input: &DecisionInput,
    force_st: bool,
) -> NodkrayResult<DecisionOutput> {
    DecisionProviderRegistry::from_config(config).decide(input, force_st)
}

fn resolve_provider(config: &DecisionConfig) -> Box<dyn DecisionProvider> {
    if config.provider != "jev" {
        return Box::new(LocalProvider {
            config: config.clone(),
        });
    }

    let url = match config.url.as_deref().filter(|url| !url.is_empty()) {
        Some(url) => url.to_string(),
        None => {
            tracing::warn!("decision.provider=jev but no URL; using local");
            return Box::new(LocalProvider {
                config: config.clone(),
            });
        }
    };
    let api_key = std::env::var(config.jev_api_key_env())
        .ok()
        .filter(|key| !key.is_empty());
    Box::new(JevProvider {
        transport: HttpJevTransport { url, api_key },
        fallback: LocalProvider {
            config: config.clone(),
        },
    })
}

/// Strict Decision Schema parser (spec §75). Arbitrary text is never accepted.
pub fn parse_decision_schema(value: &Value) -> NodkrayResult<DecisionOutput> {
    if !value.is_object() {
        return Err(NodkrayError::network(
            "JEV_SCHEMA_INVALID",
            "JEV response is not a JSON object",
        ));
    }

    let workflow = value
        .get("workflow")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_err("missing string field `workflow`"))?;
    let workflow = match workflow.to_ascii_uppercase().as_str() {
        "ST" | "ODD" | "SDD" => workflow.to_ascii_uppercase(),
        other => {
            return Err(schema_err(format!("unknown workflow `{other}`")));
        }
    };

    let effort = value
        .get("effort")
        .and_then(Value::as_u64)
        .ok_or_else(|| schema_err("missing numeric field `effort`"))?;
    if effort > 100 {
        return Err(schema_err("effort must be between 0 and 100"));
    }

    let review_depth = value
        .get("review_depth")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_err("missing string field `review_depth`"))?;
    let review_depth = match review_depth.to_ascii_lowercase().as_str() {
        "fast" | "balanced" | "deep" => review_depth.to_ascii_lowercase(),
        other => {
            return Err(schema_err(format!("unknown review_depth `{other}`")));
        }
    };

    let reasons = match value.get("reasons") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| schema_err("reasons must be an array of strings"))
            })
            .collect::<NodkrayResult<Vec<String>>>()?,
        None => {
            return Err(schema_err("missing array field `reasons`"));
        }
        Some(_) => {
            return Err(schema_err("reasons must be an array of strings"));
        }
    };

    Ok(DecisionOutput {
        workflow,
        effort: effort as u32,
        review_depth,
        reasons,
    })
}

fn schema_err(message: impl Into<String>) -> NodkrayError {
    NodkrayError::network("JEV_SCHEMA_INVALID", message.into())
}

struct HttpUrl {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> NodkrayResult<HttpUrl> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        NodkrayError::network(
            "JEV_URL_UNSUPPORTED",
            "JEV URL must be an http:// endpoint in V1",
        )
    })?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() {
        return Err(NodkrayError::network(
            "JEV_URL_INVALID",
            "JEV URL is missing a host",
        ));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => {
            let port = port.parse::<u16>().map_err(|_| {
                NodkrayError::network("JEV_URL_INVALID", format!("invalid port in {url}"))
            })?;
            (host.to_string(), port)
        }
        None => (authority.to_string(), 80),
    };
    Ok(HttpUrl { host, port, path })
}

fn http_post_json(url: &str, body: &str, api_key: Option<&str>) -> NodkrayResult<String> {
    let parsed = parse_http_url(url)?;
    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port)).map_err(|err| {
        NodkrayError::network("JEV_CONNECT_FAILED", err.to_string())
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .ok();

    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n",
        path = parsed.path,
        host = parsed.host,
        port = parsed.port,
        len = body.len(),
    );
    if let Some(key) = api_key {
        request.push_str(&format!("Authorization: Bearer {key}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).map_err(|err| {
        NodkrayError::network("JEV_WRITE_FAILED", err.to_string())
    })?;

    let mut raw = String::new();
    stream.read_to_string(&mut raw).map_err(|err| {
        NodkrayError::network("JEV_READ_FAILED", err.to_string())
    })?;
    let (_headers, response_body) = raw.split_once("\r\n\r\n").ok_or_else(|| {
        NodkrayError::network("JEV_BAD_RESPONSE", "JEV response is not valid HTTP")
    })?;
    Ok(response_body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ThresholdsConfig;

    fn thresholds() -> DecisionConfig {
        DecisionConfig {
            provider: "local".to_string(),
            thresholds: ThresholdsConfig {
                st_max: 20,
                odd_max: 60,
            },
            ..DecisionConfig::default()
        }
    }

    struct ScriptedTransport {
        response: NodkrayResult<Value>,
    }

    impl JevTransport for ScriptedTransport {
        fn request(&self, _payload: &Value) -> NodkrayResult<Value> {
            match &self.response {
                Ok(value) => Ok(value.clone()),
                Err(error) => Err(error.clone()),
            }
        }
    }

    fn jev(response: NodkrayResult<Value>) -> JevProvider<ScriptedTransport> {
        JevProvider {
            transport: ScriptedTransport { response },
            fallback: LocalProvider {
                config: thresholds(),
            },
        }
    }

    #[test]
    fn valid_schema_is_accepted() {
        let value = serde_json::json!({
            "workflow": "ODD",
            "effort": 47,
            "review_depth": "balanced",
            "reasons": ["multiple files", "moderate uncertainty"]
        });
        let parsed = parse_decision_schema(&value).expect("valid");
        assert_eq!(parsed.workflow, "ODD");
        assert_eq!(parsed.effort, 47);
        assert_eq!(parsed.review_depth, "balanced");
        assert_eq!(parsed.reasons.len(), 2);
    }

    #[test]
    fn arbitrary_text_is_rejected() {
        let err = parse_decision_schema(&Value::String("just ship it".into())).expect_err("text");
        assert_eq!(err.code(), "JEV_SCHEMA_INVALID");
    }

    #[test]
    fn unknown_workflow_is_rejected() {
        let value = serde_json::json!({
            "workflow": "maybe",
            "effort": 10,
            "review_depth": "fast",
            "reasons": []
        });
        let err = parse_decision_schema(&value).expect_err("workflow");
        assert_eq!(err.code(), "JEV_SCHEMA_INVALID");
    }

    #[test]
    fn jev_success_uses_remote_decision() {
        let provider = jev(Ok(serde_json::json!({
            "workflow": "SDD",
            "effort": 80,
            "review_depth": "deep",
            "reasons": ["architectural redesign"]
        })));
        let decision = provider
            .decide(
                &DecisionInput {
                    title: "rewrite".into(),
                    description: "redesign".into(),
                },
                false,
            )
            .expect("jev");
        assert_eq!(decision.workflow, "SDD");
        assert_eq!(provider.id(), "jev");
    }

    #[test]
    fn jev_invalid_schema_falls_back_to_local() {
        let provider = jev(Ok(serde_json::json!("not a decision")));
        let decision = provider
            .decide(
                &DecisionInput {
                    title: "fix typo".into(),
                    description: "Fix a typo in the README".into(),
                },
                false,
            )
            .expect("fallback");
        assert_eq!(decision.workflow, "ST");
        assert!(decision
            .reasons
            .iter()
            .any(|reason| reason.contains("jev fallback")));
    }

    #[test]
    fn jev_disabled_uses_local() {
        let config = thresholds();
        assert_eq!(config.provider, "local");
        let decision = classify(
            &config,
            &DecisionInput {
                title: "fix typo".into(),
                description: "Fix a typo in the README".into(),
            },
            false,
        )
        .expect("local");
        assert_eq!(decision.workflow, "ST");
    }

    #[test]
    fn jev_without_url_uses_local() {
        let mut config = thresholds();
        config.provider = "jev".to_string();
        let registry = DecisionProviderRegistry::from_config(&config);
        assert_eq!(registry.id(), "local");
    }
}
