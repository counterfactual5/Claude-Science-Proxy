//! Local config I/O: `~/.csp/CSP.json` (multi-profile, schema v4).
//!
//! Security (see `CLAUDE.md` iron rules):
//!   - dir `0700`, file `0600`; reject symlinks via `lstat` before read/write.
//!   - atomic write: temp file `O_CREAT|O_EXCL` + rename.
//!   - profile keys stored in plaintext (user-aware); never logged; API returns masked tail only.
//!
//! Migrations: v1 fixed slots → v2 profiles → v3 `active_models` → v4 `active_ids` (runtime: 0–1).
//! Backups: `CSP.json.v1.bak` on v1 migration; rolling `CSP.json.bak` before overwrite; rolling
//! backup sanitized after key clear / profile delete.
//!
//! All APIs take an explicit `dir` for tests; production uses [`default_dir`] (`$HOME/.csp`).

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::runtime::i18n::i18n_err;

pub(crate) fn default_proxy_port() -> u16 {
    18991
}
pub(crate) fn default_sandbox_port() -> u16 {
    8990
}
pub(crate) fn default_mode() -> String {
    "proxy".to_string()
}

pub(crate) fn validate_runtime_ports(proxy_port: u16, sandbox_port: u16) -> Result<(), String> {
    if proxy_port == 8765 || sandbox_port == 8765 {
        return Err(i18n_err("errPortReserved8765", json!({})));
    }
    if proxy_port == 0 || sandbox_port == 0 {
        return Err(i18n_err("errPortZero", json!({})));
    }
    if proxy_port == sandbox_port {
        return Err(i18n_err("errPortSame", json!({})));
    }
    Ok(())
}

/// Current config schema. Files with version >4 were written by a newer app — refuse to load.
pub const CURRENT_SCHEMA_VERSION: u32 = 4;

pub(crate) const CONFIG_BASENAME: &str = "CSP.json";
const MIGRATION_BACKUP_NAME: &str = "CSP.json.v1.bak";
const ROLLING_BACKUP_NAME: &str = "CSP.json.bak";

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// Named connection profile (cc-switch calls these providers). Keys on disk in plaintext; API masks them.
/// Runtime behavior comes from `template_id` via the templates registry (not from name/icon/base_url).
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub template_id: String,
    pub category: String,
    pub api_format: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    /// Models exposed to Science (multi-select). Migrated from lone `model` when empty.
    #[serde(default)]
    pub active_models: Vec<String>,
    /// Default / primary model for background agents. Falls back to first active_models entry.
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub icon_color: Option<String>,
    #[serde(default)]
    pub sort_index: Option<i64>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Top-level config. All fields have defaults so partial legacy files still deserialize.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Config {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// Active profile id (legacy mirror of first `active_ids` entry).
    #[serde(default)]
    pub active_id: String,
    /// Active profile ids (schema compat; runtime normalizes to 0 or 1 entry).
    #[serde(default)]
    pub active_ids: Vec<String>,
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default = "default_sandbox_port")]
    pub sandbox_port: u16,
    /// Persistent proxy path-secret (reused across restarts so sandbox URL stays valid).
    #[serde(default)]
    pub secret: String,
    /// Legacy official-mode field; always normalized to `proxy` on load.
    #[serde(default = "default_mode")]
    pub mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema_version: CURRENT_SCHEMA_VERSION,
            profiles: Vec::new(),
            active_id: String::new(),
            active_ids: Vec::new(),
            proxy_port: default_proxy_port(),
            sandbox_port: default_sandbox_port(),
            secret: String::new(),
            mode: default_mode(),
        }
    }
}

impl Profile {
    /// Effective model list: `active_models` first, else single `model`.
    pub fn effective_models(&self) -> Vec<String> {
        let from_active: Vec<String> = self
            .active_models
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !from_active.is_empty() {
            return from_active;
        }
        let m = self.model.trim();
        if m.is_empty() {
            Vec::new()
        } else {
            vec![m.to_string()]
        }
    }

    pub fn effective_default_model(&self) -> String {
        let d = self.default_model.trim();
        if !d.is_empty() {
            return d.to_string();
        }
        self.effective_models().first().cloned().unwrap_or_default()
    }

    /// Sort active models (newest first) and align default/model with the flagship entry.
    pub fn normalize_model_selection(&mut self) {
        let mut models = self.effective_models();
        if models.is_empty() {
            return;
        }
        crate::runtime::model_sort::sort_model_ids(&mut models);
        self.active_models = models.clone();
        let flagship = models[0].clone();
        self.default_model = flagship.clone();
        self.model = flagship;
    }

    /// Keep model / default_model / active_models consistent before save.
    pub fn sync_model_fields(&mut self) {
        let models = self.effective_models();
        if self.active_models.is_empty() && !models.is_empty() {
            self.active_models = models.clone();
        }
        let default = self.effective_default_model();
        if !default.is_empty() {
            self.default_model = default.clone();
            self.model = default;
        }
    }
}

impl Config {
    /// Whether `id` is the currently active profile.
    pub fn is_profile_active(&self, id: &str) -> bool {
        self.active_ids.iter().any(|x| x == id)
    }

    /// Remove `id` from active list (used when deleting a profile).
    pub fn deactivate_profile(&mut self, id: &str) {
        self.active_ids.retain(|x| x != id);
        self.sync_active_id();
    }

    /// Set exactly one active profile (single-active semantics).
    pub fn set_exclusive_active(&mut self, id: &str) {
        if self.profile_by_id(id).is_some() {
            self.active_ids = vec![id.to_string()];
            self.sync_active_id();
        }
    }

    /// Keep `active_id` in sync with first `active_ids` entry (API backward compat).
    pub fn sync_active_id(&mut self) {
        self.active_id = self.active_ids.first().cloned().unwrap_or_default();
    }

    /// Currently active profile (0 or 1 after normalization).
    pub fn active_profile(&self) -> Option<&Profile> {
        self.active_ids
            .first()
            .and_then(|id| self.profile_by_id(id))
    }
    pub fn profile_by_id(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
    pub fn profile_by_id_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }
}

/// 16 random bytes as 32 hex chars (`/dev/urandom`, else time-based fallback).
pub fn new_id() -> String {
    use std::io::Read;
    let mut buf = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return buf.iter().map(|b| format!("{b:02x}")).collect();
        }
    }
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{n:032x}")
}

/// Unix epoch milliseconds (created_at / sort_index).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- version detection ----------
#[derive(Debug, Clone, PartialEq)]
pub enum VersionKind {
    Legacy,
    V2,
    V3,
    V4,
    TooNew(u32),
}

#[derive(Deserialize)]
struct VersionProbe {
    #[serde(default)]
    schema_version: u32,
}

/// Map raw file bytes to version kind: <2 Legacy, 2 V2, 3 V3, 4 V4, >4 TooNew.
pub fn detect_version(data: &[u8]) -> io::Result<VersionKind> {
    let probe: VersionProbe = serde_json::from_slice(data).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            i18n_err("errCspJsonParse", json!({ "detail": e.to_string() })),
        )
    })?;
    Ok(match probe.schema_version {
        v if v < 2 => VersionKind::Legacy,
        2 => VersionKind::V2,
        3 => VersionKind::V3,
        v if v == CURRENT_SCHEMA_VERSION => VersionKind::V4,
        v => VersionKind::TooNew(v),
    })
}

/// Migrate legacy fixed slots to profile list. Skip empty slots; active_id from legacy provider pointer only.
pub fn migrate_v1_to_v2(mut legacy: crate::config_legacy::ConfigV1) -> Config {
    // Re-home bare relay slots to relay-<preset> templates first.
    crate::templates::migrate_legacy_relay(&mut legacy.providers, &mut legacy.provider);
    let ts = now_ms();
    let mut profiles = Vec::new();
    let mut active_id = String::new();
    for (i, (slot, pc)) in legacy.providers.iter().enumerate() {
        if pc.key.is_empty() && pc.base_url.is_empty() && pc.model.is_empty() {
            continue;
        }
        let tid = crate::templates::template_id_for_legacy_slot(slot);
        let tpl = crate::templates::by_id(tid);
        let id = new_id();
        let base_url = if pc.base_url.is_empty() {
            tpl.map(|t| t.base_url.to_string()).unwrap_or_default()
        } else {
            pc.base_url.clone()
        };
        profiles.push(Profile {
            id: id.clone(),
            name: tpl
                .map(|t| t.name.to_string())
                .unwrap_or_else(|| slot.clone()),
            template_id: tid.to_string(),
            category: tpl
                .map(|t| t.category.to_string())
                .unwrap_or_else(|| "custom".into()),
            api_format: tpl
                .map(|t| t.api_format.to_string())
                .unwrap_or_else(|| "anthropic".into()),
            base_url,
            api_key: pc.key.clone(),
            model: pc.model.clone(),
            active_models: if pc.model.trim().is_empty() {
                Vec::new()
            } else {
                vec![pc.model.clone()]
            },
            default_model: pc.model.clone(),
            website_url: tpl.map(|t| t.website_url.to_string()),
            icon: tpl.map(|t| t.icon.to_string()),
            icon_color: tpl.map(|t| t.icon_color.to_string()),
            sort_index: Some(i as i64),
            created_at: Some(ts),
            notes: None,
        });
        if *slot == legacy.provider {
            active_id = id;
        }
    }
    Config {
        schema_version: CURRENT_SCHEMA_VERSION,
        profiles,
        active_id: active_id.clone(),
        active_ids: if active_id.is_empty() {
            Vec::new()
        } else {
            vec![active_id]
        },
        proxy_port: legacy.proxy_port,
        sandbox_port: legacy.sandbox_port,
        secret: legacy.secret,
        mode: legacy.mode,
    }
}

/// Production config directory: `$HOME/.csp`.
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".csp")
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join(CONFIG_BASENAME)
}

/// Error if `path` exists and is a symlink (never follow). Missing path is Ok.
pub(crate) fn assert_not_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            i18n_err(
                "errSymlinkRejected",
                json!({ "path": path.display().to_string() }),
            ),
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Ensure config dir exists as a real directory with mode 0700 (reject symlinks).
pub fn ensure_dir(dir: &Path) -> io::Result<()> {
    assert_not_symlink(dir)?;
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    let md = fs::metadata(dir)?;
    if !md.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            i18n_err(
                "errConfigDirNotDir",
                json!({ "path": dir.display().to_string() }),
            ),
        ));
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

// ---------- backups ----------
/// Atomic copy src → dst (reject symlinks, mode 0600, O_EXCL temp + rename). Missing src → Err.
fn atomic_copy(src: &Path, dst: &Path) -> io::Result<()> {
    assert_not_symlink(dst)?;
    let data = fs::read(src)?; // missing src aborts migration backup
    let tmp = dst.with_extension(format!(
        "baktmp-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let write_res = (|| -> io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&data)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, dst) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::set_permissions(dst, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Pre-migration backup of CSP.json → CSP.json.v1.bak. Missing source or backup failure → Err.
pub fn write_migration_backup(dir: &Path) -> io::Result<()> {
    atomic_copy(&config_path(dir), &dir.join(MIGRATION_BACKUP_NAME))
}

/// Rolling backup before ordinary save → CSP.json.bak. Best-effort for callers; still atomic/0600.
pub fn write_rolling_backup(dir: &Path) -> io::Result<()> {
    atomic_copy(&config_path(dir), &dir.join(ROLLING_BACKUP_NAME))
}

/// Drop rolling backup after key clear / profile delete so old plaintext keys are not recoverable.
pub fn drop_rolling_backup(dir: &Path) {
    let _ = fs::remove_file(dir.join(ROLLING_BACKUP_NAME));
}

/// Load config from `dir/CSP.json`. Missing file → [`Config::default`].
/// Legacy schema migrates with v1.bak backup; schema too new → Err. Rejects symlink paths.
pub fn load_from(dir: &Path) -> io::Result<Config> {
    // Reject symlinked config dir (e.g. attacker swapping ~/.csp).
    assert_not_symlink(dir)?;
    let path = config_path(dir);
    assert_not_symlink(&path)?;
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e),
    };
    // Reset permissions on read (defense against external chmod widening).
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    match detect_version(&data)? {
        VersionKind::TooNew(v) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            i18n_err("errSchemaTooNew", json!({ "v": v })),
        )),
        VersionKind::Legacy => {
            write_migration_backup(dir)?; // backup failure aborts migration
            let legacy: crate::config_legacy::ConfigV1 =
                serde_json::from_slice(&data).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        i18n_err("errLegacyConfigParse", json!({ "detail": e.to_string() })),
                    )
                })?;
            let cfg = normalize_active(migrate_v1_to_v2(legacy));
            validate_loaded_ports(&cfg)?;
            save_to(dir, &cfg)?; // persist migrated schema (idempotent on next read)
            Ok(cfg)
        }
        VersionKind::V2 => {
            let cfg: Config = serde_json::from_slice(&data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    i18n_err("errCspJsonParse", json!({ "detail": e.to_string() })),
                )
            })?;
            let cfg = migrate_v3_to_v4(migrate_v2_to_v3(normalize_active(cfg)));
            validate_loaded_ports(&cfg)?;
            save_to(dir, &cfg)?;
            Ok(cfg)
        }
        VersionKind::V3 => {
            let cfg: Config = serde_json::from_slice(&data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    i18n_err("errCspJsonParse", json!({ "detail": e.to_string() })),
                )
            })?;
            let mut cfg = migrate_v3_to_v4(normalize_active(cfg));
            validate_loaded_ports(&cfg)?;
            save_to(dir, &cfg)?;
            Ok(cfg)
        }
        VersionKind::V4 => {
            let raw: Config = serde_json::from_slice(&data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    i18n_err("errCspJsonParse", json!({ "detail": e.to_string() })),
                )
            })?;
            let mode_migrated = raw.mode != "proxy";
            let before_active_len = raw.active_ids.len();
            let before_models: Vec<_> = raw
                .profiles
                .iter()
                .map(|p| {
                    (
                        p.active_models.clone(),
                        p.default_model.clone(),
                        p.model.clone(),
                    )
                })
                .collect();
            let mut cfg = normalize_active(raw);
            let models_normalized =
                cfg.profiles
                    .iter()
                    .zip(before_models.iter())
                    .any(|(p, (am, dm, m))| {
                        p.active_models != *am || p.default_model != *dm || p.model != *m
                    });
            validate_loaded_ports(&cfg)?;
            let folded_active = before_active_len > 1 && cfg.active_ids.len() <= 1;
            if mode_migrated || folded_active || models_normalized {
                save_to(dir, &cfg)?;
            }
            Ok(cfg)
        }
    }
}

fn validate_loaded_ports(cfg: &Config) -> io::Result<()> {
    validate_runtime_ports(cfg.proxy_port, cfg.sandbox_port)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Post-load invariants (spec §4): unknown template_id → `custom`; dangling active_* cleared;
/// legacy single active_id backfills active_ids; deprecated multi active_ids → first only;
/// legacy `mode: "official"` → `proxy`.
fn normalize_active(mut cfg: Config) -> Config {
    for p in cfg.profiles.iter_mut() {
        if crate::templates::by_id(&p.template_id).is_none() {
            p.template_id = "custom".to_string();
        }
        p.normalize_model_selection();
    }
    if cfg.active_ids.is_empty()
        && !cfg.active_id.is_empty()
        && cfg.profile_by_id(&cfg.active_id).is_some()
    {
        cfg.active_ids.push(cfg.active_id.clone());
    }
    cfg.active_ids = cfg
        .active_ids
        .iter()
        .filter(|id| cfg.profile_by_id(id).is_some())
        .cloned()
        .collect();
    if cfg.active_ids.len() > 1 {
        cfg.active_ids.truncate(1);
    }
    if !cfg.active_id.is_empty() && cfg.profile_by_id(&cfg.active_id).is_none() {
        cfg.active_id.clear();
    }
    cfg.sync_active_id();
    if cfg.mode != "proxy" {
        cfg.mode = default_mode();
    }
    cfg
}

/// v2 → v3: fill active_models/default_model and bump schema_version.
fn migrate_v2_to_v3(mut cfg: Config) -> Config {
    cfg.schema_version = 3;
    for p in cfg.profiles.iter_mut() {
        p.normalize_model_selection();
    }
    cfg
}

/// v3 → v4: active_id → active_ids and bump schema_version.
fn migrate_v3_to_v4(mut cfg: Config) -> Config {
    cfg.schema_version = CURRENT_SCHEMA_VERSION;
    if cfg.active_ids.is_empty()
        && !cfg.active_id.is_empty()
        && cfg.profile_by_id(&cfg.active_id).is_some()
    {
        cfg.active_ids = vec![cfg.active_id.clone()];
    }
    cfg.sync_active_id();
    cfg
}

/// Atomically write `dir/CSP.json` (mode 0600). Rejects symlinked dir or target file.
pub fn save_to(dir: &Path, cfg: &Config) -> io::Result<()> {
    ensure_dir(dir)?;
    let path = config_path(dir);
    assert_not_symlink(&path)?;
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            i18n_err("errConfigSerialize", json!({ "detail": e.to_string() })),
        )
    })?;

    // Temp file in same dir as target (atomic rename on one filesystem).
    // Name includes pid + thread id to avoid O_EXCL collisions under concurrent writers.
    let tmp = dir.join(format!(
        ".CSP.json.tmp-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    // O_CREAT|O_EXCL + 0600: never reuse an existing temp file; mode set at creation.
    let write_res = (|| -> io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // rename replaces the target name (not symlink follow); target already asserted non-link above.
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Serialized read-modify-write under a process-wide write lock so concurrent commands
/// cannot each load stale config and overwrite each other's field updates.
pub fn update<F: FnOnce(&mut Config)>(dir: &Path, f: F) -> io::Result<Config> {
    static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cfg = load_from(dir)?;
    f(&mut cfg);
    save_to(dir, &cfg)?;
    Ok(cfg)
}

/// Mask API keys as four dots plus last four chars (`••••tail`). Empty → ""; ≤4 chars fully masked.
/// Fixed width avoids horizontal overflow in WKWebView lists and does not leak key length.
pub fn mask(key: &str) -> String {
    let n = key.chars().count();
    if n == 0 {
        String::new()
    } else if n <= 4 {
        "•".repeat(n)
    } else {
        let last4: String = key.chars().skip(n - 4).collect();
        format!("••••{last4}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn tmpdir() -> PathBuf {
        // Per-test isolated subdir (process id + thread id) to avoid parallel test interference.
        let base = std::env::temp_dir().join(format!("csp-cfg-test-{}", std::process::id()));
        let d = base.join(format!("{:?}", std::thread::current().id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn mode_of(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    // ---------- A1: structure + accessors + new_id/now_ms ----------
    #[test]
    fn config_default_is_v4_empty() {
        let c = Config::default();
        assert_eq!(c.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(c.schema_version, 4);
        assert!(c.profiles.is_empty());
        assert_eq!(c.active_id, "");
        assert!(c.active_ids.is_empty());
        assert_eq!(c.proxy_port, 18991);
        assert_eq!(c.mode, "proxy");
    }

    #[test]
    fn load_from_migrates_legacy_official_mode_to_proxy() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            config_path(&d),
            br#"{"schema_version":4,"profiles":[],"active_id":"","active_ids":[],"proxy_port":18991,"sandbox_port":8990,"mode":"official"}"#,
        )
        .unwrap();
        let cfg = load_from(&d).unwrap();
        assert_eq!(cfg.mode, "proxy");
        let on_disk: Config = serde_json::from_slice(&fs::read(config_path(&d)).unwrap()).unwrap();
        assert_eq!(on_disk.mode, "proxy");
    }

    #[test]
    fn profile_accessors_by_id_and_active() {
        let p = Profile {
            id: "abc".into(),
            name: "DS".into(),
            template_id: "deepseek".into(),
            category: "cn_official".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_key: "sk-1".into(),
            model: String::new(),
            ..Default::default()
        };
        let c = Config {
            profiles: vec![p.clone()],
            active_ids: vec!["abc".into()],
            ..Default::default()
        };
        assert_eq!(c.profile_by_id("abc").unwrap().name, "DS");
        assert!(c.profile_by_id("nope").is_none());
        assert_eq!(c.active_profile().unwrap().id, "abc");
        assert!(c.is_profile_active("abc"));
        let c2 = Config {
            active_ids: vec![],
            ..c.clone()
        };
        assert!(c2.active_profile().is_none());
    }

    #[test]
    fn normalize_active_folds_multiple_active_ids_to_first() {
        let mut c = Config {
            profiles: vec![
                Profile {
                    id: "a".into(),
                    ..Default::default()
                },
                Profile {
                    id: "b".into(),
                    ..Default::default()
                },
            ],
            active_ids: vec!["a".into(), "b".into()],
            active_id: "a".into(),
            ..Default::default()
        };
        c = normalize_active(c);
        assert_eq!(c.active_ids, vec!["a"]);
        assert_eq!(c.active_id, "a");
    }

    #[test]
    fn deactivate_and_exclusive_active() {
        let mut c = Config {
            profiles: vec![
                Profile {
                    id: "a".into(),
                    ..Default::default()
                },
                Profile {
                    id: "b".into(),
                    ..Default::default()
                },
            ],
            active_ids: vec!["a".into(), "b".into()],
            active_id: "a".into(),
            ..Default::default()
        };
        c.deactivate_profile("a");
        assert_eq!(c.active_ids, vec!["b"]);
        c.set_exclusive_active("a");
        assert_eq!(c.active_ids, vec!["a"]);
    }

    #[test]
    fn migrate_v3_to_v4_populates_active_ids() {
        let d = tmpdir().join(".csp-v3");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            config_path(&d),
            br#"{"schema_version":3,"profiles":[{"id":"p1","name":"X","template_id":"glm","category":"relay","api_format":"anthropic","base_url":"https://x/y"}],"active_id":"p1"}"#,
        )
        .unwrap();
        let cfg = load_from(&d).unwrap();
        assert_eq!(cfg.schema_version, 4);
        assert_eq!(cfg.active_ids, vec!["p1"]);
        assert_eq!(cfg.active_id, "p1");
    }

    #[test]
    fn new_id_is_unique_hex_and_now_ms_positive() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert!(now_ms() > 0);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let d = tmpdir().join(".csp");
        let p = Profile {
            id: "id1".into(),
            name: "DeepSeek".into(),
            template_id: "deepseek".into(),
            category: "cn_official".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_key: "sk-abcdef1234".into(),
            model: String::new(),
            ..Default::default()
        };
        let cfg = Config {
            profiles: vec![p],
            active_id: "id1".into(),
            active_ids: vec!["id1".into()],
            proxy_port: 12345,
            ..Default::default()
        };
        save_to(&d, &cfg).unwrap();
        let got = load_from(&d).unwrap();
        assert_eq!(got, cfg);
        assert_eq!(got.active_profile().unwrap().api_key, "sk-abcdef1234");
    }

    #[test]
    fn load_rejects_invalid_runtime_ports() {
        let cases = [
            ("proxy_8765", 8765, 8990, "errPortReserved8765"),
            ("sandbox_8765", 18991, 8765, "errPortReserved8765"),
            ("proxy_zero", 0, 8990, "errPortZero"),
            ("sandbox_zero", 18991, 0, "errPortZero"),
            ("same_ports", 18991, 18991, "errPortSame"),
        ];
        for (name, proxy_port, sandbox_port, i18n_key) in cases {
            let d = tmpdir().join(format!(".csp-{name}"));
            fs::create_dir_all(&d).unwrap();
            fs::write(
                config_path(&d),
                format!(
                    r#"{{"schema_version":2,"profiles":[],"active_id":"","proxy_port":{proxy_port},"sandbox_port":{sandbox_port}}}"#
                ),
            )
            .unwrap();
            let err = load_from(&d).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "{name}");
            assert!(
                err.to_string().contains(i18n_key),
                "error should contain i18n key {i18n_key} for {name}: {err}"
            );
        }
    }

    #[test]
    fn load_rejects_legacy_invalid_ports_before_v2_save() {
        let d = tmpdir().join(".csp-legacy-bad-port");
        fs::create_dir_all(&d).unwrap();
        let legacy = r#"{
            "provider":"deepseek",
            "proxy_port":18991,
            "sandbox_port":8765,
            "secret":"sec",
            "mode":"proxy",
            "providers":{"deepseek":{"key":"sk-ds","base_url":"","model":""}}
        }"#;
        fs::write(config_path(&d), legacy).unwrap();
        let err = load_from(&d).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let after = fs::read_to_string(config_path(&d)).unwrap();
        assert!(
            !after.contains("\"schema_version\""),
            "invalid legacy config should not be saved as v2: {after}"
        );
        assert!(d.join("CSP.json.v1.bak").is_file());
    }

    // ---------- A2: version detection ----------
    #[test]
    fn detect_version_missing_field_is_legacy() {
        let d = br#"{"provider":"deepseek","providers":{}}"#;
        assert!(matches!(detect_version(d).unwrap(), VersionKind::Legacy));
    }
    #[test]
    fn detect_version_two_is_v2() {
        let d = br#"{"schema_version":2,"profiles":[],"active_id":""}"#;
        assert!(matches!(detect_version(d).unwrap(), VersionKind::V2));
    }
    #[test]
    fn detect_version_three_is_v3() {
        let d = br#"{"schema_version":3}"#;
        assert!(matches!(detect_version(d).unwrap(), VersionKind::V3));
    }

    #[test]
    fn detect_version_four_is_v4() {
        let d = br#"{"schema_version":4}"#;
        assert!(matches!(detect_version(d).unwrap(), VersionKind::V4));
    }

    #[test]
    fn detect_version_five_is_too_new() {
        let d = br#"{"schema_version":5}"#;
        assert!(matches!(detect_version(d).unwrap(), VersionKind::TooNew(5)));
    }
    #[test]
    fn detect_version_garbage_errors() {
        assert!(detect_version(b"not json").is_err());
    }

    // ---------- A4: migration v1 → v2 ----------
    #[test]
    fn migrate_maps_slots_to_profiles_and_active() {
        use crate::config_legacy::{ConfigV1, ProviderCfgV1};
        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "deepseek".to_string(),
            ProviderCfgV1 {
                key: "sk-ds".into(),
                base_url: "".into(),
                model: "".into(),
            },
        );
        providers.insert(
            "relay-glm".to_string(),
            ProviderCfgV1 {
                key: "glmk".into(),
                base_url: "https://open.bigmodel.cn/api/anthropic".into(),
                model: "glm-5".into(),
            },
        );
        providers.insert(
            "qwen".to_string(),
            ProviderCfgV1 {
                key: "".into(),
                base_url: "".into(),
                model: "".into(),
            },
        ); // empty slot
        let legacy = ConfigV1 {
            provider: "relay-glm".into(),
            proxy_port: 18991,
            sandbox_port: 8990,
            secret: "sec".into(),
            mode: "proxy".into(),
            providers,
        };
        let cfg = migrate_v1_to_v2(legacy);
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        let glm = cfg
            .profiles
            .iter()
            .find(|p| p.template_id == "glm")
            .unwrap();
        assert_eq!(glm.api_key, "glmk");
        assert_eq!(glm.base_url, "https://open.bigmodel.cn/api/anthropic");
        assert_eq!(glm.model, "glm-5");
        assert_eq!(glm.api_format, "anthropic");
        assert_eq!(
            cfg.active_id, glm.id,
            "legacy provider=relay-glm → active points at that profile"
        );
        assert_eq!(cfg.secret, "sec");
    }

    #[test]
    fn migrate_invalid_active_yields_empty() {
        use crate::config_legacy::{ConfigV1, ProviderCfgV1};
        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "deepseek".to_string(),
            ProviderCfgV1 {
                key: "k".into(),
                base_url: "".into(),
                model: "".into(),
            },
        );
        // Legacy provider points at empty/missing slot → active_id must be empty (no silent first-profile pick).
        let legacy = ConfigV1 {
            provider: "qwen".into(),
            proxy_port: 18991,
            sandbox_port: 8990,
            secret: "".into(),
            mode: "proxy".into(),
            providers,
        };
        let cfg = migrate_v1_to_v2(legacy);
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(
            cfg.active_id, "",
            "invalid active → empty, wait for user choice"
        );
    }

    #[test]
    fn migrate_legacy_bare_relay_slot() {
        use crate::config_legacy::{ConfigV1, ProviderCfgV1};
        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "relay".to_string(),
            ProviderCfgV1 {
                key: "rk".into(),
                base_url: "https://open.bigmodel.cn/api/anthropic".into(),
                model: "".into(),
            },
        );
        let legacy = ConfigV1 {
            provider: "relay".into(),
            proxy_port: 18991,
            sandbox_port: 8990,
            secret: "".into(),
            mode: "proxy".into(),
            providers,
        };
        let cfg = migrate_v1_to_v2(legacy);
        let glm = cfg
            .profiles
            .iter()
            .find(|p| p.template_id == "glm")
            .unwrap();
        assert_eq!(glm.api_key, "rk");
        assert_eq!(cfg.active_id, glm.id);
    }

    // ---------- A5: backup infrastructure ----------
    #[test]
    fn migration_backup_copies_and_is_0600() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), b"OLD-V1-BYTES").unwrap();
        write_migration_backup(&d).unwrap();
        let bak = d.join("CSP.json.v1.bak");
        assert_eq!(fs::read(&bak).unwrap(), b"OLD-V1-BYTES");
        assert_eq!(mode_of(&bak), 0o600);
    }
    #[test]
    fn migration_backup_missing_source_errors() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        assert!(write_migration_backup(&d).is_err());
    }
    #[test]
    fn rolling_backup_then_drop_removes_key_recoverability() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), br#"{"api_key":"sk-SECRET-TAIL"}"#).unwrap();
        write_rolling_backup(&d).unwrap();
        let bak = d.join("CSP.json.bak");
        assert!(fs::read_to_string(&bak).unwrap().contains("sk-SECRET-TAIL"));
        drop_rolling_backup(&d);
        assert!(
            !bak.exists(),
            "after sanitization rolling backup removed; cleared key not recoverable from .bak"
        );
    }
    #[test]
    fn backup_rejects_symlinked_target() {
        let base = tmpdir();
        let d = base.join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), b"X").unwrap();
        let elsewhere = base.join("elsewhere");
        fs::write(&elsewhere, b"ORIG").unwrap();
        symlink(&elsewhere, d.join("CSP.json.v1.bak")).unwrap();
        assert!(write_migration_backup(&d).is_err());
        assert_eq!(fs::read(&elsewhere).unwrap(), b"ORIG");
    }

    // ---------- A6: load_from integration ----------
    #[test]
    fn load_migrates_old_file_and_writes_v1_bak() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            config_path(&d),
            br#"{"provider":"deepseek","providers":{"deepseek":{"key":"sk-x"}}}"#,
        )
        .unwrap();
        let cfg = load_from(&d).unwrap();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.active_profile().unwrap().api_key, "sk-x");
        assert!(
            d.join("CSP.json.v1.bak").exists(),
            "migration must leave v1 backup"
        );
        // After persist, reload is v4 (idempotent, no re-migration).
        let again = load_from(&d).unwrap();
        assert_eq!(again, cfg);
        assert_eq!(again.schema_version, CURRENT_SCHEMA_VERSION);
    }
    #[test]
    fn load_too_new_errors() {
        let d = tmpdir().join(".csp");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), br#"{"schema_version":9,"profiles":[]}"#).unwrap();
        let e = load_from(&d).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        assert!(e.to_string().contains("errSchemaTooNew"));
    }
    #[test]
    fn load_normalizes_dangling_active() {
        let d = tmpdir().join(".csp");
        let cfg = Config {
            active_id: "ghost".into(),
            profiles: vec![Profile {
                id: "real".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        save_to(&d, &cfg).unwrap();
        let got = load_from(&d).unwrap();
        assert_eq!(
            got.active_ids,
            vec![] as Vec<String>,
            "dangling active → normalized to empty"
        );
        assert_eq!(got.active_id, "");
    }

    // ---------- MP-2 Minor [2]: unknown template_id → normalize to custom ----------
    #[test]
    fn load_normalizes_unknown_template_id_to_custom() {
        let d = tmpdir().join(".csp");
        // Build v2 profile with template_id not in registry (connection fields preserved).
        let cfg = Config {
            active_id: "p1".into(),
            profiles: vec![Profile {
                id: "p1".into(),
                name: "野模板".into(),
                template_id: "totally-unknown-xyz".into(),
                api_format: "anthropic".into(),
                base_url: "https://relay.example/claude".into(),
                api_key: "sk-x".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        save_to(&d, &cfg).unwrap();
        let got = load_from(&d).unwrap();
        let p = got.profile_by_id("p1").unwrap();
        assert_eq!(
            p.template_id, "custom",
            "unknown template_id → normalize to custom"
        );
        assert_eq!(
            p.base_url, "https://relay.example/claude",
            "connection fields preserved"
        );
        assert_eq!(p.api_key, "sk-x");
        assert_eq!(got.active_ids, vec!["p1"]);
        assert_eq!(got.active_id, "p1", "active still valid, not cleared");
    }

    // ---------- Existing security/permission invariants (retained) ----------
    #[test]
    fn load_missing_returns_default() {
        let d = tmpdir().join(".csp");
        let cfg = load_from(&d).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg.proxy_port, 18991);
    }

    #[test]
    fn save_sets_dir_0700_and_file_0600() {
        let d = tmpdir().join(".csp");
        save_to(&d, &Config::default()).unwrap();
        assert_eq!(mode_of(&d), 0o700, "dir must be 0700");
        assert_eq!(mode_of(&config_path(&d)), 0o600, "file must be 0600");
    }

    #[test]
    fn load_resets_widened_perms_to_0600() {
        let d = tmpdir().join(".csp");
        save_to(&d, &Config::default()).unwrap();
        let p = config_path(&d);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
        load_from(&d).unwrap();
        assert_eq!(mode_of(&p), 0o600, "load must reset perms to 0600");
    }

    #[test]
    fn save_rejects_symlinked_file_and_leaves_target_untouched() {
        let base = tmpdir();
        let d = base.join(".csp");
        fs::create_dir_all(&d).unwrap();
        let target = base.join("real-elsewhere.txt");
        fs::write(&target, b"ORIGINAL").unwrap();
        symlink(&target, config_path(&d)).unwrap();
        let err = save_to(&d, &Config::default()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&target).unwrap(), b"ORIGINAL");
    }

    #[test]
    fn load_rejects_symlinked_file() {
        let base = tmpdir();
        let d = base.join(".csp");
        fs::create_dir_all(&d).unwrap();
        let target = base.join("secret.txt");
        fs::write(&target, b"{\"schema_version\":2}").unwrap();
        symlink(&target, config_path(&d)).unwrap();
        let err = load_from(&d).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn load_rejects_symlinked_dir() {
        let base = tmpdir();
        let realdir = base.join("realdir");
        fs::create_dir_all(&realdir).unwrap();
        fs::write(realdir.join("CSP.json"), b"{\"schema_version\":2}").unwrap();
        let link = base.join(".csp");
        symlink(&realdir, &link).unwrap();
        let err = load_from(&link).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn ensure_dir_rejects_symlinked_dir() {
        let base = tmpdir();
        let realdir = base.join("realdir");
        fs::create_dir_all(&realdir).unwrap();
        let link = base.join(".csp");
        symlink(&realdir, &link).unwrap();
        let err = save_to(&link, &Config::default()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn no_tmp_file_left_after_save() {
        let d = tmpdir().join(".csp");
        save_to(&d, &Config::default()).unwrap();
        let leftovers: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".CSP.json.tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp file should be renamed away");
    }

    #[test]
    fn update_applies_and_persists() {
        let d = tmpdir().join(".csp");
        save_to(&d, &Config::default()).unwrap();
        update(&d, |c| {
            c.profiles.push(Profile {
                id: "id1".into(),
                name: "Q".into(),
                template_id: "qwen".into(),
                ..Default::default()
            });
            c.set_exclusive_active("id1");
        })
        .unwrap();
        let got = load_from(&d).unwrap();
        assert_eq!(got.active_ids, vec!["id1"]);
        assert_eq!(got.active_id, "id1");
        assert_eq!(got.active_profile().unwrap().name, "Q");
    }

    #[test]
    fn secret_persists_and_survives_reload() {
        // path-secret must persist once generated; proxy restart/reopen app keeps same value.
        let d = tmpdir().join(".csp");
        save_to(&d, &Config::default()).unwrap();
        assert!(
            load_from(&d).unwrap().secret.is_empty(),
            "initial should be empty"
        );
        update(&d, |c| c.secret = "deadbeef00112233".into()).unwrap();
        assert_eq!(load_from(&d).unwrap().secret, "deadbeef00112233");
        // Changing other fields must not affect secret.
        update(&d, |c| c.proxy_port = 20000).unwrap();
        assert_eq!(load_from(&d).unwrap().secret, "deadbeef00112233");
    }

    #[test]
    fn mask_hides_all_but_last4() {
        assert_eq!(mask("sk-1234567890ab"), "••••90ab"); // fixed 4 dots + last 4
        assert_eq!(mask(""), "");
        assert_eq!(mask("abc"), "•••");
        assert_eq!(mask("abcd"), "••••");
        assert_eq!(mask("abcde"), "••••bcde"); // fixed 4 dots + last 4
        let full = "sk-secret-tail9999";
        assert!(!mask(full).contains("secret"));
        // Fixed width: masked output always 8 chars (4 dots + last 4); no length leak
        assert_eq!(
            mask("sk-aaaaaaaaaaaaaaaaaaaaaaaaaaaa1234").chars().count(),
            8
        );
    }
}
