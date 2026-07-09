//! Scratch probe core: spawn a **temporary** proxy (scratch port + secret), inject candidate
//! provider/base_url/key/model via env, probe `/v1/models` or `/v1/messages`, classify by status,
//! then kill the child. **Never** writes config, mutates `AppState`, or touches the formal proxy
//! serving Science. Shared by profile switch and connection validate/save paths.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::json;

use crate::runtime::i18n::i18n_err;
use crate::runtime::operation::{self, OperationStage, OperationTrace};

/// Probe kind: `Models` checks endpoint + auth; `Message` checks a concrete model when required.
pub enum ProbeKind {
    Models,
    Message,
}

impl ProbeKind {
    fn as_str(&self) -> &'static str {
        match self {
            ProbeKind::Models => "models",
            ProbeKind::Message => "message",
        }
    }
}

/// Raw result of one upstream probe.
pub struct ProbeResult {
    pub status: Option<u16>,
    pub body: String,
}

/// Classified probe outcome for save/fetch/switch commands.
#[derive(Debug, PartialEq)]
pub enum ProbeOutcome {
    Ok,                     // 200 — safe to commit
    Auth(u16),              // 401/403 — bad key/permission
    ModelError(u16),        // 400/404/422 — model rejected
    Unsupported(u16),       // 405 — endpoint does not support this probe
    Ambiguous(Option<u16>), // 429/5xx/other — inconclusive; user may skip verify
    NoResponse,             // network / no response
}

/// Map HTTP status to [`ProbeOutcome`] (pure).
pub fn classify(status: Option<u16>) -> ProbeOutcome {
    match status {
        Some(200) => ProbeOutcome::Ok,
        Some(c @ (401 | 403)) => ProbeOutcome::Auth(c),
        Some(c @ (400 | 404 | 422)) => ProbeOutcome::ModelError(c),
        Some(405) => ProbeOutcome::Unsupported(405),
        Some(c) => ProbeOutcome::Ambiguous(Some(c)), // 429 / 5xx / other
        None => ProbeOutcome::NoResponse,
    }
}

/// `fetch_models` fallback source label (pure): 4xx → `"unsupported"`; 429/5xx/none → `"network"`.
/// Auth outcomes are handled separately and must not mask a bad key.
pub fn discovery_fallback_source(outcome: &ProbeOutcome) -> &'static str {
    match outcome {
        ProbeOutcome::ModelError(_) | ProbeOutcome::Unsupported(_) => "unsupported",
        _ => "network",
    }
}

/// Pick a free loopback port via `bind("127.0.0.1:0")`, then drop the listener (TOCTOU-safe enough with bind retry on launch).
pub fn pick_scratch_port() -> Option<u16> {
    use std::net::TcpListener;
    let l = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = l.local_addr().ok()?.port();
    // Listener drops here; port is released.
    Some(port)
}

/// RAII guard: kills the scratch proxy child on drop (including panic/early return).
struct ScratchGuard(Option<Child>);
impl Drop for ScratchGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Env pairs for a scratch launch (pure, testable). Key goes in `key_env`; base/model envs only when non-empty.
/// Native adapters (deepseek/qwen) pass empty base_url → hard-coded upstream endpoints in the proxy.
pub fn scratch_env(
    provider: &str,
    key_env: &str,
    key: &str,
    base_url: &str,
    model: Option<&str>,
    relay_thinking: &str,
) -> Vec<(String, String)> {
    let mut v = vec![(key_env.to_string(), key.to_string())];
    if !base_url.is_empty() {
        let env = if matches!(provider, "openai-custom" | "openai-responses") {
            "CSP_OPENAI_BASE_URL"
        } else {
            "CSP_RELAY_BASE_URL"
        };
        v.push((env.to_string(), base_url.to_string()));
    }
    if let Some(m) = model {
        if !m.is_empty() {
            let env = if matches!(provider, "openai-custom" | "openai-responses") {
                "CSP_OPENAI_MODEL"
            } else {
                "CSP_RELAY_MODEL"
            };
            v.push((env.to_string(), m.to_string()));
        }
    }
    if !matches!(provider, "openai-custom" | "openai-responses") && !relay_thinking.is_empty() {
        v.push(("CSP_RELAY_THINKING".to_string(), relay_thinking.to_string()));
    }
    v
}

/// Scratch probe target: `provider` is passed to `--provider` (native deepseek/qwen or relay).
/// `key_env` selects which env var receives the candidate key; relay uses `CSP_RELAY_*` base/model envs.
pub struct ScratchTarget<'a> {
    pub provider: &'a str,
    pub key_env: &'a str,
    pub base_url: &'a str,
    pub key: &'a str,
    pub model: Option<&'a str>,
    pub relay_thinking: &'a str, // relay thinking_policy → CSP_RELAY_THINKING when non-empty
}

/// Spawn scratch proxy, probe upstream, then kill. Does not touch config / AppState / formal proxy.
/// Caller supplies `py` and `script` from `asset_root` + `find_exe`; keys only via env (never argv).
pub fn scratch_probe(
    py: &Path,
    script: &Path,
    target: &ScratchTarget,
    kind: ProbeKind,
    trace: Option<&OperationTrace>,
) -> ProbeResult {
    let port = match pick_scratch_port() {
        Some(p) => p,
        None => {
            return ProbeResult {
                status: None,
                body: i18n_err("errScratchNoPort", json!({})),
            }
        }
    };
    let secret = match crate::proc::gen_secret() {
        Ok(s) => s,
        Err(_) => {
            return ProbeResult {
                status: None,
                body: i18n_err("errScratchNoSecret", json!({})),
            }
        }
    };
    let mut cmd = Command::new(py);
    if let Some(t) = trace {
        t.stage(
            OperationStage::ScratchSpawn,
            format!("provider={} kind={}", target.provider, kind.as_str()),
        );
    }
    cmd.arg(script)
        .arg("--provider")
        .arg(target.provider) // native deepseek/qwen or relay (Python accepts these)
        .arg("--port")
        .arg(port.to_string())
        .arg("--auth-token")
        .arg(&secret)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Inject key/base/model via env only (never argv — avoids `ps` leakage).
    for (k, v) in scratch_env(
        target.provider,
        target.key_env,
        target.key,
        target.base_url,
        target.model,
        target.relay_thinking,
    ) {
        cmd.env(k, v);
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                status: None,
                body: i18n_err("errScratchSpawnFailed", json!({ "detail": e.to_string() })),
            }
        }
    };
    let _guard = ScratchGuard(Some(child)); // killed on drop
                                            // Health poll budget ~4s.
    let mut alive = false;
    for _ in 0..(operation::SCRATCH_READY_BUDGET_MS / operation::POLL_INTERVAL_MS) {
        std::thread::sleep(Duration::from_millis(operation::POLL_INTERVAL_MS));
        if crate::proc::http_health(port, Some(&secret), operation::LOCAL_HEALTH_TIMEOUT_MS) {
            alive = true;
            break;
        }
    }
    if let Some(t) = trace {
        t.stage(
            OperationStage::ScratchHealth,
            if alive { "ready" } else { "not_ready" },
        );
    }
    if !alive {
        return ProbeResult {
            status: None,
            body: i18n_err("errScratchNotReady", json!({})),
        };
    }
    match kind {
        ProbeKind::Models => {
            if let Some(t) = trace {
                t.stage(OperationStage::ScratchUpstreamProbe, "GET /v1/models");
            }
            match crate::proc::http_get_body(
                port,
                Some(&secret),
                "/v1/models",
                operation::UPSTREAM_PROBE_TIMEOUT_MS,
            ) {
                Some((code, body)) => ProbeResult {
                    status: Some(code),
                    body,
                },
                None => ProbeResult {
                    status: None,
                    body: String::new(),
                },
            }
        }
        ProbeKind::Message => {
            // Placeholder model id; relay may override via CSP_RELAY_MODEL.
            let payload = br#"{"model":"claude-opus-4-8","max_tokens":1,"messages":[{"role":"user","content":"ping"}]}"#;
            if let Some(t) = trace {
                t.stage(OperationStage::ScratchUpstreamProbe, "POST /v1/messages");
            }
            match crate::proc::http_post_status(
                port,
                Some(&secret),
                "/v1/messages",
                payload,
                operation::UPSTREAM_PROBE_TIMEOUT_MS,
            ) {
                Some(code) => ProbeResult {
                    status: Some(code),
                    body: String::new(),
                },
                None => ProbeResult {
                    status: None,
                    body: String::new(),
                },
            }
        }
    }
    // _guard drop kills scratch proxy.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_status_to_outcome() {
        assert_eq!(classify(Some(200)), ProbeOutcome::Ok);
        assert_eq!(classify(Some(401)), ProbeOutcome::Auth(401));
        assert_eq!(classify(Some(403)), ProbeOutcome::Auth(403));
        assert_eq!(classify(Some(404)), ProbeOutcome::ModelError(404));
        assert_eq!(classify(Some(400)), ProbeOutcome::ModelError(400));
        // 405 = endpoint does not support this probe (not lumped into Ambiguous).
        assert_eq!(classify(Some(405)), ProbeOutcome::Unsupported(405));
        assert_eq!(classify(Some(429)), ProbeOutcome::Ambiguous(Some(429)));
        assert_eq!(classify(Some(502)), ProbeOutcome::Ambiguous(Some(502)));
        assert_eq!(classify(None), ProbeOutcome::NoResponse);
    }

    #[test]
    fn discovery_fallback_source_splits_unsupported_from_network() {
        // fetch_models fallback: 4xx → unsupported; 5xx/429/none → network.
        assert_eq!(
            discovery_fallback_source(&ProbeOutcome::ModelError(404)),
            "unsupported"
        );
        assert_eq!(
            discovery_fallback_source(&ProbeOutcome::Unsupported(405)),
            "unsupported"
        );
        assert_eq!(
            discovery_fallback_source(&ProbeOutcome::Ambiguous(Some(429))),
            "network"
        );
        assert_eq!(
            discovery_fallback_source(&ProbeOutcome::NoResponse),
            "network"
        );
    }

    #[test]
    fn scratch_env_native_uses_native_key_env_and_no_relay_base() {
        // Native: key in DEEPSEEK_API_KEY; never set CSP_RELAY_BASE_URL.
        let env = scratch_env("deepseek", "DEEPSEEK_API_KEY", "sk-x", "", None, "");
        assert_eq!(
            env,
            vec![("DEEPSEEK_API_KEY".to_string(), "sk-x".to_string())]
        );
    }

    #[test]
    fn scratch_env_relay_sets_base_url_and_model() {
        let env = scratch_env(
            "relay",
            "CSP_RELAY_KEY",
            "sk-y",
            "https://r/claude",
            Some("m1"),
            "",
        );
        assert_eq!(
            env,
            vec![
                ("CSP_RELAY_KEY".to_string(), "sk-y".to_string()),
                (
                    "CSP_RELAY_BASE_URL".to_string(),
                    "https://r/claude".to_string()
                ),
                ("CSP_RELAY_MODEL".to_string(), "m1".to_string()),
            ]
        );
    }

    #[test]
    fn scratch_env_models_discovery_does_not_pin_relay_model() {
        let env = scratch_env(
            "relay",
            "CSP_RELAY_KEY",
            "sk-y",
            "https://r/claude",
            None,
            "",
        );
        assert!(env.iter().any(|(k, _)| k == "CSP_RELAY_BASE_URL"));
        assert!(!env.iter().any(|(k, _)| k == "CSP_RELAY_MODEL"));
        assert!(!env.iter().any(|(k, _)| k == "CSP_OPENAI_MODEL"));
    }

    #[test]
    fn scratch_env_models_discovery_does_not_pin_openai_model() {
        let env = scratch_env(
            "openai-custom",
            "CSP_OPENAI_KEY",
            "sk-z",
            "https://open.bigmodel.cn/api/paas/v4",
            None,
            "",
        );
        assert!(env.iter().any(|(k, _)| k == "CSP_OPENAI_BASE_URL"));
        assert!(!env.iter().any(|(k, _)| k == "CSP_OPENAI_MODEL"));
        assert!(!env.iter().any(|(k, _)| k == "CSP_RELAY_MODEL"));
    }

    #[test]
    fn scratch_env_openai_custom_sets_openai_base_and_model() {
        let env = scratch_env(
            "openai-custom",
            "CSP_OPENAI_KEY",
            "sk-z",
            "https://open.bigmodel.cn/api/paas/v4",
            Some("glm-4.5"),
            "enabled",
        );
        assert_eq!(
            env,
            vec![
                ("CSP_OPENAI_KEY".to_string(), "sk-z".to_string()),
                (
                    "CSP_OPENAI_BASE_URL".to_string(),
                    "https://open.bigmodel.cn/api/paas/v4".to_string()
                ),
                ("CSP_OPENAI_MODEL".to_string(), "glm-4.5".to_string()),
            ]
        );
    }

    #[test]
    fn scratch_env_openai_responses_sets_openai_base_and_model() {
        let env = scratch_env(
            "openai-responses",
            "CSP_OPENAI_KEY",
            "sk-z",
            "https://api.openai.com/v1",
            Some("gpt-5.2"),
            "enabled",
        );
        assert_eq!(
            env,
            vec![
                ("CSP_OPENAI_KEY".to_string(), "sk-z".to_string()),
                (
                    "CSP_OPENAI_BASE_URL".to_string(),
                    "https://api.openai.com/v1".to_string()
                ),
                ("CSP_OPENAI_MODEL".to_string(), "gpt-5.2".to_string()),
            ]
        );
    }

    #[test]
    fn scratch_env_relay_injects_thinking_policy() {
        let env = scratch_env(
            "relay",
            "CSP_RELAY_KEY",
            "sk-y",
            "https://r/claude",
            Some("m1"),
            "enabled",
        );
        assert!(env.contains(&("CSP_RELAY_THINKING".to_string(), "enabled".to_string())));
    }

    #[test]
    fn scratch_env_empty_thinking_not_injected() {
        let env = scratch_env(
            "relay",
            "CSP_RELAY_KEY",
            "sk-y",
            "https://r/claude",
            None,
            "",
        );
        assert!(!env.iter().any(|(k, _)| k == "CSP_RELAY_THINKING"));
    }

    #[test]
    fn pick_scratch_port_returns_usable_nonreserved_port() {
        let p = pick_scratch_port().expect("should allocate a port");
        assert!(p > 1024, "ephemeral port should be > 1024");
        assert_ne!(p, 8765, "must not collide with real Science port 8765");
    }

    #[test]
    fn two_picks_are_bindable() {
        // After bind(:0) the listener is dropped; port should be bindable again (retry for OS races).
        use std::net::TcpListener;
        let rebound = (0..8).any(|_| {
            let p = pick_scratch_port().unwrap();
            TcpListener::bind(("127.0.0.1", p)).is_ok()
        });
        assert!(
            rebound,
            "port from pick_scratch_port should be released and re-bindable"
        );
    }
}
