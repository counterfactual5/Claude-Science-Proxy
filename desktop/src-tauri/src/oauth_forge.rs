//! Virtual OAuth forger (Rust-native). Writes **sandbox-only** fake login material so Claude Science
//! can start without touching real `~/.claude-science`. Zero network, zero real credentials.
//! Wire format matches `scripts/make-virtual-oauth.mjs` — see `docs/verified-facts.md`.
//!
//! Sandbox `auth_dir` layout (byte-compatible with the Node forger; tests lock the format):
//!   - Token file: `<auth_dir>/.oauth-tokens/<sanitized account_uuid>.enc` (exactly one `.enc`)
//!   - v2 payload: `"v2:" + base64( IV(12) ‖ AES-256-GCM(ciphertext) ‖ authTag(16) )`
//!     derivedKey = HKDF-SHA256(ikm=base64_decode(OAUTH_ENCRYPTION_KEY), salt=empty,
//!     info="operon:aes-256-gcm:oauth", 32); AAD = "v2:oauth"; plaintext = JSON blob
//!   - `encryption.key`: newline-separated KEY=base64(≥16B); far-future expiry avoids refresh
//!   - `active-org.json`: `{ "org_uuid": <uuid> }` (Science only checks UUID shape)
//!
//! Iron rules: **never write the real credential tree** (`~/.claude-science`); fake email must use
//! `localhost.invalid`; reject symlinks before write; O_EXCL temp file + rename + mode 0600.
//! See `CLAUDE.md` for the full safety contract.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use hkdf::Hkdf;
use serde_json::json;
use sha2::Sha256;

use crate::runtime::i18n::i18n_err;

const KEY_NAMES: [&str; 4] = [
    "ANTHROPIC_API_KEY_ENCRYPTION_KEY",
    "OAUTH_ENCRYPTION_KEY",
    "JWT_SIGNING_SECRET",
    "USER_SECRET_ENCRYPTION_KEY",
];
const HKDF_INFO: &[u8] = b"operon:aes-256-gcm:oauth";
const AAD: &[u8] = b"v2:oauth";

/// Summary after a successful forge (for upper-layer logging/echo; no secret material).
#[derive(Debug)]
pub struct ForgeResult {
    pub auth_dir: PathBuf,
    pub account_uuid: String,
    pub org_uuid: String,
    pub enc_file: PathBuf,
}

/// What one-click login did to the sandbox virtual login this run (for factual upper-layer prompts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginAction {
    Reused,   // Existing login intact and consistent; reused as-is; no files written
    Repaired, // Partially damaged; rewritten but same org kept (old conversations preserved)
    Created,  // True first run; mint a brand-new org
}

// ---------- Random & encoding ----------
fn rand_bytes(n: usize) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open("/dev/urandom")?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b)?;
    Ok(b)
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 0xf) as usize] as char);
    }
    s
}

/// base64(32 random bytes): encryption.key field values (matches `.mjs` randomBytes(32).toString("base64")).
fn b64_32() -> std::io::Result<String> {
    Ok(B64.encode(rand_bytes(32)?))
}

/// RFC 4122 v4 UUID (16 random bytes + version/variant bits).
fn uuid_v4() -> std::io::Result<String> {
    let mut b = rand_bytes(16)?;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

// ---------- v2 GCM (matches binary ZtW/XtW and .mjs encryptTokenV2) ----------
fn derive_key(oauth_key_b64: &str) -> Result<[u8; 32], String> {
    let ikm = B64.decode(oauth_key_b64.trim()).map_err(|e| {
        i18n_err(
            "errOauthKeyInvalidBase64",
            json!({ "detail": e.to_string() }),
        )
    })?;
    // Empty salt (= Node hkdfSync Buffer.alloc(0)). HMAC pads an empty key to block size,
    // equivalent to an all-zero salt, so Some(&[]) and None match; use Some(&[]) explicitly to align with Node.
    let hk = Hkdf::<Sha256>::new(Some(&[]), &ikm);
    let mut out = [0u8; 32];
    hk.expand(HKDF_INFO, &mut out)
        .map_err(|_| i18n_err("errOauthHkdfFailed", json!({})))?;
    Ok(out)
}

/// Encrypt: returns `"v2:" + base64(IV ‖ ciphertext ‖ tag)`. aes-gcm appends the 16-byte tag after ciphertext,
/// so `iv ‖ (ciphertext‖tag)` is exactly this format.
pub fn encrypt_token_v2(plaintext: &[u8], oauth_key_b64: &str) -> Result<String, String> {
    let derived = derive_key(oauth_key_b64)?;
    let iv = rand_bytes(12).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived));
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&iv),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| i18n_err("errOauthEncryptFailed", json!({})))?;
    let mut framed = iv;
    framed.extend_from_slice(&ct);
    Ok(format!("v2:{}", B64.encode(&framed)))
}

/// Decrypt `"v2:..."` and verify tag; returns Err on failure (tampering / wrong key).
pub fn decrypt_token_v2(body: &str, oauth_key_b64: &str) -> Result<Vec<u8>, String> {
    let raw = B64
        .decode(
            body.strip_prefix("v2:")
                .ok_or_else(|| i18n_err("errOauthV2PrefixMissing", json!({})))?,
        )
        .map_err(|e| i18n_err("errOauthV2BodyInvalid", json!({ "detail": e.to_string() })))?;
    if raw.len() < 12 + 16 {
        return Err(i18n_err("errOauthV2CiphertextTooShort", json!({})));
    }
    let (iv, rest) = raw.split_at(12);
    let derived = derive_key(oauth_key_b64)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived));
    cipher
        .decrypt(
            Nonce::from_slice(iv),
            Payload {
                msg: rest,
                aad: AAD,
            },
        )
        .map_err(|_| i18n_err("errOauthDecryptFailed", json!({})))
}

// ---------- Path guards & safe write ----------
/// Walk up to the nearest existing ancestor and canonicalize (follow symlinks), then re-append missing tail segments.
fn real_ancestor(p: &Path) -> PathBuf {
    let mut cur = p.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !cur.exists() {
        if let Some(name) = cur.file_name() {
            tail.push(name.to_os_string());
        }
        match cur.parent() {
            Some(par) if par != cur => cur = par.to_path_buf(),
            _ => break,
        }
    }
    let mut base = std::fs::canonicalize(&cur).unwrap_or(cur);
    for name in tail.iter().rev() {
        base.push(name);
    }
    base
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn assert_not_symlink(p: &Path) -> Result<(), String> {
    if is_symlink(p) {
        return Err(i18n_err(
            "errOauthSymlinkRejected",
            json!({ "path": p.display().to_string() }),
        ));
    }
    Ok(())
}

/// Safe write: reject symlinks + O_EXCL temp file + rename + chmod, avoiding follow/race writes to unintended targets.
fn safe_write(path: &Path, data: &[u8], mode: u32) -> Result<(), String> {
    assert_not_symlink(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| i18n_err("errOauthNoParentDir", json!({})))?;
    let suffix = hex(&rand_bytes(6).map_err(|e| e.to_string())?);
    let tmp = parent.join(format!(".tmp-{suffix}"));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true) // O_CREAT|O_EXCL
            .mode(mode)
            .open(&tmp)
            .map_err(|e| i18n_err("errOauthTempFileFailed", json!({ "detail": e.to_string() })))?;
        f.write_all(data).map_err(|e| {
            i18n_err(
                "errOauthWriteTempFailed",
                json!({ "detail": e.to_string() }),
            )
        })?;
    }
    std::fs::rename(&tmp, path)
        .map_err(|e| i18n_err("errOauthRenameFailed", json!({ "detail": e.to_string() })))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| i18n_err("errOauthChmodFailed", json!({ "detail": e.to_string() })))?;
    Ok(())
}

fn chmod_best_effort(p: &Path, mode: u32) {
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

// ---------- Main flow ----------
// Note: one-shot `forge` (always mint new org) was superseded by idempotent `ensure_virtual_login` (fixes #3/#6);
// the app no longer needs it. `forge_guarded` (= guards + one-shot mint) remains for tests to inject a fake
// real_cred_dir and verify guards and writes directly.

/// Inner entry with injectable real credential dir (tests pass a temp dir, never touch real `~/.claude-science`).
/// One-shot mint (new org/account). = `resolve_guarded` guards + `write_login(None,None)`.
/// Test-only now (app uses idempotent `ensure_virtual_login`) → `#[cfg(test)]` avoids dead_code in non-test builds.
#[cfg(test)]
fn forge_guarded(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
    real_cred_dir: &Path,
) -> Result<ForgeResult, String> {
    let resolved = resolve_guarded(auth_dir, email, sandbox_root, real_cred_dir)?;
    write_login(&resolved, email, None, None)
}

/// Guards (before writing anything; `real_ancestor` already follows symlinks): real-dir protection (guard 0,
/// iron rule, highest priority) → inside sandbox root (guard 1) → fake-account email. Returns resolved write root.
fn resolve_guarded(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
    real_cred_dir: &Path,
) -> Result<PathBuf, String> {
    let resolved = real_ancestor(auth_dir);
    // Load-bearing guard 0 (iron rule 1, highest priority, before sandbox-root check): resolved write root must
    // never fall inside or equal real ~/.claude-science. Blocks pre-setting ~/.csswitch/sandbox (or an ancestor)
    // as a symlink into the real tree—then sandbox_root also resolves into the real tree and the sandbox-root
    // check below would pass (resolved and root both inside the real tree). Independent of sandbox root: absolute
    // protection of the real directory; any abnormal layout must never touch it.
    let real_root = real_ancestor(real_cred_dir);
    if resolved.starts_with(&real_root) {
        return Err(i18n_err(
            "errOauthRealScienceDirRejected",
            json!({ "path": real_root.display().to_string() }),
        ));
    }
    // Load-bearing guard 1 (fix P1): resolved must stay under sandbox root. A symlink from auth_dir or an ancestor
    // outside the sandbox makes canonicalize dereference elsewhere; block redirected writes before any file I/O
    // (Node sandbox scope limit restored here as "must be under root").
    let root = real_ancestor(sandbox_root);
    if !resolved.starts_with(&root) {
        return Err(i18n_err(
            "errOauthOutsideSandboxRejected",
            json!({
                "path": resolved.display().to_string(),
                "root": root.display().to_string(),
            }),
        ));
    }
    if !email.ends_with("localhost.invalid") {
        return Err(i18n_err("errOauthEmailInvalid", json!({ "email": email })));
    }
    Ok(resolved)
}

/// Write a virtual login under guarded `resolved`. `prefer_org`/`prefer_account` Some → reuse
/// (repair keeps org so old conversation DB still attaches); None → mint new. Write logic matches legacy forge.
fn write_login(
    resolved: &Path,
    email: &str,
    prefer_org: Option<String>,
    prefer_account: Option<String>,
) -> Result<ForgeResult, String> {
    std::fs::create_dir_all(resolved)
        .map_err(|e| i18n_err("errOauthMkdirFailed", json!({ "detail": e.to_string() })))?;
    chmod_best_effort(resolved, 0o700);

    // —— encryption.key: reuse existing (keeps old .enc decryptable), else mint new ——
    let key_file = resolved.join("encryption.key");
    assert_not_symlink(&key_file)?;
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    if key_file.exists() {
        let txt = std::fs::read_to_string(&key_file).map_err(|e| {
            i18n_err(
                "errOauthReadEncryptionKeyFailed",
                json!({ "detail": e.to_string() }),
            )
        })?;
        for line in txt.lines() {
            if let Some(eq) = line.find('=') {
                if eq > 0 {
                    let v = line[eq + 1..].trim();
                    if !v.is_empty() {
                        keys.insert(line[..eq].trim().to_string(), v.to_string());
                    }
                }
            }
        }
    }
    // P2a: reused OAUTH_ENCRYPTION_KEY must base64-decode to ≥16 bytes, else drop → fill loop below
    // remints. Avoids keeping "present but invalid base64" key → encrypt_token_v2 derive_key fails instead of
    // self-healing. Only this key is validated (we encrypt .enc with it); other three are Science-internal, kept if present.
    let oauth_usable = keys
        .get("OAUTH_ENCRYPTION_KEY")
        .map(|v| B64.decode(v.trim()).map(|b| b.len() >= 16).unwrap_or(false))
        .unwrap_or(false);
    if !oauth_usable {
        keys.remove("OAUTH_ENCRYPTION_KEY");
    }
    for k in KEY_NAMES {
        if !keys.contains_key(k) {
            keys.insert(k.to_string(), b64_32().map_err(|e| e.to_string())?);
        }
    }
    let key_blob = KEY_NAMES
        .iter()
        .map(|k| format!("{k}={}", keys[*k]))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    safe_write(&key_file, key_blob.as_bytes(), 0o600)?;

    // —— Token blob (fields align with _adapt / _tryOauthToken): reuse org/account when preferred, else mint ——
    let account_uuid = match prefer_account {
        Some(a) => a,
        None => uuid_v4().map_err(|e| e.to_string())?,
    };
    let org_uuid = match prefer_org {
        Some(o) => o,
        None => uuid_v4().map_err(|e| e.to_string())?,
    };
    let access = format!(
        "sk-ant-virtual-{}",
        hex(&rand_bytes(24).map_err(|e| e.to_string())?)
    );
    let blob = json!({
        "access_token": access,          // Proxy strips it; value arbitrary
        "refresh_token": "",
        "api_key": null,
        "token_expires_at": "2099-01-01T00:00:00.000Z", // Far future → never refresh over network
        "provider": "claude_ai",
        "scopes": "user:inference user:file_upload user:profile user:mcp_servers user:plugins",
        "email": email,
        "account_uuid": account_uuid.clone(),
        "subscription_type": "max",
        "rate_limit_tier": null,
        "seat_tier": null,
        "org_uuid": org_uuid.clone(),
        "billing_type": null,
        "has_extra_usage_enabled": false
    });
    let plaintext = serde_json::to_vec(&blob).map_err(|e| {
        i18n_err(
            "errOauthSerializeBlobFailed",
            json!({ "detail": e.to_string() }),
        )
    })?;
    let oauth_key = keys
        .get("OAUTH_ENCRYPTION_KEY")
        .ok_or_else(|| i18n_err("errOauthEncryptionKeyMissing", json!({})))?;
    let enc_body = encrypt_token_v2(&plaintext, oauth_key)?;

    // —— Write .oauth-tokens/<sanitized>.enc; clear other .enc first so exactly one remains ——
    let tok_dir = resolved.join(".oauth-tokens");
    assert_not_symlink(&tok_dir)?;
    std::fs::create_dir_all(&tok_dir).map_err(|e| {
        i18n_err(
            "errOauthMkdirTokensFailed",
            json!({ "detail": e.to_string() }),
        )
    })?;
    chmod_best_effort(&tok_dir, 0o700);
    if let Ok(rd) = std::fs::read_dir(&tok_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "enc").unwrap_or(false) {
                assert_not_symlink(&p)?;
                // Delete failure must surface (fix P2): otherwise stale .enc + new .enc = multiple files,
                // but Science expects exactly one → "starts OK but still not logged in".
                std::fs::remove_file(&p).map_err(|err| {
                    i18n_err(
                        "errOauthDeleteOldTokenFailed",
                        json!({
                            "path": p.display().to_string(),
                            "detail": err.to_string(),
                        }),
                    )
                })?;
            }
        }
    }
    let user_id: String = account_uuid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let enc_file = tok_dir.join(format!("{user_id}.enc"));
    safe_write(&enc_file, enc_body.as_bytes(), 0o600)?;

    // —— Self-check: decrypt round-trip with same logic; ensure Science can read it ——
    let roundtrip = decrypt_token_v2(&enc_body, oauth_key)?;
    let rt: serde_json::Value = serde_json::from_slice(&roundtrip).map_err(|e| {
        i18n_err(
            "errOauthSelfVerifyParseFailed",
            json!({ "detail": e.to_string() }),
        )
    })?;
    if rt.get("email").and_then(|v| v.as_str()) != Some(email) {
        return Err(i18n_err("errOauthSelfVerifyEmailMismatch", json!({})));
    }

    // —— active-org.json (Science only requires org_uuid to be a UUID) ——
    let org_json = serde_json::to_string_pretty(&json!({ "org_uuid": org_uuid })).unwrap() + "\n";
    safe_write(
        &resolved.join("active-org.json"),
        org_json.as_bytes(),
        0o600,
    )?;

    Ok(ForgeResult {
        auth_dir: resolved.to_path_buf(),
        account_uuid,
        org_uuid,
        enc_file,
    })
}

// ---------- Idempotent: read/validate existing login ----------
/// Strict check: s is a hex UUID in 8-4-4-4-12 form.
fn looks_like_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Parse encryption.key for OAUTH_ENCRYPTION_KEY (non-empty only).
fn parse_oauth_key(resolved: &Path) -> Option<String> {
    let txt = std::fs::read_to_string(resolved.join("encryption.key")).ok()?;
    for line in txt.lines() {
        if let Some(v) = line.strip_prefix("OAUTH_ENCRYPTION_KEY=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Return path only when `.oauth-tokens/` has exactly one `.enc`; zero or multiple → None.
fn single_enc(resolved: &Path) -> Option<PathBuf> {
    let mut found: Option<PathBuf> = None;
    for e in std::fs::read_dir(resolved.join(".oauth-tokens"))
        .ok()?
        .flatten()
    {
        let p = e.path();
        if p.extension().map(|x| x == "enc").unwrap_or(false) {
            if found.is_some() {
                return None;
            }
            found = Some(p);
        }
    }
    found
}

/// Valid UUID org_uuid from active-org.json.
fn read_active_org(resolved: &Path) -> Option<String> {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(resolved.join("active-org.json")).ok()?)
            .ok()?;
    let o = v.get("org_uuid")?.as_str()?;
    if looks_like_uuid(o) {
        Some(o.to_string())
    } else {
        None
    }
}

/// Best-effort read of valid UUID account_uuid from old .enc (reuse on repair; failure/invalid → None, mint new).
fn read_prior_account(resolved: &Path) -> Option<String> {
    let key = parse_oauth_key(resolved)?;
    let body = std::fs::read_to_string(single_enc(resolved)?).ok()?;
    let blob: serde_json::Value =
        serde_json::from_slice(&decrypt_token_v2(&body, &key).ok()?).ok()?;
    let a = blob.get("account_uuid")?.as_str()?;
    if looks_like_uuid(a) {
        Some(a.to_string())
    } else {
        None
    }
}

/// Valid org_uuid from the sole decryptable .enc token blob (fallback when active-org.json is missing).
fn read_token_org(resolved: &Path) -> Option<String> {
    let key = parse_oauth_key(resolved)?;
    let body = std::fs::read_to_string(single_enc(resolved)?).ok()?;
    let blob: serde_json::Value =
        serde_json::from_slice(&decrypt_token_v2(&body, &key).ok()?).ok()?;
    let o = blob.get("org_uuid")?.as_str()?;
    if looks_like_uuid(o) {
        Some(o.to_string())
    } else {
        None
    }
}

/// Scan `<auth_dir>/orgs/` for historical org directory names shaped like UUIDs (last fallback when active-org and token are gone).
fn scan_org_dirs(resolved: &Path) -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(resolved.join("orgs")) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    if looks_like_uuid(name) {
                        v.push(name.to_string());
                    }
                }
            }
        }
    }
    v
}

/// Today's UTC date `YYYY-MM-DD` (no external crate; Howard Hinnant civil-from-days).
fn today_utc_ymd() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64; // Days since 1970-01-01
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Whether the date part of `token_expires_at` (ISO8601) is ≥ today (UTC), i.e. not expired (P2).
/// Compare first 10 chars `YYYY-MM-DD` lexicographically with today (ISO8601 date order = time order);
/// malformed input (too short / not `dddd-dd-dd`) counts as expired. Day granularity suffices for far-future vs past.
fn token_not_expired(expires_at: &str) -> bool {
    if expires_at.len() < 10 {
        return false;
    }
    let date = &expires_at[..10];
    let b = date.as_bytes();
    let shaped = b.iter().enumerate().all(|(i, &c)| match i {
        4 | 7 => c == b'-',
        _ => c.is_ascii_digit(),
    });
    shaped && date >= today_utc_ymd().as_str()
}

/// Whether existing login is complete and self-consistent; if so return identity (for reuse). Any failure → None
/// (→ downgrade to repair, safe direction). Strict checks (P2): read paths must not be symlinks; decrypted org matches,
/// email is fake account, `account_uuid` valid UUID, `provider`=claude_ai, `access_token` non-empty,
/// `token_expires_at` not expired.
fn read_intact_login(resolved: &Path, email: &str) -> Option<ForgeResult> {
    // Read paths must not be symlinks (no follow; suspicious layout → treat as inconsistent → repair; repair uses assert_not_symlink).
    if is_symlink(&resolved.join("encryption.key"))
        || is_symlink(&resolved.join(".oauth-tokens"))
        || is_symlink(&resolved.join("active-org.json"))
    {
        return None;
    }
    let key = parse_oauth_key(resolved)?;
    let enc = single_enc(resolved)?;
    if is_symlink(&enc) {
        return None;
    }
    let active_org = read_active_org(resolved)?;
    let body = std::fs::read_to_string(&enc).ok()?;
    let blob: serde_json::Value =
        serde_json::from_slice(&decrypt_token_v2(&body, &key).ok()?).ok()?;
    let blob_org = blob.get("org_uuid")?.as_str()?;
    let blob_email = blob.get("email")?.as_str()?;
    let account = blob.get("account_uuid")?.as_str()?;
    let provider_ok = blob.get("provider").and_then(|v| v.as_str()) == Some("claude_ai");
    let access_ok = blob
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let expiry_ok = blob
        .get("token_expires_at")
        .and_then(|v| v.as_str())
        .map(token_not_expired)
        .unwrap_or(false);
    if blob_org != active_org
        || blob_email != email
        || !blob_email.ends_with("localhost.invalid")
        || !looks_like_uuid(account)
        || !provider_ok
        || !access_ok
        || !expiry_ok
    {
        return None;
    }
    Some(ForgeResult {
        auth_dir: resolved.to_path_buf(),
        account_uuid: account.to_string(),
        org_uuid: active_org,
        enc_file: enc,
    })
}

/// Idempotent virtual login: intact → reuse; partially damaged → repair keeping org; true first run → mint new.
pub fn ensure_virtual_login(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
) -> Result<(ForgeResult, LoginAction), String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| i18n_err("errOauthNoHome", json!({})))?;
    ensure_virtual_login_guarded(auth_dir, email, sandbox_root, &home.join(".claude-science"))
}

/// Inner entry with injectable real credential dir (for tests).
fn ensure_virtual_login_guarded(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
    real_cred_dir: &Path,
) -> Result<(ForgeResult, LoginAction), String> {
    let resolved = resolve_guarded(auth_dir, email, sandbox_root, real_cred_dir)?;
    // Intact → reuse as-is, touch no files (operon may be reading).
    if let Some(fr) = read_intact_login(&resolved, email) {
        return Ok((fr, LoginAction::Reused));
    }
    // Org source priority (P1a, never silently mint new): active-org.json → decryptable token → orgs/ dirs.
    // Reuse any located historical org (preserve old conversations); multiple orgs with no active → error and abort.
    let (prior_org, action) = if let Some(o) = read_active_org(&resolved) {
        (Some(o), LoginAction::Repaired)
    } else if let Some(o) = read_token_org(&resolved) {
        (Some(o), LoginAction::Repaired)
    } else {
        let dirs = scan_org_dirs(&resolved);
        match dirs.len() {
            0 => (None, LoginAction::Created), // True first run: no history
            1 => (Some(dirs[0].clone()), LoginAction::Repaired), // Adopt sole historical org
            _ => {
                return Err(i18n_err(
                    "errOauthOrphanOrgs",
                    json!({
                        "count": dirs.len(),
                        "orgs_dir": format!("{}/orgs/", resolved.display()),
                        "active_org_path": format!("{}/active-org.json", resolved.display()),
                    }),
                ));
            }
        }
    };
    let prior_account = read_prior_account(&resolved);
    let fr = write_login(&resolved, email, prior_org, prior_account)?;
    Ok((fr, action))
}

/// Read-only check: whether sandbox virtual login is currently complete and self-consistent (directly reusable).
/// **Never writes any file** (operon may be reading). For one-click health fast path: daemon alive on `8990` ≠ login usable;
/// if login is broken (legacy / damaged creds / login page), reopening only lands on login page again → stop sandbox →
/// repair keeping org → restart. Fixes 0.2.0 fast path treating "healthy but login dead" as OK (0.2.1 Bug2).
pub fn login_intact(auth_dir: &Path, email: &str, sandbox_root: &Path) -> bool {
    match std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude-science")) {
        Some(real) => login_intact_guarded(auth_dir, email, sandbox_root, &real),
        None => false,
    }
}

/// Inner entry with injectable real credential dir (for tests). Guard failure (abnormal layout / inside real tree) →
/// not intact (false), nudging upper layer to repair; repair path still has guards, never touches real dir. Read-only.
fn login_intact_guarded(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
    real_cred_dir: &Path,
) -> bool {
    match resolve_guarded(auth_dir, email, sandbox_root, real_cred_dir) {
        Ok(resolved) => read_intact_login(&resolved, email).is_some(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static CTR: AtomicU32 = AtomicU32::new(0);

    fn tmpdir(tag: &str) -> PathBuf {
        let n = CTR.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!(
            "csswitch-forge-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn read_oauth_key(auth_dir: &Path) -> String {
        let txt = std::fs::read_to_string(auth_dir.join("encryption.key")).unwrap();
        for line in txt.lines() {
            if let Some(v) = line.strip_prefix("OAUTH_ENCRYPTION_KEY=") {
                return v.trim().to_string();
            }
        }
        panic!("no OAUTH_ENCRYPTION_KEY");
    }

    fn the_enc_file(auth_dir: &Path) -> PathBuf {
        let tok = auth_dir.join(".oauth-tokens");
        let mut encs: Vec<PathBuf> = std::fs::read_dir(&tok)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "enc").unwrap_or(false))
            .collect();
        assert_eq!(encs.len(), 1, "exactly one .enc expected");
        encs.pop().unwrap()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = b64_32().unwrap();
        let pt = br#"{"email":"virtual@localhost.invalid","x":1}"#;
        let body = encrypt_token_v2(pt, &key).unwrap();
        assert!(body.starts_with("v2:"));
        let back = decrypt_token_v2(&body, &key).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn decrypt_fails_on_wrong_key() {
        let k1 = b64_32().unwrap();
        let k2 = b64_32().unwrap();
        let body = encrypt_token_v2(b"hello", &k1).unwrap();
        assert!(
            decrypt_token_v2(&body, &k2).is_err(),
            "wrong key should fail verify"
        );
    }

    #[test]
    fn forge_writes_files_and_selfchecks() {
        let dir = tmpdir("ok");
        let fake_real = tmpdir("realcred"); // Different from auth_dir so guards allow
        let email = "virtual@localhost.invalid";
        let r = forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert!(r.enc_file.is_file());
        assert!(dir.join("encryption.key").is_file());
        assert!(dir.join("active-org.json").is_file());
        // Decrypt round-trip matches
        let key = read_oauth_key(&dir);
        let body = std::fs::read_to_string(the_enc_file(&dir)).unwrap();
        let blob: serde_json::Value =
            serde_json::from_slice(&decrypt_token_v2(&body, &key).unwrap()).unwrap();
        assert_eq!(blob["email"], email);
        assert_eq!(blob["provider"], "claude_ai");
        // active-org.json org_uuid matches summary
        let org: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("active-org.json")).unwrap())
                .unwrap();
        assert_eq!(org["org_uuid"], r.org_uuid);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake_real);
    }

    #[test]
    fn forge_rejects_real_cred_dir() {
        // auth_dir equals real cred dir (temp dir stand-in) → must reject and write nothing.
        let real = tmpdir("real2");
        let r = forge_guarded(&real, "virtual@localhost.invalid", &real, &real);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("errOauthRealScienceDirRejected"));
        assert!(
            !real.join("encryption.key").exists(),
            "reject path must not write any files"
        );
    }

    #[test]
    fn forge_rejects_symlink_into_real_science_tree() {
        // Iron-rule regression: pre-set sandbox-root ancestor symlink → real Science dir; sandbox root then
        // resolves into real tree and guard 1 "inside sandbox root" alone would pass (resolved and root both in real tree).
        // Guard 0 must reject before any write; real directory unchanged.
        let real = tmpdir("real-science");
        std::fs::create_dir_all(real.join(".oauth-tokens")).unwrap();
        std::fs::write(real.join(".oauth-tokens/victim.enc"), b"keep-me").unwrap();
        std::fs::write(real.join("encryption.key"), b"KEEP=me\n").unwrap();

        let csw = tmpdir("csw");
        std::fs::create_dir_all(&csw).unwrap();
        // ~/.csswitch/sandbox -> real Science dir (pre-set malicious/abnormal symlink)
        let sandbox_link = csw.join("sandbox");
        std::os::unix::fs::symlink(&real, &sandbox_link).unwrap();

        let sandbox_root = sandbox_link.join("home");
        let auth_dir = sandbox_root.join(".claude-science");
        let r = forge_guarded(&auth_dir, "virtual@localhost.invalid", &sandbox_root, &real);
        assert!(
            r.is_err(),
            "sandbox root resolving into real tree via symlink must be rejected"
        );
        assert!(r.unwrap_err().contains("errOauthRealScienceDirRejected"));
        // Real directory unchanged.
        assert_eq!(
            std::fs::read(real.join("encryption.key")).unwrap(),
            b"KEEP=me\n"
        );
        assert!(
            real.join(".oauth-tokens/victim.enc").exists(),
            "real .enc must not be touched"
        );
        assert!(
            !real.join("home").exists(),
            "must not create anything in real tree"
        );
        for d in [real, csw] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn forge_rejects_symlink_escaping_sandbox_root() {
        // P1 regression: auth_dir inside sandbox pre-set as symlink to outside dir; forger must reject before any write and never touch link target.
        let root = tmpdir("sbroot");
        std::fs::create_dir_all(&root).unwrap();
        let outside = tmpdir("outside");
        std::fs::create_dir_all(&outside).unwrap();
        // Pre-seed target with a stale .enc that must not be deleted and a key file that must not be overwritten.
        std::fs::create_dir_all(outside.join(".oauth-tokens")).unwrap();
        std::fs::write(outside.join(".oauth-tokens/victim.enc"), b"keep-me").unwrap();
        std::fs::write(outside.join("encryption.key"), b"KEEP=me\n").unwrap();

        let auth_dir = root.join(".claude-science");
        std::os::unix::fs::symlink(&outside, &auth_dir).unwrap(); // auth_dir -> outside

        let fake_real = tmpdir("realcred5");
        let r = forge_guarded(&auth_dir, "virtual@localhost.invalid", &root, &fake_real);
        assert!(r.is_err(), "symlink escaping sandbox root must be rejected");
        assert!(r.unwrap_err().contains("errOauthOutsideSandboxRejected"));
        // Target directory unchanged.
        assert_eq!(
            std::fs::read(outside.join("encryption.key")).unwrap(),
            b"KEEP=me\n"
        );
        assert!(
            outside.join(".oauth-tokens/victim.enc").exists(),
            "stale .enc must not be deleted"
        );
        assert!(
            !outside.join("active-org.json").exists(),
            "must not write to symlink target"
        );
        for d in [root, outside, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn forge_rejects_non_localhost_email() {
        let dir = tmpdir("email");
        let fake_real = tmpdir("realcred3");
        let r = forge_guarded(&dir, "attacker@example.com", &dir, &fake_real);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("errOauthEmailInvalid"));
    }

    // ---- node ↔ rust cross-compat: byte-level v2 GCM format matches .mjs (skip if no node) ----
    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR = <repo>/desktop/src-tauri
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }
    fn have_node() -> bool {
        std::process::Command::new("node")
            .arg("-v")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn crosscompat_rust_reads_node_and_node_reads_rust() {
        if !have_node() {
            eprintln!("skip: node not available in environment");
            return;
        }
        let root = repo_root();
        let mjs = root.join("scripts/make-virtual-oauth.mjs");
        let decrypt_mjs = root.join("test/decrypt-oauth.mjs");
        let email = "virtual@localhost.invalid";

        // Direction 1: node forge → rust decrypt readback.
        let dir_n = tmpdir("node2rust");
        let st = std::process::Command::new("node")
            .arg(&mjs)
            .args(["--auth-dir"])
            .arg(&dir_n)
            .args(["--email", email, "--force"])
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(st.success(), "node forge should succeed");
        let key = read_oauth_key(&dir_n);
        let body = std::fs::read_to_string(the_enc_file(&dir_n)).unwrap();
        let blob: serde_json::Value =
            serde_json::from_slice(&decrypt_token_v2(&body, &key).unwrap()).unwrap();
        assert_eq!(blob["email"], email, "rust should decrypt node's .enc");
        assert_eq!(blob["provider"], "claude_ai");

        // Direction 2: rust forge → node decrypt readback.
        let dir_r = tmpdir("rust2node");
        let fake_real = tmpdir("realcred4");
        forge_guarded(&dir_r, email, &dir_r, &fake_real).unwrap();
        let out = std::process::Command::new("node")
            .arg(&decrypt_mjs)
            .args(["--auth-dir"])
            .arg(&dir_r)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "node decrypt should succeed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let printed = String::from_utf8_lossy(&out.stdout);
        assert!(
            printed.contains(email),
            "node should decrypt rust .enc, output: {printed}"
        );
        assert!(printed.contains("claude_ai"));

        for d in [dir_n, dir_r, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    // ---------- Idempotent ensure_virtual_login ----------
    fn read_active_org_uuid(auth_dir: &Path) -> String {
        let v: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(auth_dir.join("active-org.json")).unwrap(),
        )
        .unwrap();
        v["org_uuid"].as_str().unwrap().to_string()
    }

    #[test]
    fn ensure_reuses_intact_login() {
        let dir = tmpdir("reuse");
        let fake_real = tmpdir("realcred-reuse");
        let email = "virtual@localhost.invalid";
        let first = forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        let enc0 = std::fs::read(the_enc_file(&dir)).unwrap();
        let key0 = std::fs::read(dir.join("encryption.key")).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Reused);
        assert_eq!(r.org_uuid, first.org_uuid, "org unchanged");
        assert_eq!(r.org_uuid, org0);
        assert_eq!(
            std::fs::read(the_enc_file(&dir)).unwrap(),
            enc0,
            ".enc bytes unchanged"
        );
        assert_eq!(
            std::fs::read(dir.join("encryption.key")).unwrap(),
            key0,
            "key bytes unchanged"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_still_rejects_real_cred_dir() {
        let real = tmpdir("real-ensure");
        let r = ensure_virtual_login_guarded(&real, "virtual@localhost.invalid", &real, &real);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("errOauthRealScienceDirRejected"));
        assert!(
            !real.join("encryption.key").exists(),
            "reject path must not write any files"
        );
        let _ = std::fs::remove_dir_all(&real);
    }

    #[test]
    fn ensure_repairs_missing_enc_keeps_org() {
        let dir = tmpdir("rep-missing");
        let fake_real = tmpdir("realcred-rm");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        std::fs::remove_file(the_enc_file(&dir)).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Repaired);
        assert_eq!(r.org_uuid, org0, "repair must keep original org");
        assert_eq!(read_active_org_uuid(&dir), org0);
        let key = read_oauth_key(&dir);
        let body = std::fs::read_to_string(the_enc_file(&dir)).unwrap();
        assert!(decrypt_token_v2(&body, &key).is_ok());
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_repairs_extra_enc_keeps_org() {
        let dir = tmpdir("rep-extra");
        let fake_real = tmpdir("realcred-re");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        std::fs::write(dir.join(".oauth-tokens/stale.enc"), b"v2:garbage").unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Repaired);
        assert_eq!(r.org_uuid, org0);
        let _ = the_enc_file(&dir); // internal assert: exactly one .enc
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_repairs_replaced_key_keeps_org() {
        let dir = tmpdir("rep-key");
        let fake_real = tmpdir("realcred-rk");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        let mut blob = String::new();
        for k in KEY_NAMES {
            blob.push_str(&format!("{k}={}\n", b64_32().unwrap()));
        }
        std::fs::write(dir.join("encryption.key"), blob).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(
            action,
            LoginAction::Repaired,
            "active-org.json still present → repair not mint"
        );
        assert_eq!(r.org_uuid, org0, "org unchanged when key rotated");
        let key = read_oauth_key(&dir);
        let body = std::fs::read_to_string(the_enc_file(&dir)).unwrap();
        assert!(
            decrypt_token_v2(&body, &key).is_ok(),
            "reminted token should decrypt"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_creates_on_first_run() {
        let dir = tmpdir("create");
        let fake_real = tmpdir("realcred-cr");
        let email = "virtual@localhost.invalid";
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Created);
        assert!(r.enc_file.is_file());
        assert!(looks_like_uuid(&r.org_uuid));
        assert_eq!(read_active_org_uuid(&dir), r.org_uuid);
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_recovers_org_from_token_when_active_org_missing() {
        // P1a: active-org.json missing but .enc decryptable → recover org from token, never mint new.
        let dir = tmpdir("tok-org");
        let fake_real = tmpdir("realcred-to");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        std::fs::remove_file(dir.join("active-org.json")).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Repaired);
        assert_eq!(r.org_uuid, org0, "should recover org from token");
        assert_eq!(
            read_active_org_uuid(&dir),
            org0,
            "active-org.json should be rewritten with same org"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_adopts_single_org_dir_when_active_and_token_gone() {
        // P1a: active-org.json and .enc gone, but exactly one historical org under orgs/ → adopt it.
        let dir = tmpdir("one-orgdir");
        let fake_real = tmpdir("realcred-1o");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        std::fs::create_dir_all(dir.join("orgs").join(&org0)).unwrap();
        std::fs::remove_file(the_enc_file(&dir)).unwrap();
        std::fs::remove_file(dir.join("active-org.json")).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(action, LoginAction::Repaired);
        assert_eq!(r.org_uuid, org0, "should adopt sole historical org dir");
        assert_eq!(read_active_org_uuid(&dir), org0);
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_errors_on_ambiguous_multi_org() {
        // P1a: no active-org, no decryptable token, multiple orgs under orgs/ → error, never silently mint.
        let dir = tmpdir("multi-org");
        let fake_real = tmpdir("realcred-mo");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let a = uuid_v4().unwrap();
        let b = uuid_v4().unwrap();
        std::fs::create_dir_all(dir.join("orgs").join(&a)).unwrap();
        std::fs::create_dir_all(dir.join("orgs").join(&b)).unwrap();
        std::fs::remove_file(the_enc_file(&dir)).unwrap();
        std::fs::remove_file(dir.join("active-org.json")).unwrap();
        let r = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real);
        assert!(r.is_err(), "ambiguous multi-org history should error");
        assert!(r.unwrap_err().contains("errOauthOrphanOrgs"));
        assert!(
            !dir.join("active-org.json").exists(),
            "error path must not write active-org.json"
        );
        assert_eq!(
            scan_org_dirs(&dir).len(),
            2,
            "must not silently mint new org"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_recreates_key_on_invalid_base64() {
        // P2a: invalid base64 OAUTH_ENCRYPTION_KEY → no error; remint valid key, reuse org, new .enc decrypts.
        let dir = tmpdir("badkey");
        let fake_real = tmpdir("realcred-bk");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        let mut blob = String::new();
        for k in KEY_NAMES {
            if k == "OAUTH_ENCRYPTION_KEY" {
                blob.push_str(&format!("{k}=!!!!not-base64!!!!\n"));
            } else {
                blob.push_str(&format!("{k}={}\n", b64_32().unwrap()));
            }
        }
        std::fs::write(dir.join("encryption.key"), blob).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(
            action,
            LoginAction::Repaired,
            "active-org.json still present → repair"
        );
        assert_eq!(r.org_uuid, org0, "org unchanged when key rotated");
        let key = read_oauth_key(&dir);
        let body = std::fs::read_to_string(the_enc_file(&dir)).unwrap();
        assert!(
            decrypt_token_v2(&body, &key).is_ok(),
            "new .enc should decrypt after key remint"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_repairs_when_token_structurally_damaged() {
        // P2: decryptable .enc but structurally damaged (tampered provider / non-UUID account) → not Reused, repair keeping org.
        let dir = tmpdir("bad-struct");
        let fake_real = tmpdir("realcred-bs");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        let key = read_oauth_key(&dir);
        // Rewrite a decryptable but structurally bad .enc with existing key: tampered provider, non-UUID account.
        let bad = serde_json::json!({
            "email": email,
            "org_uuid": org0,
            "account_uuid": "not-a-uuid",
            "provider": "tampered",
            "token_expires_at": "2099-01-01T00:00:00.000Z"
        });
        let enc = encrypt_token_v2(&serde_json::to_vec(&bad).unwrap(), &key).unwrap();
        std::fs::write(the_enc_file(&dir), enc).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(
            action,
            LoginAction::Repaired,
            "structural damage should repair not reuse"
        );
        assert_eq!(r.org_uuid, org0, "repair still keeps org");
        // After repair should be self-consistent again (provider=claude_ai, valid UUID account)
        assert!(
            looks_like_uuid(&r.account_uuid),
            "account should be valid UUID after repair"
        );
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn ensure_repairs_when_token_expired() {
        // P2: decryptable but expired token → not Reused, repair; after repair (far-future) should reuse.
        let dir = tmpdir("expired");
        let fake_real = tmpdir("realcred-exp");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        let org0 = read_active_org_uuid(&dir);
        let key = read_oauth_key(&dir);
        let expired = serde_json::json!({
            "email": email,
            "org_uuid": org0,
            "account_uuid": uuid_v4().unwrap(),
            "provider": "claude_ai",
            "access_token": "sk-ant-virtual-x",
            "token_expires_at": "2000-01-01T00:00:00.000Z"
        });
        let enc = encrypt_token_v2(&serde_json::to_vec(&expired).unwrap(), &key).unwrap();
        std::fs::write(the_enc_file(&dir), enc).unwrap();
        let (r, action) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(
            action,
            LoginAction::Repaired,
            "expired token must not be misclassified as Reused"
        );
        assert_eq!(r.org_uuid, org0);
        // After repair new token is far-future → second ensure should reuse.
        let (_r2, a2) = ensure_virtual_login_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert_eq!(a2, LoginAction::Reused, "should reuse after repair");
        for d in [dir, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn login_intact_true_for_fresh_false_when_damaged_and_readonly() {
        // Bug2 (0.2.1): health fast path needs read-only check to separate "intact" vs "healthy but login dead".
        let dir = tmpdir("intact");
        let fake_real = tmpdir("realcred-intact");
        let email = "virtual@localhost.invalid";
        forge_guarded(&dir, email, &dir, &fake_real).unwrap();

        // Fresh forge → intact → true.
        assert!(
            login_intact_guarded(&dir, email, &dir, &fake_real),
            "fresh forge should be judged intact"
        );

        // Read-only: call twice; three file byte blobs unchanged.
        let enc0 = std::fs::read(the_enc_file(&dir)).unwrap();
        let key0 = std::fs::read(dir.join("encryption.key")).unwrap();
        let org0 = std::fs::read(dir.join("active-org.json")).unwrap();
        assert!(login_intact_guarded(&dir, email, &dir, &fake_real));
        assert_eq!(
            std::fs::read(the_enc_file(&dir)).unwrap(),
            enc0,
            "must not modify .enc"
        );
        assert_eq!(
            std::fs::read(dir.join("encryption.key")).unwrap(),
            key0,
            "must not modify encryption.key"
        );
        assert_eq!(
            std::fs::read(dir.join("active-org.json")).unwrap(),
            org0,
            "must not modify active-org.json"
        );

        // Delete .enc → not intact → false (old stub always-true would fail here).
        std::fs::remove_file(the_enc_file(&dir)).unwrap();
        assert!(
            !login_intact_guarded(&dir, email, &dir, &fake_real),
            "missing .enc should be judged not intact"
        );

        // Expired token → not intact → false.
        let dir2 = tmpdir("intact-exp");
        let fr2 = tmpdir("realcred-exp2");
        forge_guarded(&dir2, email, &dir2, &fr2).unwrap();
        let org2 = read_active_org_uuid(&dir2);
        let key2 = read_oauth_key(&dir2);
        let expired = serde_json::json!({
            "email": email, "org_uuid": org2, "account_uuid": uuid_v4().unwrap(),
            "provider": "claude_ai", "access_token": "sk-ant-virtual-x",
            "token_expires_at": "2000-01-01T00:00:00.000Z"
        });
        let enc = encrypt_token_v2(&serde_json::to_vec(&expired).unwrap(), &key2).unwrap();
        std::fs::write(the_enc_file(&dir2), enc).unwrap();
        assert!(
            !login_intact_guarded(&dir2, email, &dir2, &fr2),
            "expired token should be judged not intact"
        );

        for d in [dir, fake_real, dir2, fr2] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn token_expiry_check() {
        assert!(token_not_expired("2099-01-01T00:00:00.000Z"));
        assert!(!token_not_expired("2000-01-01T00:00:00.000Z"));
        assert!(!token_not_expired(""), "empty string counts as expired");
        assert!(!token_not_expired("2099-13"), "too short counts as expired");
        assert!(
            !token_not_expired("20990101ZZ"),
            "malformed date counts as expired"
        );
        let t = today_utc_ymd();
        assert_eq!(t.len(), 10);
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[7..8], "-");
    }
}
