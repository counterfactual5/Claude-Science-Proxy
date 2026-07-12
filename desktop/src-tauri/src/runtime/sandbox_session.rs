use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::Runtime;

use crate::runtime::i18n::i18n_err;
use crate::runtime::operation::{
    self, OperationKind, OperationStage, OperationTrace, POLL_INTERVAL_MS,
};
use crate::runtime::proxy_lifecycle::ensure_proxy;
use crate::runtime::science::{
    ensure_sandbox_runtime_permissions, sandbox_home, sandbox_running_ours, sandbox_url,
    stop_sandbox,
};
use crate::runtime::system::{asset_root, log_path, open_in_browser, open_log, redact, tail_file};
use crate::{config, lifecycle, lock, oauth_forge, proc, AppState, SharedAppState};

fn stop_sandbox_state<R: Runtime>(
    app: &tauri::AppHandle<R>,
    st: &mut AppState,
) -> Result<(), String> {
    stop_sandbox(app, &mut st.sandbox, &mut st.sandbox_url)
}

/// One-click session startup: active proxy, virtual login, sandbox, browser.
///
/// Callers must hold the command serializer lock.
pub(crate) fn one_click_login<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
) -> Result<Value, String> {
    let trace = OperationTrace::start(OperationKind::OneClickLogin, "command=one_click_login");
    let (pport, secret, proxy_action) = ensure_proxy(&app, &state, lifecycle, Some(&trace))?;

    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
    let sport = cfg.sandbox_port;

    let sbx_home = sandbox_home();
    let auth_dir = sbx_home.join(".claude-science");
    ensure_sandbox_runtime_permissions(&auth_dir);

    if sandbox_running_ours(sport) {
        if oauth_forge::login_intact(&auth_dir, "virtual@localhost.invalid", &sbx_home) {
            // Refresh Skill/MCP deployment against the current inventory. Science
            // only reads these at launch, so if anything actually changed we must
            // restart the sandbox for the change to take effect — otherwise the
            // user would silently keep running the old config. Deploys are
            // idempotent, so an unchanged reopen stays fast.
            let (_, skill_changed) = deploy_sandbox_skills(&auth_dir, &sbx_home);
            let (_, mcp_changed) = deploy_sandbox_mcp(&auth_dir, &sbx_home);
            if !skill_changed && !mcp_changed {
                let url = sandbox_url(sport);
                {
                    let mut st = lock(&state);
                    st.sandbox_port = sport;
                    st.sandbox_url = Some(url.clone());
                }
                let _ = open_in_browser(&url);
                trace.finish(format!(
                    "ok action=reopened proxy_action={}",
                    proxy_action.as_str()
                ));
                return Ok(json!({
                    "url": url,
                    "action": "reopened"
                }));
            }
            // Config changed → tear down and fall through to a full relaunch.
            let mut st = lock(&state);
            let _ = stop_sandbox_state(&app, &mut st);
        } else {
            let mut st = lock(&state);
            let _ = stop_sandbox_state(&app, &mut st);
        }
    }

    let root = asset_root(&app).ok_or_else(|| i18n_err("errSandboxScriptMissing", json!({})))?;

    trace.stage(OperationStage::SandboxLogin, "ensure_virtual_login");
    let (forged, login_action) =
        oauth_forge::ensure_virtual_login(&auth_dir, "virtual@localhost.invalid", &sbx_home)
            .map_err(|e| i18n_err("errSandboxVirtualLoginFailed", json!({ "error": e })))?;

    // Deploy enabled Skills into the sandbox Science skills dir
    // (`<data-dir>/skills/<name>/`, confirmed against the installed app). The
    // deployer only manages folders it marks with `.csp_managed`, so Science's
    // bundled Skills are never removed, and it refuses to write outside the
    // sandbox or into the real `~/.claude-science`.
    let (skill_report, _) = deploy_sandbox_skills(&auth_dir, &sbx_home);

    // Deploy enabled local stdio MCP servers into the sandbox
    // (`<data-dir>/mcp/local-mcp.json` + `[sandbox] user_read_paths` in
    // `config.toml`, confirmed against a live sandbox). Same iron-rule guards as
    // Skills: only ever writes under the sandbox, never the real `~/.claude-science`.
    let (mcp_report, _) = deploy_sandbox_mcp(&auth_dir, &sbx_home);

    let launch = root.join("scripts/sandbox/launch-virtual-sandbox.sh");
    if !launch.is_file() {
        return Err(i18n_err("errSandboxScriptMissing", json!({})));
    }

    let proxy_url = format!("http://127.0.0.1:{pport}/{secret}");
    let logf = open_log("sandbox.log")
        .map_err(|e| i18n_err("errSandboxLogOpenFailed", json!({ "error": e.to_string() })))?;
    {
        use std::io::Write;
        let mut lw = &logf;
        let _ = writeln!(
            lw,
            "[oauth] 虚拟登录已就绪（Rust，零 node；action={:?}）：auth_dir={} account={} org={} enc={}",
            login_action,
            forged.auth_dir.display(),
            forged.account_uuid,
            forged.org_uuid,
            forged.enc_file.display()
        );
        let _ = writeln!(lw, "[skill] {skill_report}");
        let _ = writeln!(lw, "[mcp] {mcp_report}");
    }
    let logf2 = logf.try_clone().map_err(|e| e.to_string())?;
    trace.stage(OperationStage::SandboxLaunch, format!("port={sport}"));
    let status = Command::new("zsh")
        .arg(&launch)
        .arg("--port")
        .arg(sport.to_string())
        .arg("--proxy-url")
        .arg(&proxy_url)
        .arg("--skip-oauth-forge")
        .env("SANDBOX_HOME", sandbox_home())
        .stdout(Stdio::from(logf))
        .stderr(Stdio::from(logf2))
        .status()
        .map_err(|e| i18n_err("errSandboxSpawnFailed", json!({ "error": e.to_string() })))?;
    if !status.success() {
        let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
        trace.finish("error=sandbox_launch_failed");
        return Err(i18n_err(
            "errSandboxLaunchScriptFailed",
            json!({ "tail": tail }),
        ));
    }

    let mut ok = false;
    for _ in 0..(operation::SANDBOX_HEALTH_BUDGET_MS / POLL_INTERVAL_MS) {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        if proc::http_health(sport, None, operation::LOCAL_HEALTH_TIMEOUT_MS) {
            ok = true;
            break;
        }
    }
    trace.stage(
        OperationStage::SandboxHealth,
        if ok { "ready" } else { "not_ready" },
    );
    if !ok {
        let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
        {
            let mut st = lock(&state);
            let _ = stop_sandbox_state(&app, &mut st);
        }
        trace.finish("error=sandbox_health_timeout");
        return Err(i18n_err(
            "errSandboxHealthTimeout",
            json!({ "port": sport, "tail": tail }),
        ));
    }

    if !sandbox_running_ours(sport) {
        {
            let mut st = lock(&state);
            let _ = stop_sandbox_state(&app, &mut st);
        }
        trace.finish("error=sandbox_identity_mismatch");
        return Err(i18n_err(
            "errSandboxIdentityMismatch",
            json!({ "port": sport }),
        ));
    }

    // Built-in probe: once Science is up it scans `<data-dir>/skills/` and writes a
    // `.catalog_stamp` into each folder it recognizes. Log how many of our managed
    // Skills got stamped so a real launch self-verifies the deployment path.
    // `skill_report` is the wrapper's summary, e.g. `deploy: deployed=[...] ...`.
    // Only probe when at least one Skill was actually deployed.
    if !skill_report.starts_with("deploy: deployed=[]") {
        let verify = verify_sandbox_skills(&auth_dir);
        if let Ok(logf3) = open_log("sandbox.log") {
            use std::io::Write;
            let mut lw = &logf3;
            let _ = writeln!(lw, "[skill] {verify}");
        }
    }

    let url = sandbox_url(sport);
    {
        let mut st = lock(&state);
        st.sandbox_port = sport;
        st.sandbox_url = Some(url.clone());
    }
    let _ = open_in_browser(&url);
    trace.stage(OperationStage::OpenBrowser, "done");
    trace.finish(format!(
        "ok action=started proxy_action={}",
        proxy_action.as_str()
    ));
    Ok(json!({
        "url": url,
        "action": "started"
    }))
}

/// Deploy enabled Skills into the sandbox. Returns a redaction-safe log summary
/// and whether anything on disk changed (for restart decisions). Never fails the
/// launch: any error is reported and the sandbox still starts.
fn deploy_sandbox_skills(auth_dir: &Path, sbx_home: &Path) -> (String, bool) {
    let real_science = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".claude-science"))
        .unwrap_or_else(|| PathBuf::from("/nonexistent/.claude-science"));
    let store = match crate::skill_manager::SkillStore::open() {
        Ok(s) => s,
        Err(e) => return (format!("deploy skipped: store open failed: {e}"), false),
    };
    let enabled = match store.enabled_skills() {
        Ok(e) => e,
        Err(e) => return (format!("deploy skipped: list failed: {e}"), false),
    };
    match crate::skill_manager::deploy_enabled_skills(&enabled, auth_dir, sbx_home, &real_science) {
        Ok(r) => (
            format!(
                "deploy: deployed={:?} skipped={:?} removed={} changed={}",
                r.deployed, r.skipped, r.removed, r.changed
            ),
            r.changed,
        ),
        Err(e) => (format!("deploy error (sandbox launch continues): {e}"), false),
    }
}

/// Deploy enabled local stdio MCP servers into the sandbox. Returns a
/// redaction-safe log summary and whether anything on disk changed (for restart
/// decisions). Never fails the launch: any error is reported and the sandbox
/// still starts.
fn deploy_sandbox_mcp(auth_dir: &Path, sbx_home: &Path) -> (String, bool) {
    let real_science = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".claude-science"))
        .unwrap_or_else(|| PathBuf::from("/nonexistent/.claude-science"));
    let store = match crate::mcp_manager::McpStore::open() {
        Ok(s) => s,
        Err(e) => return (format!("deploy skipped: store open failed: {e}"), false),
    };
    let enabled = match store.enabled_servers() {
        Ok(e) => e,
        Err(e) => return (format!("deploy skipped: list failed: {e}"), false),
    };
    match crate::mcp_manager::deploy_enabled_mcp(&enabled, auth_dir, sbx_home, &real_science) {
        Ok(r) => (
            format!(
                "deploy: servers={:?} granted_paths={} cleared={} changed={}",
                r.deployed, r.granted_paths, r.cleared, r.changed
            ),
            r.changed,
        ),
        Err(e) => (format!("deploy error (sandbox launch continues): {e}"), false),
    }
}

/// Scan `<data-dir>/skills/` for our managed Skills and report how many Science
/// recognized (folders carrying both our `.csp_managed` marker and Science's
/// `.catalog_stamp`). Best-effort observability; not a launch gate.
fn verify_sandbox_skills(auth_dir: &Path) -> String {
    let skills_dir = auth_dir.join("skills");
    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return "verify: skills dir unreadable".to_string();
    };
    let (mut managed, mut stamped) = (0u32, 0u32);
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() && p.join(".csp_managed").is_file() {
            managed += 1;
            if p.join(".catalog_stamp").is_file() {
                stamped += 1;
            }
        }
    }
    format!("verify: managed={managed} recognized_by_science={stamped}")
}
