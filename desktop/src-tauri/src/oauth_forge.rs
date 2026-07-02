//! 虚拟 OAuth 伪造器（Rust 原生，替代 `scripts/make-virtual-oauth.mjs`，去 node 依赖）。
//!
//! 在【沙箱】auth_dir 里写一套本地自造、绝不联网的登录凭证，让 Claude Science 认为已登录
//! （virtual@localhost.invalid），推理经 `ANTHROPIC_BASE_URL` 导去本项目代理。全程零 Anthropic
//! 接触、零真实凭证。逆向依据与 `.mjs` 一致（见该文件头注与 `docs/verified-facts.md`）：
//!   - 令牌文件 `<auth_dir>/.oauth-tokens/<sanitized account_uuid>.enc`（目录里恰好一个 .enc）
//!   - 内容 v2 格式：`"v2:" + base64( IV(12) ‖ AES-256-GCM(密文) ‖ authTag(16) )`
//!     derivedKey = HKDF-SHA256(ikm=base64_decode(OAUTH_ENCRYPTION_KEY), salt=空, info="operon:aes-256-gcm:oauth", 32)
//!     AAD = "v2:oauth"；明文 = JSON(tokenBlob)
//!   - `encryption.key`：换行分隔 KEY=base64(≥16B)；过期设远期 → 绝不触发联网刷新
//!   - `active-org.json`：`{ "org_uuid": <uuid> }`（Science 只校验 org_uuid 是合法 UUID）
//!
//! 铁律护栏：**载重护栏 = 绝不写真实凭证目录**（载荷是假凭证，唯一致命的就是误写真实
//! `~/.claude-science`）；另加假账号（email 必须 localhost.invalid）、写前拒符号链接、
//! O_EXCL 临时文件 + rename + 0600。`.mjs` 里那条「必须在 `.sandbox/` 下」是给人手敲
//! `--auth-dir` 的 CLI 兜底；本函数由 app 用自己构造的沙箱路径调用，不需要该启发式。
//!
//! 与 `.mjs` 的 v2 GCM 格式**字节兼容**，由本文件 `tests` 的 node↔rust 双向对拍单测钉死。

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

const KEY_NAMES: [&str; 4] = [
    "ANTHROPIC_API_KEY_ENCRYPTION_KEY",
    "OAUTH_ENCRYPTION_KEY",
    "JWT_SIGNING_SECRET",
    "USER_SECRET_ENCRYPTION_KEY",
];
const HKDF_INFO: &[u8] = b"operon:aes-256-gcm:oauth";
const AAD: &[u8] = b"v2:oauth";

/// 伪造成功后的摘要（供上层日志/回显，不含任何密钥材料）。
#[derive(Debug)]
pub struct ForgeResult {
    pub auth_dir: PathBuf,
    pub account_uuid: String,
    pub org_uuid: String,
    pub enc_file: PathBuf,
}

// ---------- 随机与编码 ----------
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

/// base64(32 随机字节)：encryption.key 各项的值（与 `.mjs` 的 randomBytes(32).toString("base64") 一致）。
fn b64_32() -> std::io::Result<String> {
    Ok(B64.encode(rand_bytes(32)?))
}

/// RFC 4122 v4 UUID（16 随机字节 + 版本/变体位）。
fn uuid_v4() -> std::io::Result<String> {
    let mut b = rand_bytes(16)?;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

// ---------- v2 GCM（与二进制 ZtW/XtW、.mjs encryptTokenV2 一致） ----------
fn derive_key(oauth_key_b64: &str) -> Result<[u8; 32], String> {
    let ikm = B64
        .decode(oauth_key_b64.trim())
        .map_err(|e| format!("OAUTH_ENCRYPTION_KEY 非法 base64：{e}"))?;
    // salt 空（= Node hkdfSync 的 Buffer.alloc(0)）。HMAC 会把空 key 补零到块长，
    // 与全零 salt 等价，故 Some(&[]) 与 None 结果相同；这里显式用 Some(&[]) 对齐 Node。
    let hk = Hkdf::<Sha256>::new(Some(&[]), &ikm);
    let mut out = [0u8; 32];
    hk.expand(HKDF_INFO, &mut out)
        .map_err(|_| "hkdf expand 失败".to_string())?;
    Ok(out)
}

/// 加密：返回 `"v2:" + base64(IV ‖ 密文 ‖ tag)`。aes-gcm 把 16 字节 tag 追加在密文末尾，
/// 故 `iv ‖ (密文‖tag)` 恰为该格式。
pub fn encrypt_token_v2(plaintext: &[u8], oauth_key_b64: &str) -> Result<String, String> {
    let derived = derive_key(oauth_key_b64)?;
    let iv = rand_bytes(12).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived));
    let ct = cipher
        .encrypt(Nonce::from_slice(&iv), Payload { msg: plaintext, aad: AAD })
        .map_err(|_| "aes-gcm 加密失败".to_string())?;
    let mut framed = iv;
    framed.extend_from_slice(&ct);
    Ok(format!("v2:{}", B64.encode(&framed)))
}

/// 解密 `"v2:..."`，校验 tag；失败（含篡改/密钥不符）返回 Err。
pub fn decrypt_token_v2(body: &str, oauth_key_b64: &str) -> Result<Vec<u8>, String> {
    let raw = B64
        .decode(body.strip_prefix("v2:").ok_or("缺 v2: 前缀")?)
        .map_err(|e| format!("v2 体非法 base64：{e}"))?;
    if raw.len() < 12 + 16 {
        return Err("v2 密文过短".into());
    }
    let (iv, rest) = raw.split_at(12);
    let derived = derive_key(oauth_key_b64)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&derived));
    cipher
        .decrypt(Nonce::from_slice(iv), Payload { msg: rest, aad: AAD })
        .map_err(|_| "aes-gcm 解密/验签失败".to_string())
}

// ---------- 路径护栏与安全写 ----------
/// 逐层向上找到最近的已存在祖先并 canonicalize（看穿符号链接），再把不存在的尾巴拼回。
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
        return Err(format!("拒绝：{} 是符号链接，绝不跟随写入。", p.display()));
    }
    Ok(())
}

/// 安全写：拒符号链接 + O_EXCL 临时文件 + rename + chmod，避免跟随/竞态写到非预期目标。
fn safe_write(path: &Path, data: &[u8], mode: u32) -> Result<(), String> {
    assert_not_symlink(path)?;
    let parent = path.parent().ok_or("目标无父目录")?;
    let suffix = hex(&rand_bytes(6).map_err(|e| e.to_string())?);
    let tmp = parent.join(format!(".tmp-{suffix}"));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true) // O_CREAT|O_EXCL
            .mode(mode)
            .open(&tmp)
            .map_err(|e| format!("建临时文件失败：{e}"))?;
        f.write_all(data).map_err(|e| format!("写临时文件失败：{e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("rename 失败：{e}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| format!("chmod 失败：{e}"))?;
    Ok(())
}

fn chmod_best_effort(p: &Path, mode: u32) {
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

// ---------- 主流程 ----------
/// 在沙箱 `auth_dir` 写虚拟登录。`sandbox_root` 是允许写入的沙箱根（app 传 `sandbox_home()`），
/// 解析后的路径必须落在其下，防符号链接重定向。护栏另用真实 `~/.claude-science` 作对照。
pub fn forge(auth_dir: &Path, email: &str, sandbox_root: &Path) -> Result<ForgeResult, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("无 HOME 环境变量")?;
    forge_guarded(auth_dir, email, sandbox_root, &home.join(".claude-science"))
}

/// 可注入「真实凭证目录」的内层（供测试传入临时目录，不碰真实 `~/.claude-science`）。
fn forge_guarded(
    auth_dir: &Path,
    email: &str,
    sandbox_root: &Path,
    real_cred_dir: &Path,
) -> Result<ForgeResult, String> {
    // —— 护栏（写任何东西之前；real_ancestor 已看穿符号链接）——
    let resolved = real_ancestor(auth_dir);
    // 载重护栏 0（铁律 1，最高优先，先于沙箱根检查）：解析后的写入根绝不落在真实
    // ~/.claude-science 之内或其本身。防「把 ~/.csswitch/sandbox（或其祖先）预置成指向
    // 真实目录的符号链接」——此时 sandbox_root 也会解析进真实树，令下方『沙箱根内』检查失效
    // （resolved 与 root 同处真实树内而放行）。这条不依赖沙箱根，是对真实目录的绝对保护：
    // 任何异常布局都绝不触碰真实目录。
    let real_root = real_ancestor(real_cred_dir);
    if resolved.starts_with(&real_root) {
        return Err(format!(
            "拒绝：auth_dir 解析到真实 Science 目录（{}）之内或本身，铁律禁止触碰。",
            real_root.display()
        ));
    }
    // 载重护栏 1（修 P1）：resolved 必须落在沙箱根之下。预置符号链接把 auth_dir 或其祖先
    // 链到沙箱外任意目录会让 canonicalize 解引用到别处；把写重定向挡在写任何文件之前
    // （Node 版的沙箱范围限制在此以「根内」形式恢复）。
    let root = real_ancestor(sandbox_root);
    if !resolved.starts_with(&root) {
        return Err(format!(
            "拒绝：auth_dir 解析到沙箱根之外（{} 不在 {} 下），疑似符号链接重定向。",
            resolved.display(),
            root.display()
        ));
    }
    if !email.ends_with("localhost.invalid") {
        return Err(format!(
            "拒绝：email 必须以 localhost.invalid 结尾（当前 {email}），确保是假账号。"
        ));
    }

    std::fs::create_dir_all(&resolved).map_err(|e| format!("建 auth_dir 失败：{e}"))?;
    chmod_best_effort(&resolved, 0o700);

    // —— encryption.key：复用已存在的（保持旧 .enc 可解），否则新造 ——
    let key_file = resolved.join("encryption.key");
    assert_not_symlink(&key_file)?;
    let mut keys: BTreeMap<String, String> = BTreeMap::new();
    if key_file.exists() {
        let txt = std::fs::read_to_string(&key_file).map_err(|e| format!("读 encryption.key 失败：{e}"))?;
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

    // —— 令牌 blob（字段对齐 _adapt / _tryOauthToken）——
    let account_uuid = uuid_v4().map_err(|e| e.to_string())?;
    let org_uuid = uuid_v4().map_err(|e| e.to_string())?;
    let access = format!("sk-ant-virtual-{}", hex(&rand_bytes(24).map_err(|e| e.to_string())?));
    let blob = json!({
        "access_token": access,          // 代理会剥离，值任意
        "refresh_token": "",
        "api_key": null,
        "token_expires_at": "2099-01-01T00:00:00.000Z", // 远期 → 绝不联网刷新
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
    let plaintext = serde_json::to_vec(&blob).map_err(|e| format!("序列化 blob 失败：{e}"))?;
    let oauth_key = keys.get("OAUTH_ENCRYPTION_KEY").ok_or("缺 OAUTH_ENCRYPTION_KEY")?;
    let enc_body = encrypt_token_v2(&plaintext, oauth_key)?;

    // —— 写 .oauth-tokens/<sanitized>.enc；先清其它 .enc 保证唯一 ——
    let tok_dir = resolved.join(".oauth-tokens");
    assert_not_symlink(&tok_dir)?;
    std::fs::create_dir_all(&tok_dir).map_err(|e| format!("建 .oauth-tokens 失败：{e}"))?;
    chmod_best_effort(&tok_dir, 0o700);
    if let Ok(rd) = std::fs::read_dir(&tok_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "enc").unwrap_or(false) {
                assert_not_symlink(&p)?;
                // 删除失败必须显式失败（修 P2）：否则残留旧 .enc + 新 .enc = 多个，
                // 而 Science 预期目录内恰好一个 → 会「显示启动成功却仍登录不上」。
                std::fs::remove_file(&p)
                    .map_err(|err| format!("删除旧令牌 {} 失败：{err}（需目录内恰好一个 .enc）", p.display()))?;
            }
        }
    }
    let user_id: String = account_uuid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let enc_file = tok_dir.join(format!("{user_id}.enc"));
    safe_write(&enc_file, enc_body.as_bytes(), 0o600)?;

    // —— 自校验：用同样逻辑解密回读，确保 Science 能解开 ——
    let roundtrip = decrypt_token_v2(&enc_body, oauth_key)?;
    let rt: serde_json::Value =
        serde_json::from_slice(&roundtrip).map_err(|e| format!("自校验解析失败：{e}"))?;
    if rt.get("email").and_then(|v| v.as_str()) != Some(email) {
        return Err("自校验失败：解密回读的 email 不符".into());
    }

    // —— active-org.json（Science 只要求 org_uuid 是 UUID）——
    let org_json = serde_json::to_string_pretty(&json!({ "org_uuid": org_uuid })).unwrap() + "\n";
    safe_write(&resolved.join("active-org.json"), org_json.as_bytes(), 0o600)?;

    Ok(ForgeResult {
        auth_dir: resolved,
        account_uuid,
        org_uuid,
        enc_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static CTR: AtomicU32 = AtomicU32::new(0);

    fn tmpdir(tag: &str) -> PathBuf {
        let n = CTR.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("csswitch-forge-{}-{}-{}", std::process::id(), tag, n));
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
        assert_eq!(encs.len(), 1, "应恰好一个 .enc");
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
        assert!(decrypt_token_v2(&body, &k2).is_err(), "错 key 应验签失败");
    }

    #[test]
    fn forge_writes_files_and_selfchecks() {
        let dir = tmpdir("ok");
        let fake_real = tmpdir("realcred"); // 与 auth_dir 不同，护栏放行
        let email = "virtual@localhost.invalid";
        let r = forge_guarded(&dir, email, &dir, &fake_real).unwrap();
        assert!(r.enc_file.is_file());
        assert!(dir.join("encryption.key").is_file());
        assert!(dir.join("active-org.json").is_file());
        // 解密回读一致
        let key = read_oauth_key(&dir);
        let body = std::fs::read_to_string(the_enc_file(&dir)).unwrap();
        let blob: serde_json::Value =
            serde_json::from_slice(&decrypt_token_v2(&body, &key).unwrap()).unwrap();
        assert_eq!(blob["email"], email);
        assert_eq!(blob["provider"], "claude_ai");
        // active-org.json 的 org_uuid 与摘要一致
        let org: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("active-org.json")).unwrap()).unwrap();
        assert_eq!(org["org_uuid"], r.org_uuid);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fake_real);
    }

    #[test]
    fn forge_rejects_real_cred_dir() {
        // auth_dir 指向「真实凭证目录」（这里用临时目录扮演）→ 必须拒、且不写。
        let real = tmpdir("real2");
        let r = forge_guarded(&real, "virtual@localhost.invalid", &real, &real);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("真实 Science 目录"));
        assert!(!real.join("encryption.key").exists(), "拒绝路径不应写任何文件");
    }

    #[test]
    fn forge_rejects_symlink_into_real_science_tree() {
        // 铁律回归：把沙箱根的祖先预置成指向【真实 Science 目录】的符号链接——此时沙箱根
        // 自身也解析进真实树，仅靠护栏 1「沙箱根内」会放行（resolved 与 root 同在真实树内）。
        // 护栏 0 必须在写任何文件之前拒绝，且真实目录零改动。
        let real = tmpdir("real-science");
        std::fs::create_dir_all(real.join(".oauth-tokens")).unwrap();
        std::fs::write(real.join(".oauth-tokens/victim.enc"), b"keep-me").unwrap();
        std::fs::write(real.join("encryption.key"), b"KEEP=me\n").unwrap();

        let csw = tmpdir("csw");
        std::fs::create_dir_all(&csw).unwrap();
        // ~/.csswitch/sandbox -> 真实 Science 目录（预置的恶意/异常软链接）
        let sandbox_link = csw.join("sandbox");
        std::os::unix::fs::symlink(&real, &sandbox_link).unwrap();

        let sandbox_root = sandbox_link.join("home");
        let auth_dir = sandbox_root.join(".claude-science");
        let r = forge_guarded(&auth_dir, "virtual@localhost.invalid", &sandbox_root, &real);
        assert!(r.is_err(), "沙箱根经符号链接落入真实树必须被拒");
        assert!(r.unwrap_err().contains("真实 Science 目录"));
        // 真实目录零改动。
        assert_eq!(std::fs::read(real.join("encryption.key")).unwrap(), b"KEEP=me\n");
        assert!(real.join(".oauth-tokens/victim.enc").exists(), "真实 .enc 不该被碰");
        assert!(!real.join("home").exists(), "不该在真实树里建任何目录");
        for d in [real, csw] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn forge_rejects_symlink_escaping_sandbox_root() {
        // P1 回归：把沙箱内的 auth_dir 预置成指向沙箱外目录的符号链接，伪造器必须
        // 在写任何文件之前拒绝，且绝不碰链接目标。
        let root = tmpdir("sbroot");
        std::fs::create_dir_all(&root).unwrap();
        let outside = tmpdir("outside");
        std::fs::create_dir_all(&outside).unwrap();
        // 预置目标里一个「不该被删」的旧 .enc 与一个「不该被覆盖」的 key 文件。
        std::fs::create_dir_all(outside.join(".oauth-tokens")).unwrap();
        std::fs::write(outside.join(".oauth-tokens/victim.enc"), b"keep-me").unwrap();
        std::fs::write(outside.join("encryption.key"), b"KEEP=me\n").unwrap();

        let auth_dir = root.join(".claude-science");
        std::os::unix::fs::symlink(&outside, &auth_dir).unwrap(); // auth_dir -> outside

        let fake_real = tmpdir("realcred5");
        let r = forge_guarded(&auth_dir, "virtual@localhost.invalid", &root, &fake_real);
        assert!(r.is_err(), "符号链接逃出沙箱根应被拒");
        assert!(r.unwrap_err().contains("沙箱根之外"));
        // 目标目录零改动。
        assert_eq!(std::fs::read(outside.join("encryption.key")).unwrap(), b"KEEP=me\n");
        assert!(outside.join(".oauth-tokens/victim.enc").exists(), "旧 .enc 不该被删");
        assert!(!outside.join("active-org.json").exists(), "不该写入目标");
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
        assert!(r.unwrap_err().contains("localhost.invalid"));
    }

    // ---- node ↔ rust 双向对拍：证明与 .mjs 的 v2 GCM 格式字节兼容（无 node 则跳过）----
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
            eprintln!("跳过：环境无 node");
            return;
        }
        let root = repo_root();
        let mjs = root.join("scripts/make-virtual-oauth.mjs");
        let decrypt_mjs = root.join("test/decrypt-oauth.mjs");
        let email = "virtual@localhost.invalid";

        // 方向 1：node 伪造 → rust 解密读回。
        let dir_n = tmpdir("node2rust");
        let st = std::process::Command::new("node")
            .arg(&mjs)
            .args(["--auth-dir"])
            .arg(&dir_n)
            .args(["--email", email, "--force"])
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(st.success(), "node 伪造应成功");
        let key = read_oauth_key(&dir_n);
        let body = std::fs::read_to_string(the_enc_file(&dir_n)).unwrap();
        let blob: serde_json::Value =
            serde_json::from_slice(&decrypt_token_v2(&body, &key).unwrap()).unwrap();
        assert_eq!(blob["email"], email, "rust 应能解开 node 的 .enc");
        assert_eq!(blob["provider"], "claude_ai");

        // 方向 2：rust 伪造 → node 解密读回。
        let dir_r = tmpdir("rust2node");
        let fake_real = tmpdir("realcred4");
        forge_guarded(&dir_r, email, &dir_r, &fake_real).unwrap();
        let out = std::process::Command::new("node")
            .arg(&decrypt_mjs)
            .args(["--auth-dir"])
            .arg(&dir_r)
            .output()
            .unwrap();
        assert!(out.status.success(), "node 解密应成功：{}", String::from_utf8_lossy(&out.stderr));
        let printed = String::from_utf8_lossy(&out.stdout);
        assert!(printed.contains(email), "node 应能解开 rust 的 .enc，输出：{printed}");
        assert!(printed.contains("claude_ai"));

        for d in [dir_n, dir_r, fake_real] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }
}
