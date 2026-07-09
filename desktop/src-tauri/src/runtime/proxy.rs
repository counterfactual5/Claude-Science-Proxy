use serde_json::json;

use crate::runtime::i18n::i18n_err;

/// What the last `ensure_proxy` did (for one-click status hints).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProxyAction {
    Reused,    // same port/adapter/key fingerprint and healthy — keep child
    Restarted, // first start / key or profile change / unhealthy — restarted
}

impl ProxyAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ProxyAction::Reused => "reused",
            ProxyAction::Restarted => "restarted",
        }
    }
}

/// After health check, may we write `st.proxy`? Requires matching generation **and** secret
/// (guards the cold-start double-launch window where generation can match but secret differs).
pub(crate) fn should_write_back(
    gen_captured: u64,
    gen_now: u64,
    st_secret: &str,
    my_secret: &str,
) -> bool {
    gen_captured == gen_now && st_secret == my_secret
}

/// i18n payload for local `/health` timeout. Local health does not validate upstream keys.
/// Bind failures (EADDRINUSE) → port occupied; otherwise probe timeout (deps/script).
pub(crate) fn health_timeout_reason(port: u16, tail: &str) -> String {
    let occupied = tail.contains("Address already in use")
        || tail.contains("EADDRINUSE")
        || tail.contains("Errno 48") // macOS EADDRINUSE
        || tail.contains("Errno 98"); // Linux EADDRINUSE
    if occupied {
        i18n_err("errProxyPortOccupied", json!({ "port": port }))
    } else {
        i18n_err("errProxyHealthTimeout", json!({ "port": port }))
    }
}

/// Escape ERE metacharacters so a path can be matched literally by `pkill -f`.
pub(crate) fn ere_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if "\\.^$*+?()[]{}|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{ere_escape, health_timeout_reason, should_write_back};

    #[test]
    fn should_write_back_requires_both_gen_and_secret() {
        assert!(should_write_back(5, 5, "sekret", "sekret"));
        assert!(!should_write_back(5, 5, "other", "sekret"));
        assert!(!should_write_back(5, 6, "sekret", "sekret"));
        assert!(!should_write_back(5, 6, "other", "sekret"));
    }

    #[test]
    fn health_timeout_reason_flags_port_conflict_and_never_blames_key() {
        let occ = health_timeout_reason(18991, "OSError: [Errno 48] Address already in use");
        assert!(occ.contains("18991"));
        assert!(
            occ.contains("errProxyPortOccupied"),
            "should report port conflict: {occ}"
        );
        assert!(!occ.contains("key"), "port conflict must not mention key: {occ}");
        let generic = health_timeout_reason(18991, "ModuleNotFoundError: No module named 'x'");
        assert!(
            generic.contains("errProxyHealthTimeout"),
            "generic timeout must not blame key validity: {generic}"
        );
    }

    #[test]
    fn ere_escape_makes_path_literal_for_extended_regex() {
        assert_eq!(
            ere_escape("/tmp/a+b(proxy).py"),
            "/tmp/a\\+b\\(proxy\\)\\.py"
        );
    }
}
