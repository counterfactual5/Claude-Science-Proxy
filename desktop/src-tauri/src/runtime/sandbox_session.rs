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
            let org_uuid = read_sandbox_org_uuid(&auth_dir);
            let (_, skill_changed) =
                deploy_sandbox_skills(&auth_dir, &sbx_home, org_uuid.as_deref());
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

    // Deploy enabled Skills into the sandbox org-scoped Science skills dir
    // (`<data-dir>/orgs/<org_uuid>/skills/<name>/`). Current Science builds stamp
    // and index org-scoped skills; root `<data-dir>/skills/` is not recognized.
    // The deployer only manages folders it marks with `.csp_managed`, so Science's
    // bundled Skills are never removed, and it refuses to write outside the
    // sandbox or into the real `~/.claude-science`.
    let (skill_report, _) = deploy_sandbox_skills(&auth_dir, &sbx_home, Some(&forged.org_uuid));

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

    // Built-in probe: once Science is up it scans the org-scoped skills dir and
    // writes a `.catalog_stamp` into each folder it recognizes. Log how many of our
    // managed Skills got stamped so a real launch self-verifies the deployment path.
    // `skill_report` is the wrapper's summary, e.g. `deploy: org=... deployed=[...] ...`.
    // Only probe when at least one Skill was actually deployed.
    if !skill_report.contains("deployed=[]") {
        let verify = verify_sandbox_skills(&auth_dir, Some(&forged.org_uuid));
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
pub(crate) fn deploy_sandbox_skills(
    auth_dir: &Path,
    sbx_home: &Path,
    org_uuid: Option<&str>,
) -> (String, bool) {
    let Some(org_uuid) = org_uuid else {
        return (
            "deploy skipped: org uuid unavailable (active-org.json missing/invalid)".to_string(),
            false,
        );
    };
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
    let org_data_dir = auth_dir.join("orgs").join(org_uuid);
    match crate::skill_manager::deploy_enabled_skills(
        &enabled,
        &org_data_dir,
        sbx_home,
        &real_science,
    ) {
        Ok(r) => (
            format!(
                "deploy: org={} deployed={:?} skipped={:?} removed={} legacy_removed={} changed={}",
                org_uuid,
                r.deployed,
                r.skipped,
                r.removed,
                cleanup_legacy_root_skills(auth_dir),
                r.changed
            ),
            r.changed,
        ),
        Err(e) => (
            format!("deploy error (sandbox launch continues): {e}"),
            false,
        ),
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
    let mut enabled = match store.enabled_servers() {
        Ok(e) => e,
        Err(e) => return (format!("deploy skipped: list failed: {e}"), false),
    };
    // Built-in connectors (e.g. `web-search`) carry a bundled script that must
    // exist on disk, and their interpreter/script path are re-resolved here so
    // the entry self-heals against the current sandbox Python layout. Also
    // force-refresh the built-in description so already-seeded inventories pick
    // up upgraded tool-name guidance (command/args already rewritten here).
    // If the embedded script bytes change, treat deploy as changed so a running
    // sandbox is restarted — MCP children keep the old script in memory.
    let mut script_rewritten = false;
    if enabled.iter().any(|s| s.builtin) {
        use crate::mcp_manager::builtin;
        let mcp_dir = auth_dir.join("mcp");
        let _ = std::fs::create_dir_all(&mcp_dir);
        let (script, rewritten) = match builtin::write_web_search_server(&mcp_dir) {
            Some((path, rewritten)) => (path, rewritten),
            None => (builtin::web_search_script_path(sbx_home), false),
        };
        script_rewritten = rewritten;
        let script_str = script.to_string_lossy().into_owned();
        let python = builtin::resolve_sandbox_python(sbx_home)
            .map(|p| p.to_string_lossy().into_owned());
        for s in enabled.iter_mut().filter(|s| s.builtin) {
            if s.name == builtin::BUILTIN_WEB_SEARCH_NAME {
                s.description = builtin::BUILTIN_WEB_SEARCH_DESCRIPTION.to_string();
            }
            if let Some(py) = &python {
                s.command = py.clone();
            }
            s.args = vec![script_str.clone()];
        }
    }
    // Prefill Science network grants for built-in web-search providers (+ user
    // extensions in ~/.csp/network-allowlist.json). New grants need a Science
    // restart for Operon to honour them — OR into `changed` like script rewrites.
    let (allowlist_report, allowlist_changed) =
        crate::mcp_manager::network_allowlist::apply_best_effort(auth_dir);

    match crate::mcp_manager::deploy_enabled_mcp(&enabled, auth_dir, sbx_home, &real_science) {
        Ok(r) => {
            let changed = r.changed || script_rewritten || allowlist_changed;
            (
                format!(
                    "deploy: servers={:?} granted_paths={} cleared={} changed={} script_rewritten={}; {}",
                    r.deployed,
                    r.granted_paths,
                    r.cleared,
                    changed,
                    script_rewritten,
                    allowlist_report
                ),
                changed,
            )
        }
        Err(e) => (
            format!(
                "deploy error (sandbox launch continues): {e}; {allowlist_report}"
            ),
            allowlist_changed,
        ),
    }
}

/// Scan `<data-dir>/orgs/<org_uuid>/skills/` for our managed Skills and report how many Science
/// recognized (folders carrying both our `.csp_managed` marker and Science's
/// `.catalog_stamp`). Best-effort observability; not a launch gate.
///
/// Caveat (audited on Science `0.1.17-dev`): the `.catalog_stamp` is written by
/// Science **once**, during the initial org catalog build — every bundled Skill's
/// stamp shares one identical timestamp/value — and later-added folders are **not**
/// re-stamped on subsequent launches (a CSP-managed Skill deployed after that first
/// build stays unstamped across relaunches). So `recognized_by_science=0` for a
/// Skill added to an already-initialized org is a **false-negative of this stamp
/// heuristic**, not proof Science can't load it: Science's live catalog is tracked
/// in `orgs/<org>/operon-cli.db`, and disk Skills are read by relevance regardless
/// of the stamp. It is NOT a SKILL.md frontmatter/format issue — `crypto-data` has
/// valid `name`/`description` frontmatter (identical in shape to recognized bundled
/// Skills) yet is unstamped. On a **fresh** org, CSP deploys before Science's first
/// catalog build, so the built-in Skill is stamped and recognized normally.
fn verify_sandbox_skills(auth_dir: &Path, org_uuid: Option<&str>) -> String {
    let Some(org_uuid) = org_uuid else {
        return "verify: org uuid unavailable".to_string();
    };
    let skills_dir = auth_dir.join("orgs").join(org_uuid).join("skills");
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
    format!("verify: org={org_uuid} managed={managed} recognized_by_science={stamped}")
}

fn read_sandbox_org_uuid(auth_dir: &Path) -> Option<String> {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(auth_dir.join("active-org.json")).ok()?)
            .ok()?;
    let org = v.get("org_uuid")?.as_str()?;
    if org.len() == 36 && org.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        Some(org.to_string())
    } else {
        None
    }
}

/// Remove CSP-managed Skills from the legacy root-level path used by earlier
/// builds. Current Science indexes org-scoped Skills, so these folders only cause
/// confusion in diagnostics. Bundled/unmanaged folders are never touched.
fn cleanup_legacy_root_skills(auth_dir: &Path) -> usize {
    let root_skills = auth_dir.join("skills");
    let Ok(entries) = std::fs::read_dir(&root_skills) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir()
            && p.join(".csp_managed").is_file()
            && std::fs::remove_dir_all(&p).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

/// Redeploy enabled Skills into the sandbox data-dir. Returns whether disk changed.
pub(crate) fn redeploy_sandbox_skills() -> bool {
    let sbx_home = sandbox_home();
    let auth_dir = sbx_home.join(".claude-science");
    let org_uuid = read_sandbox_org_uuid(&auth_dir);
    let (_, changed) = deploy_sandbox_skills(&auth_dir, &sbx_home, org_uuid.as_deref());
    changed
}

/// Redeploy enabled local MCP servers into the sandbox data-dir. Returns whether
/// disk changed (mirrors [`redeploy_sandbox_skills`]).
pub(crate) fn redeploy_sandbox_mcp() -> bool {
    let sbx_home = sandbox_home();
    let auth_dir = sbx_home.join(".claude-science");
    let (_, changed) = deploy_sandbox_mcp(&auth_dir, &sbx_home);
    changed
}

/// If our sandbox is running, stop it so the caller can flag `needs_restart` and
/// the frontend can `one_click_login` again. Used after Skill/MCP redeploys.
pub(crate) fn stop_running_sandbox_for_redeploy<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
) -> Result<bool, String> {
    let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
    if !sandbox_running_ours(cfg.sandbox_port) {
        return Ok(false);
    }
    let mut st = lock(state);
    stop_sandbox_state(app, &mut st)?;
    Ok(true)
}
