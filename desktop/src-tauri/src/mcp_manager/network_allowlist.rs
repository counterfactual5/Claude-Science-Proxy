//! Science sandbox network allowlist for CSP-managed MCP egress.
//!
//! Claude Science's Operon proxy only CONNECTs to granted hosts. Built-in
//! scholarly APIs are already on Science's baseline list; general-web and
//! key-based search providers used by the bundled `web-search` MCP are not.
//!
//! On each MCP deploy / Start Claude Science, CSP merges:
//! 1. **Built-in** domains for every `web-search` provider (DDG, Wikipedia,
//!    Brave, Serper, Tavily) — so configuring an API key works without a
//!    separate manual grant.
//! 2. **User extensions** from `~/.csp/network-allowlist.json`.
//!
//! into the active org's `preferences.json`
//! (`userAllowedDomains` + `approvalGrants.always.allow.network` +
//! `alwaysOrigins.network`). A restart is required for Operon to pick up
//! new grants (disk edits alone do not hot-reload).

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config;

const ALLOWLIST_FILE: &str = "network-allowlist.json";
const ACTIVE_ORG_FILE: &str = "active-org.json";
const PREFERENCES_FILE: &str = "preferences.json";

/// Hosts required by the bundled `web-search` MCP providers (not already on
/// Science's scholarly baseline). Keep in sync with `web_search_server.py`.
pub const WEB_SEARCH_PROVIDER_DOMAINS: &[&str] = &[
    // DuckDuckGo
    "html.duckduckgo.com",
    "api.duckduckgo.com",
    // Wikipedia (MediaWiki API)
    "en.wikipedia.org",
    // Key-based general search (usable once the user sets env keys)
    "api.search.brave.com",
    "google.serper.dev",
    "api.tavily.com",
];

const CSP_ORIGIN_USER: &str = "csp-network-allowlist";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAllowlistFile {
    pub version: u32,
    /// Extra domains the user wants Science to grant (hostnames only).
    #[serde(default)]
    pub domains: Vec<String>,
    /// Human-readable note preserved across ensure/seed writes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Default for NetworkAllowlistFile {
    fn default() -> Self {
        Self {
            version: 1,
            domains: Vec::new(),
            description: "Extra Science sandbox egress domains (hostnames only). \
Merged with built-in web-search provider hosts on each Start Claude Science. \
Edit this file, then Stop → Start Claude Science for Operon to reload grants."
                .to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyAllowlistResult {
    pub applied: Vec<String>,
    pub added: Vec<String>,
    pub preferences_path: Option<PathBuf>,
    pub changed: bool,
}

#[derive(Deserialize)]
struct ActiveOrg {
    org_uuid: String,
}

/// Absolute path to `~/.csp/network-allowlist.json`.
pub fn allowlist_path() -> PathBuf {
    config::default_dir().join(ALLOWLIST_FILE)
}

/// Ensure the user extension file exists (creates a default empty list).
pub fn ensure_user_file() -> Result<PathBuf, String> {
    let path = allowlist_path();
    if path.is_file() {
        return Ok(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create ~/.csp: {e}"))?;
    }
    let body = serde_json::to_vec_pretty(&NetworkAllowlistFile::default())
        .map_err(|e| format!("serialize default allowlist: {e}"))?;
    write_0600(&path, &body)?;
    Ok(path)
}

/// Load user domains from disk (missing file → empty). Invalid entries skipped.
pub fn load_user_domains() -> Result<Vec<String>, String> {
    let path = allowlist_path();
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("read allowlist: {e}"))?;
    let parsed: NetworkAllowlistFile =
        serde_json::from_str(&text).map_err(|e| format!("parse allowlist: {e}"))?;
    Ok(normalize_domains(&parsed.domains))
}

/// Built-in + user domains, de-duplicated, sorted.
pub fn merged_domains() -> Result<Vec<String>, String> {
    let mut set: BTreeSet<String> = WEB_SEARCH_PROVIDER_DOMAINS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for d in load_user_domains()? {
        set.insert(d);
    }
    Ok(set.into_iter().collect())
}

/// Merge desired domains into the active org's Science preferences.
///
/// Returns `changed=true` when the preferences file was rewritten (caller
/// should treat this like other deploy changes that need a Science restart).
pub fn apply_to_active_org(auth_dir: &Path) -> Result<ApplyAllowlistResult, String> {
    let _ = ensure_user_file();
    let desired = merged_domains()?;
    let Some(org_uuid) = read_org_uuid(auth_dir) else {
        return Ok(ApplyAllowlistResult {
            applied: desired,
            added: Vec::new(),
            preferences_path: None,
            changed: false,
        });
    };
    let prefs_path = auth_dir
        .join("orgs")
        .join(&org_uuid)
        .join(PREFERENCES_FILE);
    if !prefs_path.is_file() {
        // Org not initialized yet — skip; next Start after first login will apply.
        return Ok(ApplyAllowlistResult {
            applied: desired,
            added: Vec::new(),
            preferences_path: Some(prefs_path),
            changed: false,
        });
    }

    let raw = fs::read_to_string(&prefs_path).map_err(|e| format!("read preferences: {e}"))?;
    let mut prefs: Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse preferences: {e}"))?;

    let before = prefs.clone();
    let added = merge_domains_into_preferences(&mut prefs, &desired)?;
    if prefs == before {
        return Ok(ApplyAllowlistResult {
            applied: desired,
            added: Vec::new(),
            preferences_path: Some(prefs_path),
            changed: false,
        });
    }

    let body = serde_json::to_vec_pretty(&prefs).map_err(|e| format!("serialize preferences: {e}"))?;
    // Preserve mode if present; otherwise 0600.
    let mode = fs::metadata(&prefs_path)
        .map(|m| m.permissions().mode())
        .unwrap_or(0o600);
    fs::write(&prefs_path, &body).map_err(|e| format!("write preferences: {e}"))?;
    let _ = fs::set_permissions(&prefs_path, fs::Permissions::from_mode(mode | 0o600));

    Ok(ApplyAllowlistResult {
        applied: desired,
        added,
        preferences_path: Some(prefs_path),
        changed: true,
    })
}

/// Best-effort apply used from sandbox deploy (never fails the launch).
pub fn apply_best_effort(auth_dir: &Path) -> (String, bool) {
    match apply_to_active_org(auth_dir) {
        Ok(r) => {
            if r.changed {
                (
                    format!(
                        "network-allowlist: added {:?} (total {}) → restart needed for Operon",
                        r.added,
                        r.applied.len()
                    ),
                    true,
                )
            } else {
                (
                    format!(
                        "network-allowlist: up-to-date ({} domain(s))",
                        r.applied.len()
                    ),
                    false,
                )
            }
        }
        Err(e) => (format!("network-allowlist: skipped ({e})"), false),
    }
}

fn merge_domains_into_preferences(
    prefs: &mut Value,
    domains: &[String],
) -> Result<Vec<String>, String> {
    let obj = prefs
        .as_object_mut()
        .ok_or_else(|| "preferences.json root must be an object".to_string())?;

    // userAllowedDomains
    let mut user_domains = json_string_array(obj.get("userAllowedDomains"));
    let mut added = Vec::new();
    for d in domains {
        if !user_domains.iter().any(|x| x == d) {
            user_domains.push(d.clone());
            added.push(d.clone());
        }
    }
    user_domains.sort();
    user_domains.dedup();
    obj.insert("userAllowedDomains".into(), json!(user_domains));

    // approvalGrants.always.allow.network
    let grants = obj
        .entry("approvalGrants")
        .or_insert_with(|| json!({}));
    let grants_obj = grants
        .as_object_mut()
        .ok_or_else(|| "approvalGrants must be an object".to_string())?;
    let always = grants_obj
        .entry("always")
        .or_insert_with(|| json!({}));
    let always_obj = always
        .as_object_mut()
        .ok_or_else(|| "approvalGrants.always must be an object".to_string())?;
    let allow = always_obj
        .entry("allow")
        .or_insert_with(|| json!({}));
    let allow_obj = allow
        .as_object_mut()
        .ok_or_else(|| "approvalGrants.always.allow must be an object".to_string())?;
    let mut network = json_string_array(allow_obj.get("network"));
    for d in domains {
        if !network.iter().any(|x| x == d) {
            network.push(d.clone());
            if !added.iter().any(|x| x == d) {
                added.push(d.clone());
            }
        }
    }
    network.sort();
    network.dedup();
    allow_obj.insert("network".into(), json!(network));

    // alwaysOrigins.network stubs (Science merges these for grant provenance)
    let origins = always_obj
        .entry("alwaysOrigins")
        .or_insert_with(|| json!({}));
    let origins_obj = origins
        .as_object_mut()
        .ok_or_else(|| "alwaysOrigins must be an object".to_string())?;
    let net_origins = origins_obj
        .entry("network")
        .or_insert_with(|| json!({}));
    let net_origins_obj = net_origins
        .as_object_mut()
        .ok_or_else(|| "alwaysOrigins.network must be an object".to_string())?;
    for d in domains {
        if !net_origins_obj.contains_key(d) {
            net_origins_obj.insert(
                d.clone(),
                json!({
                    "userId": CSP_ORIGIN_USER,
                    "rootFrameId": CSP_ORIGIN_USER,
                    "projectId": CSP_ORIGIN_USER,
                }),
            );
        }
    }

    added.sort();
    added.dedup();
    Ok(added)
}

fn json_string_array(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .filter(|s| valid_hostname(s))
            .collect(),
        _ => Vec::new(),
    }
}

fn normalize_domains(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = raw
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| valid_hostname(s))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Hostname only: labels, dots, optional leading `*.` not allowed (keep simple).
fn valid_hostname(s: &str) -> bool {
    if s.is_empty() || s.len() > 253 {
        return false;
    }
    if s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return false;
    }
    if s.contains('/') || s.contains('\\') || s.contains(':') || s.contains(' ') {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn read_org_uuid(auth_dir: &Path) -> Option<String> {
    let v: ActiveOrg =
        serde_json::from_str(&fs::read_to_string(auth_dir.join(ACTIVE_ORG_FILE)).ok()?).ok()?;
    let org = v.org_uuid;
    if org.len() == 36 && org.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        Some(org)
    } else {
        None
    }
}

fn write_0600(path: &Path, body: &[u8]) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    f.write_all(body)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    f.sync_all()
        .map_err(|e| format!("sync {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn tmp_dir(tag: &str) -> PathBuf {
        let p = env::temp_dir().join(format!(
            "csp-allowlist-{}-{}",
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn valid_hostname_accepts_api_hosts() {
        assert!(valid_hostname("api.search.brave.com"));
        assert!(valid_hostname("html.duckduckgo.com"));
        assert!(!valid_hostname("https://evil.com"));
        assert!(!valid_hostname("evil.com/path"));
        assert!(!valid_hostname(""));
    }

    #[test]
    fn merge_adds_missing_domains_idempotently() {
        let mut prefs = json!({
            "userAllowedDomains": ["api.coingecko.com"],
            "approvalGrants": {
                "always": {
                    "allow": {
                        "network": ["api.coingecko.com"]
                    },
                    "alwaysOrigins": {
                        "network": {
                            "api.coingecko.com": {
                                "userId": "local-dev",
                                "rootFrameId": "x",
                                "projectId": "y"
                            }
                        }
                    }
                }
            }
        });
        let domains = vec![
            "api.coingecko.com".into(),
            "api.duckduckgo.com".into(),
            "api.search.brave.com".into(),
        ];
        let added = merge_domains_into_preferences(&mut prefs, &domains).unwrap();
        assert_eq!(
            added,
            vec![
                "api.duckduckgo.com".to_string(),
                "api.search.brave.com".to_string()
            ]
        );
        let user: Vec<String> = prefs["userAllowedDomains"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert!(user.contains(&"api.coingecko.com".to_string()));
        assert!(user.contains(&"api.duckduckgo.com".to_string()));
        assert!(user.contains(&"api.search.brave.com".to_string()));

        let added2 = merge_domains_into_preferences(&mut prefs, &domains).unwrap();
        assert!(added2.is_empty());
    }

    #[test]
    fn apply_to_active_org_writes_preferences() {
        let root = tmp_dir("apply");
        let org = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let auth = root.join(".claude-science");
        let org_dir = auth.join("orgs").join(org);
        fs::create_dir_all(&org_dir).unwrap();
        fs::write(
            auth.join("active-org.json"),
            format!(r#"{{"org_uuid":"{org}"}}"#),
        )
        .unwrap();
        fs::write(
            org_dir.join("preferences.json"),
            r#"{"userAllowedDomains":[],"approvalGrants":{"always":{"allow":{"network":[]},"alwaysOrigins":{"network":{}}}}}"#,
        )
        .unwrap();

        // Point WEB_SEARCH domains through apply with a stub auth_dir only —
        // call merge via apply_to_active_org which also loads user file from
        // real ~/.csp; isolate by only testing merge helper above for unit
        // purity. Here we exercise apply with org path using public API after
        // temporarily relying on merged_domains (includes builtins).
        let result = apply_to_active_org(&auth).unwrap();
        assert!(result.changed);
        assert!(result.added.contains(&"api.duckduckgo.com".to_string()));
        assert!(result.added.contains(&"api.search.brave.com".to_string()));
        let text = fs::read_to_string(org_dir.join("preferences.json")).unwrap();
        assert!(text.contains("api.search.brave.com"));
        assert!(text.contains("en.wikipedia.org"));

        let result2 = apply_to_active_org(&auth).unwrap();
        assert!(!result2.changed);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn web_search_provider_domains_cover_key_vendors() {
        let joined = WEB_SEARCH_PROVIDER_DOMAINS.join(" ");
        assert!(joined.contains("duckduckgo"));
        assert!(joined.contains("brave"));
        assert!(joined.contains("serper"));
        assert!(joined.contains("tavily"));
        assert!(joined.contains("wikipedia"));
    }
}
