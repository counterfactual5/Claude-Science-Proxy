//! 本地配置读写：`~/.csp/CSP.json`。多 profile 形态（schema v2）。
//!
//! 安全要求（对齐 spec §3 / §5.1，参考 CC Switch 的明文本地存储但加严文件安全）：
//!   - 目录 0700，文件 0600。
//!   - 读/写前 `lstat`（symlink_metadata）拒绝符号链接，绝不跟随写到别处或读到别处。
//!   - 写用「临时文件（O_CREAT|O_EXCL, 0600）+ 原子 rename」，避免半写与竞态。
//!   - profile key 明文存盘（用户已知悉），但**绝不进日志**；回显给前端只给掩码（末 4 位）。
//!
//! 存储升级：schema_version 探测 + v1（旧固定槽）一次性迁移 → v2（profile 列表 + active_id），
//! v3（多模型 active_models）→ v4（provider pool：active_ids[]），
//! 迁移前留 `CSP.json.v1.bak`（失败即中止），普通覆盖前留滚动 `CSP.json.bak`，
//! 清 key / 删 profile 后净化滚动备份（旧明文 key 不可从 .bak 恢复）。
//!
//! 所有函数以显式 `dir` 参数工作，便于用临时目录做无副作用的单元测试；
//! 生产代码用 [`default_dir`]（`$HOME/.csp`）。

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
        return Err("端口 8765 是真实 Science 实例保留端口，不能用。".into());
    }
    if proxy_port == 0 || sandbox_port == 0 {
        return Err("端口不能为 0。".into());
    }
    if proxy_port == sandbox_port {
        return Err("代理端口与沙箱端口不能相同。".into());
    }
    Ok(())
}

/// 当前配置 schema 版本。>4 的文件由更新版本 app 写入，本版本拒绝启动（不误改）。
pub const CURRENT_SCHEMA_VERSION: u32 = 4;

pub(crate) const CONFIG_BASENAME: &str = "CSP.json";
pub(crate) const LEGACY_CONFIG_BASENAME: &str = "config.json";
pub(crate) const LEGACY_DIR_BASENAME: &str = ".csswitch";
const MIGRATION_BACKUP_NAME: &str = "CSP.json.v1.bak";
const ROLLING_BACKUP_NAME: &str = "CSP.json.bak";

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// 一条命名配置。cc-switch 叫 provider，我们叫 profile。key 明文存盘、只回掩码。
/// 运行行为与 UI 能力都由 `template_id` 经 templates 注册表派生（不靠 name/icon/base_url 猜身份）。
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
    /// 暴露给 Science 的模型列表（可多选）。空时从 `model` 迁移。
    #[serde(default)]
    pub active_models: Vec<String>,
    /// 默认/主模型（后台 agent 兜底）。空时用 active_models 首项。
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

/// 顶层配置。字段都有默认值，缺字段的旧文件也能读。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Config {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// 生效 profile 的 id（向后兼容：等于 active_ids 首项；新代码以 active_ids 为准）。
    #[serde(default)]
    pub active_id: String,
    /// 同时生效的 profile id 列表（provider pool）。空=无生效配置。
    #[serde(default)]
    pub active_ids: Vec<String>,
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default = "default_sandbox_port")]
    pub sandbox_port: u16,
    /// 代理的 path-secret。**持久化**并跨代理重启/切 profile/重开 app 复用，
    /// 这样已在跑的沙箱（其 ANTHROPIC_BASE_URL 里嵌了该 secret）不会因代理换 secret 而 403。
    /// 首次为空，由后端生成一次后写回。
    #[serde(default)]
    pub secret: String,
    /// 遗留字段：旧版「官方 Claude」模式已移除，加载时一律归一为 `proxy`。
    #[serde(default = "default_mode")]
    pub mode: String,
    /// 一次性迁移提示（#9 甲：回填默认模型后告知用户）。get_config 读后清空。
    #[serde(default)]
    pub pending_notice: Option<String>,
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
            pending_notice: None,
        }
    }
}

impl Profile {
    /// 生效的模型列表：active_models 优先，否则回退到单个 model。
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

    /// 写盘后保持 model/default_model/active_models 一致。
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
    /// 当前生效 profile 列表（active_ids 中仍存在的项，保持顺序）。
    pub fn active_profiles(&self) -> Vec<&Profile> {
        self.active_ids
            .iter()
            .filter_map(|id| self.profile_by_id(id))
            .collect()
    }

    /// 是否在某条 profile 在 active pool 中。
    pub fn is_profile_active(&self, id: &str) -> bool {
        self.active_ids.iter().any(|x| x == id)
    }

    /// 向 pool 追加 active（已存在则不变）。
    pub fn activate_profile(&mut self, id: &str) {
        if self.profile_by_id(id).is_some() && !self.is_profile_active(id) {
            self.active_ids.push(id.to_string());
            self.sync_active_id();
        }
    }

    /// 从 pool 移除 active。
    pub fn deactivate_profile(&mut self, id: &str) {
        self.active_ids.retain(|x| x != id);
        self.sync_active_id();
    }

    /// 仅保留一条 active（legacy 独占切换）。
    pub fn set_exclusive_active(&mut self, id: &str) {
        if self.profile_by_id(id).is_some() {
            self.active_ids = vec![id.to_string()];
            self.sync_active_id();
        }
    }

    /// 保持 active_id 与 active_ids 首项一致（API 向后兼容）。
    pub fn sync_active_id(&mut self) {
        self.active_id = self.active_ids.first().cloned().unwrap_or_default();
    }

    /// 当前生效 profile（active_ids 空 → None；兼容旧 active_id 只读路径）。
    pub fn active_profile(&self) -> Option<&Profile> {
        self.active_profiles().first().copied()
    }
    pub fn profile_by_id(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
    pub fn profile_by_id_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }
}

/// 16 字节随机 → 32 hex 字符。/dev/urandom（unix）；不可用时退回时间纳秒。
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

/// epoch 毫秒（用作 created_at / sort_index 初值）。
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------- 版本探测 ----------
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

/// <2（含缺失=0）→ Legacy；==2 → V2；==3 → V3（迁移到 v4）；==4 → V4；>4 → TooNew。
pub fn detect_version(data: &[u8]) -> io::Result<VersionKind> {
    let probe: VersionProbe = serde_json::from_slice(data).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CSP.json 解析失败：{e}"),
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

/// 旧固定槽 → 新 profile 列表。空槽（key/base_url/model 全空）跳过；
/// 旧 provider 指针命中已迁 profile → active_id 指它，否则 ""（不静默选第一条）。
pub fn migrate_v1_to_v2(mut legacy: crate::config_legacy::ConfigV1) -> Config {
    // 先把遗留裸 relay 槽归位到 relay-<preset>。
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
        pending_notice: None,
    }
}

/// 生产环境配置目录：`$HOME/.csp`。
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".csp")
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join(CONFIG_BASENAME)
}

/// 一次性路径迁移：`.csswitch/config.json` → `~/.csp/CSP.json`，
/// 以及 `~/.csp/config.json` → `CSP.json`；同步备份文件名。
/// 仅在生产默认目录上运行（单元测试用临时 dir 时跳过）。
fn migrate_legacy_paths(dir: &Path) -> io::Result<()> {
    if default_dir().as_path() != dir {
        return Ok(());
    }
    let target = config_path(dir);
    if target.exists() {
        return Ok(());
    }

    if dir.exists() {
        let legacy_in_dir = dir.join(LEGACY_CONFIG_BASENAME);
        if legacy_in_dir.is_file() {
            assert_not_symlink(&legacy_in_dir)?;
            ensure_dir(dir)?;
            fs::rename(&legacy_in_dir, &target)?;
            rename_legacy_backups(dir);
            return Ok(());
        }
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let legacy_dir = home.join(LEGACY_DIR_BASENAME);
    let legacy_config = legacy_dir.join(LEGACY_CONFIG_BASENAME);
    if !legacy_config.is_file() {
        return Ok(());
    }
    assert_not_symlink(&legacy_dir)?;
    assert_not_symlink(&legacy_config)?;

    let migrate_whole_dir = !dir.exists() || dir_is_empty(dir)?;
    if migrate_whole_dir {
        ensure_dir(dir)?;
        copy_dir_contents(&legacy_dir, dir)?;
        let copied_config = dir.join(LEGACY_CONFIG_BASENAME);
        if copied_config.is_file() && !target.exists() {
            fs::rename(&copied_config, &target)?;
        }
    } else {
        ensure_dir(dir)?;
        atomic_copy(&legacy_config, &target)?;
    }
    rename_legacy_backups(dir);
    Ok(())
}

fn dir_is_empty(dir: &Path) -> io::Result<bool> {
    Ok(fs::read_dir(dir)?.next().is_none())
}

fn rename_legacy_backups(dir: &Path) {
    for (old, new) in [
        ("config.json.v1.bak", MIGRATION_BACKUP_NAME),
        ("config.json.bak", ROLLING_BACKUP_NAME),
    ] {
        let from = dir.join(old);
        let to = dir.join(new);
        if from.is_file() && !to.exists() {
            let _ = fs::rename(from, to);
        }
    }
}

/// 递归拷贝目录内容（跳过符号链接；文件用 atomic_copy 保证 0600）。
fn copy_dir_contents(from: &Path, to: &Path) -> io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let src = entry.path();
        let dest = to.join(&name);
        if ft.is_dir() {
            if !dest.exists() {
                fs::create_dir_all(&dest)?;
                fs::set_permissions(&dest, fs::Permissions::from_mode(0o700))?;
            }
            copy_dir_contents(&src, &dest)?;
        } else if ft.is_file() {
            if !dest.exists() {
                atomic_copy(&src, &dest)?;
            }
        }
    }
    Ok(())
}

/// 若 path 存在且是符号链接则报错（不跟随）。path 不存在返回 Ok。
pub(crate) fn assert_not_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("拒绝符号链接（防跟随写/读到别处）：{}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// 确保配置目录存在且是普通目录、权限 0700。目录是符号链接则拒绝。
pub fn ensure_dir(dir: &Path) -> io::Result<()> {
    assert_not_symlink(dir)?;
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    let md = fs::metadata(dir)?;
    if !md.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("配置目录不是目录：{}", dir.display()),
        ));
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

// ---------- 备份 ----------
/// 原子拷贝 src → dst（拒符号链接、0600、O_EXCL 临时文件 + rename）。src 不存在 → Err。
fn atomic_copy(src: &Path, dst: &Path) -> io::Result<()> {
    assert_not_symlink(dst)?;
    let data = fs::read(src)?; // src 不存在 → Err（迁移备份据此中止）
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

/// 迁移前备份旧 CSP.json → CSP.json.v1.bak。源不存在 / 备份失败 → Err（中止迁移）。
pub fn write_migration_backup(dir: &Path) -> io::Result<()> {
    atomic_copy(&config_path(dir), &dir.join(MIGRATION_BACKUP_NAME))
}

/// 普通保存前的单份滚动备份 → CSP.json.bak。best-effort（调用方可忽略 Err），但写法仍原子/0600。
pub fn write_rolling_backup(dir: &Path) -> io::Result<()> {
    atomic_copy(&config_path(dir), &dir.join(ROLLING_BACKUP_NAME))
}

/// 清 key / 删 profile 后净化滚动备份：直接删，避免旧明文 key 残留可恢复。
pub fn drop_rolling_backup(dir: &Path) {
    let _ = fs::remove_file(dir.join(ROLLING_BACKUP_NAME));
}

/// 从 `dir/CSP.json` 读配置。文件不存在返回 [`Config::default`]。
/// 旧文件（schema<2）→ 备份 v1.bak + 迁移 + 落盘 v2；schema>2 → Err（拒绝启动）。
/// v2 悬空 active_id 归一化为空。文件/目录是符号链接则报错（不跟随读）。
pub fn load_from(dir: &Path) -> io::Result<Config> {
    migrate_legacy_paths(dir)?;
    // 目录本身也不许是符号链接：否则攻击者把 ~/.csp 换成软链就能让读取跟随到别处。
    assert_not_symlink(dir)?;
    let path = config_path(dir);
    assert_not_symlink(&path)?;
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e),
    };
    // 存在即复位权限，抵御外部把它改宽。
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    match detect_version(&data)? {
        VersionKind::TooNew(v) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CSP.json 由更新版本（schema {v}）写入，请升级 Claude Science Proxy 后再打开。"
            ),
        )),
        VersionKind::Legacy => {
            write_migration_backup(dir)?; // 备份失败即中止迁移，不动原文件
            let legacy: crate::config_legacy::ConfigV1 =
                serde_json::from_slice(&data).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("旧 config 解析失败：{e}"),
                    )
                })?;
            let cfg = normalize_active(migrate_v1_to_v2(legacy));
            validate_loaded_ports(&cfg)?;
            save_to(dir, &cfg)?; // 落盘为 v2（幂等，下次读走 V2 分支）
            Ok(cfg)
        }
        VersionKind::V2 => {
            let cfg: Config = serde_json::from_slice(&data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CSP.json 解析失败：{e}"),
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
                    format!("CSP.json 解析失败：{e}"),
                )
            })?;
            let mut cfg = migrate_v3_to_v4(normalize_active(cfg));
            for p in cfg.profiles.iter_mut() {
                p.sync_model_fields();
            }
            validate_loaded_ports(&cfg)?;
            save_to(dir, &cfg)?;
            Ok(cfg)
        }
        VersionKind::V4 => {
            let raw: Config = serde_json::from_slice(&data).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CSP.json 解析失败：{e}"),
                )
            })?;
            let mode_migrated = raw.mode != "proxy";
            let mut cfg = normalize_active(raw);
            for p in cfg.profiles.iter_mut() {
                p.sync_model_fields();
            }
            validate_loaded_ports(&cfg)?;
            if mode_migrated {
                save_to(dir, &cfg)?;
            }
            Ok(cfg)
        }
    }
}

fn validate_loaded_ports(cfg: &Config) -> io::Result<()> {
    validate_runtime_ports(cfg.proxy_port, cfg.sandbox_port).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CSP.json 端口无效：{e}"),
        )
    })
}

/// 加载后归一化不变式（spec §4）：
/// - `template_id` 未命中注册表 → 归一化为 `custom`；
/// - `active_ids` / `active_id` 悬空 → 剔除并同步；
/// - 旧文件仅有 `active_id` → 回填 `active_ids`；
/// - 遗留 `mode: "official"` → 一律归一为 `proxy`（官方模式 UI/命令已移除）。
fn normalize_active(mut cfg: Config) -> Config {
    for p in cfg.profiles.iter_mut() {
        if crate::templates::by_id(&p.template_id).is_none() {
            p.template_id = "custom".to_string();
        }
    }
    if cfg.active_ids.is_empty() && !cfg.active_id.is_empty() {
        if cfg.profile_by_id(&cfg.active_id).is_some() {
            cfg.active_ids.push(cfg.active_id.clone());
        }
    }
    cfg.active_ids = cfg
        .active_ids
        .iter()
        .filter(|id| cfg.profile_by_id(id).is_some())
        .cloned()
        .collect();
    if !cfg.active_id.is_empty() && cfg.profile_by_id(&cfg.active_id).is_none() {
        cfg.active_id.clear();
    }
    cfg.sync_active_id();
    if cfg.mode != "proxy" {
        cfg.mode = default_mode();
    }
    cfg
}

/// v2 → v3：补齐 active_models/default_model，并 bump schema_version。
fn migrate_v2_to_v3(mut cfg: Config) -> Config {
    cfg.schema_version = 3;
    for p in cfg.profiles.iter_mut() {
        p.sync_model_fields();
    }
    cfg
}

/// v3 → v4：active_id → active_ids，并 bump schema_version。
fn migrate_v3_to_v4(mut cfg: Config) -> Config {
    cfg.schema_version = CURRENT_SCHEMA_VERSION;
    if cfg.active_ids.is_empty() && !cfg.active_id.is_empty() {
        if cfg.profile_by_id(&cfg.active_id).is_some() {
            cfg.active_ids = vec![cfg.active_id.clone()];
        }
    }
    cfg.sync_active_id();
    cfg
}

/// 原子写 `dir/CSP.json`（0600）。目录/目标文件是符号链接则拒绝。
pub fn save_to(dir: &Path, cfg: &Config) -> io::Result<()> {
    ensure_dir(dir)?;
    let path = config_path(dir);
    assert_not_symlink(&path)?;
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("config 序列化失败：{e}"),
        )
    })?;

    // 临时文件与目标同目录（保证 rename 在同一文件系统上原子）。
    // 名字带 pid + 线程 id，避免同进程并发写者撞同一个 O_EXCL 临时名。
    let tmp = dir.join(format!(
        ".CSP.json.tmp-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    // O_CREAT|O_EXCL + 0600：拒绝复用已有临时文件，创建即定权限。
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
    // rename 覆盖目标名本身（替换符号链接名而非跟随），但上面已 assert 目标非链接。
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// 序列化的「读-改-写」：进程内全局写锁下 load → apply → save，避免并发命令
/// 各读一份旧 config、各改一个字段、互相覆盖。
pub fn update<F: FnOnce(&mut Config)>(dir: &Path, f: F) -> io::Result<Config> {
    static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cfg = load_from(dir)?;
    f(&mut cfg);
    save_to(dir, &cfg)?;
    Ok(cfg)
}

/// 掩码：固定 4 个圆点 + 末 4 位（`••••tail`）。空 key 返回空串；≤4 位全遮。
/// 定长而非随 key 长度增长：长 key 的掩码不会在列表里撑出横向溢出（WKWebView 不给连续
/// 圆点断行，`word-break` 拦不住），且不泄漏 key 长度。绝不返回完整 key，是回显前端的唯一形式。
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
        // 每个测试用「进程 id + 线程 id」独立子目录，避免并行测试相互踩。
        let base = std::env::temp_dir().join(format!("csswitch-cfg-test-{}", std::process::id()));
        let d = base.join(format!("{:?}", std::thread::current().id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn mode_of(p: &Path) -> u32 {
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    // ---------- A1: 结构 + 访问器 + new_id/now_ms ----------
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
        let d = tmpdir().join(".csswitch");
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
    fn activate_and_deactivate_profile_pool() {
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
            ..Default::default()
        };
        c.activate_profile("a");
        c.activate_profile("b");
        assert_eq!(c.active_ids, vec!["a", "b"]);
        assert_eq!(c.active_id, "a");
        c.deactivate_profile("a");
        assert_eq!(c.active_ids, vec!["b"]);
        c.set_exclusive_active("a");
        assert_eq!(c.active_ids, vec!["a"]);
    }

    #[test]
    fn migrate_v3_to_v4_populates_active_ids() {
        let d = tmpdir().join(".csswitch-v3");
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
        let d = tmpdir().join(".csswitch");
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
            ("proxy_8765", 8765, 8990),
            ("sandbox_8765", 18991, 8765),
            ("proxy_zero", 0, 8990),
            ("sandbox_zero", 18991, 0),
            ("same_ports", 18991, 18991),
        ];
        for (name, proxy_port, sandbox_port) in cases {
            let d = tmpdir().join(format!(".csswitch-{name}"));
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
                err.to_string().contains("CSP.json 端口无效"),
                "error should identify invalid config ports for {name}: {err}"
            );
        }
    }

    #[test]
    fn load_rejects_legacy_invalid_ports_before_v2_save() {
        let d = tmpdir().join(".csswitch-legacy-bad-port");
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

    // ---------- A2: 版本探测 ----------
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

    // ---------- A4: 迁移 v1 → v2 ----------
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
        ); // 空槽
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
            "旧 provider=relay-glm → 生效指该 profile"
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
        // 旧 provider 指向空/不存在的槽 → active_id 必须为空（不静默选第一条）。
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
        assert_eq!(cfg.active_id, "", "非法 active → 空，等用户选");
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

    // ---------- A5: 备份基础设施 ----------
    #[test]
    fn migration_backup_copies_and_is_0600() {
        let d = tmpdir().join(".csswitch");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), b"OLD-V1-BYTES").unwrap();
        write_migration_backup(&d).unwrap();
        let bak = d.join("CSP.json.v1.bak");
        assert_eq!(fs::read(&bak).unwrap(), b"OLD-V1-BYTES");
        assert_eq!(mode_of(&bak), 0o600);
    }
    #[test]
    fn migration_backup_missing_source_errors() {
        let d = tmpdir().join(".csswitch");
        fs::create_dir_all(&d).unwrap();
        assert!(write_migration_backup(&d).is_err());
    }
    #[test]
    fn rolling_backup_then_drop_removes_key_recoverability() {
        let d = tmpdir().join(".csswitch");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), br#"{"api_key":"sk-SECRET-TAIL"}"#).unwrap();
        write_rolling_backup(&d).unwrap();
        let bak = d.join("CSP.json.bak");
        assert!(fs::read_to_string(&bak).unwrap().contains("sk-SECRET-TAIL"));
        drop_rolling_backup(&d);
        assert!(
            !bak.exists(),
            "净化后滚动备份应删除，清了的 key 不可从 .bak 恢复"
        );
    }
    #[test]
    fn backup_rejects_symlinked_target() {
        let base = tmpdir();
        let d = base.join(".csswitch");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), b"X").unwrap();
        let elsewhere = base.join("elsewhere");
        fs::write(&elsewhere, b"ORIG").unwrap();
        symlink(&elsewhere, d.join("CSP.json.v1.bak")).unwrap();
        assert!(write_migration_backup(&d).is_err());
        assert_eq!(fs::read(&elsewhere).unwrap(), b"ORIG");
    }

    // ---------- A6: load_from 整合 ----------
    #[test]
    fn load_migrates_old_file_and_writes_v1_bak() {
        let d = tmpdir().join(".csswitch");
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
        assert!(d.join("CSP.json.v1.bak").exists(), "迁移必须留 v1 备份");
        // 落盘后再读是 v4（幂等，不再迁移）。
        let again = load_from(&d).unwrap();
        assert_eq!(again, cfg);
        assert_eq!(again.schema_version, CURRENT_SCHEMA_VERSION);
    }
    #[test]
    fn load_too_new_errors() {
        let d = tmpdir().join(".csswitch");
        fs::create_dir_all(&d).unwrap();
        fs::write(config_path(&d), br#"{"schema_version":9,"profiles":[]}"#).unwrap();
        let e = load_from(&d).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData);
        assert!(e.to_string().contains("更新版本"));
    }
    #[test]
    fn load_normalizes_dangling_active() {
        let d = tmpdir().join(".csswitch");
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
            "悬空 active → 归一化为空"
        );
        assert_eq!(got.active_id, "");
    }

    // ---------- MP-2 Minor [2]: template_id 未命中 → 归一 custom ----------
    #[test]
    fn load_normalizes_unknown_template_id_to_custom() {
        let d = tmpdir().join(".csswitch");
        // 造一条 template_id 未命中注册表的 v2 profile（连接字段保留）。
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
        assert_eq!(p.template_id, "custom", "未命中 template_id → 归一 custom");
        assert_eq!(p.base_url, "https://relay.example/claude", "连接字段保留");
        assert_eq!(p.api_key, "sk-x");
        assert_eq!(got.active_ids, vec!["p1"]);
        assert_eq!(got.active_id, "p1", "active 仍有效，不被清空");
    }

    // ---------- 既有安全/权限不变量（保留） ----------
    #[test]
    fn load_missing_returns_default() {
        let d = tmpdir().join(".csswitch");
        let cfg = load_from(&d).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(cfg.proxy_port, 18991);
    }

    #[test]
    fn save_sets_dir_0700_and_file_0600() {
        let d = tmpdir().join(".csswitch");
        save_to(&d, &Config::default()).unwrap();
        assert_eq!(mode_of(&d), 0o700, "dir must be 0700");
        assert_eq!(mode_of(&config_path(&d)), 0o600, "file must be 0600");
    }

    #[test]
    fn load_resets_widened_perms_to_0600() {
        let d = tmpdir().join(".csswitch");
        save_to(&d, &Config::default()).unwrap();
        let p = config_path(&d);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
        load_from(&d).unwrap();
        assert_eq!(mode_of(&p), 0o600, "load must reset perms to 0600");
    }

    #[test]
    fn save_rejects_symlinked_file_and_leaves_target_untouched() {
        let base = tmpdir();
        let d = base.join(".csswitch");
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
        let d = base.join(".csswitch");
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
        let link = base.join(".csswitch");
        symlink(&realdir, &link).unwrap();
        let err = load_from(&link).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn ensure_dir_rejects_symlinked_dir() {
        let base = tmpdir();
        let realdir = base.join("realdir");
        fs::create_dir_all(&realdir).unwrap();
        let link = base.join(".csswitch");
        symlink(&realdir, &link).unwrap();
        let err = save_to(&link, &Config::default()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn no_tmp_file_left_after_save() {
        let d = tmpdir().join(".csswitch");
        save_to(&d, &Config::default()).unwrap();
        let leftovers: Vec<_> = fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".CSP.json.tmp")
            })
            .collect();
        assert!(leftovers.is_empty(), "临时文件应已 rename 掉");
    }

    #[test]
    fn update_applies_and_persists() {
        let d = tmpdir().join(".csswitch");
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
        // path-secret 一旦生成必须持久化，代理重启/重开 app 仍是同一个值。
        let d = tmpdir().join(".csswitch");
        save_to(&d, &Config::default()).unwrap();
        assert!(load_from(&d).unwrap().secret.is_empty(), "初始应为空");
        update(&d, |c| c.secret = "deadbeef00112233".into()).unwrap();
        assert_eq!(load_from(&d).unwrap().secret, "deadbeef00112233");
        // 再改别的字段，secret 不受影响。
        update(&d, |c| c.proxy_port = 20000).unwrap();
        assert_eq!(load_from(&d).unwrap().secret, "deadbeef00112233");
    }

    #[test]
    fn mask_hides_all_but_last4() {
        assert_eq!(mask("sk-1234567890ab"), "••••90ab"); // 定长 4 点 + 末4
        assert_eq!(mask(""), "");
        assert_eq!(mask("abc"), "•••");
        assert_eq!(mask("abcd"), "••••");
        assert_eq!(mask("abcde"), "••••bcde"); // 定长 4 点 + 末4
        let full = "sk-secret-tail9999";
        assert!(!mask(full).contains("secret"));
        // 定长：掩码总长恒为 8（4 点 + 末4），不随 key 长度变长、不泄漏长度
        assert_eq!(
            mask("sk-aaaaaaaaaaaaaaaaaaaaaaaaaaaa1234").chars().count(),
            8
        );
    }
}
