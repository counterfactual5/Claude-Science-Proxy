//! CSP desktop app backend (process manager).
//!
//! Responsibilities: manage lifecycle of the translation proxy and sandbox Science child processes; read/write
//! `~/.csp/CSP.json` (multi-profile shape); inject third-party keys into the proxy child via **environment variables**
//! (never argv); liveness probes; open sandbox URL in the system browser. Verified privilege/translation logic stays in
//! Python/Node/shell subprocesses to preserve iron-rule guards and verified behavior.
//!
//! Runtime behavior derives adapter (deepseek | relay | openai-custom | openai-responses) from the active
//! profile's `template_id` via the [`templates`] registry, then passes it to the python proxy as `--provider`.
//!
//! Iron rules: keys only in memory and 0600 CSP.json; frontend gets masked keys only; sandbox port/dir guards
//! enforced by invoked scripts (fail-closed on 8765 and real dirs); quitting app stops proxy by default, keeps sandbox.

mod commands;
mod config;
mod config_legacy;
mod lifecycle;
mod mcp_manager;
mod oauth_forge;
mod proc;
mod runtime;
mod scratch;
mod skill_manager;
mod templates;

use std::process::Child;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::Manager;

use runtime::i18n::i18n_err;
use runtime::system::kill_child;

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) proxy: Option<Child>,
    pub(crate) proxy_port: u16,
    pub(crate) secret: String,
    /// Current proxy adapter name (deepseek | relay | openai-custom | openai-responses); used for healthy reuse checks.
    pub(crate) provider: String,
    /// Non-cryptographic fingerprint of the proxy's current key (memory only, never persisted/logged).
    /// Key/upstream change → fingerprint changes → triggers restart to avoid reusing a proxy with stale config.
    pub(crate) key_fp: u64,
    pub(crate) sandbox: Option<Child>,
    pub(crate) sandbox_port: u16,
    pub(crate) sandbox_url: Option<String>,
}

impl AppState {
    pub(crate) fn clear_proxy_identity(&mut self) {
        self.secret.clear();
        self.provider.clear();
        self.key_fp = 0;
    }

    pub(crate) fn stop_proxy(&mut self) {
        kill_child(&mut self.proxy);
        self.clear_proxy_identity();
    }
}

pub(crate) type SharedAppState = Arc<Mutex<AppState>>;
pub(crate) type SharedLifecycle = Arc<lifecycle::Lifecycle>;

/// Lock and recover from poison: a panicking thread holding the lock must not deadlock the whole app.
pub(crate) fn lock(m: &Mutex<AppState>) -> std::sync::MutexGuard<'_, AppState> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) async fn run_blocking<T>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| {
        i18n_err(
            "errBackgroundTaskFailed",
            json!({ "detail": e.to_string() }),
        )
    })?
}

// ---------- Entry ----------
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(AppState::default())))
        .manage(Arc::new(lifecycle::Lifecycle::new()))
        .invoke_handler(tauri::generate_handler![
            commands::profiles::get_config,
            commands::runtime::set_settings,
            commands::profiles::create_profile,
            commands::profiles::update_profile_metadata,
            commands::profiles::update_profile_connection,
            commands::profiles::delete_profile,
            commands::profiles::set_active_profile,
            commands::profiles::open_csp_json,
            commands::runtime::fetch_models,
            commands::runtime::stop_all,
            commands::runtime::one_click_login,
            commands::skills::list_skills,
            commands::skills::discover_skills,
            commands::skills::inspect_skill_source,
            commands::skills::import_skill,
            commands::skills::create_skill,
            commands::skills::set_skill_enabled,
            commands::skills::remove_skill,
            commands::skills::discover_workspace_skills,
            commands::skills::adopt_workspace_skills,
            commands::skills::open_skill_folder,
            commands::skills::open_skill_file,
            commands::mcp::list_mcp_servers,
            commands::mcp::open_mcp_inventory_json,
            commands::mcp::open_network_allowlist_json,
            commands::mcp::discover_mcp_servers,
            commands::mcp::import_discovered_mcp_server,
            commands::mcp::inspect_mcp_server,
            commands::mcp::create_mcp_server,
            commands::mcp::update_mcp_server,
            commands::mcp::set_mcp_server_enabled,
            commands::mcp::remove_mcp_server,
        ])
        .setup(|app| {
            // Normal desktop app: Dock icon, standard lifecycle. Window in tauri.conf.json has
            // decorations + visible + center for centered popup on launch. Tray icon removed.

            // Eager load on startup: if legacy v1 fixed-slot file, v1→v2 migration + persist + .v1.bak here;
            // dangling active normalized to empty. Migration merged into config::load_from (no separate relay_presets).
            let _ = config::load_from(&config::default_dir());

            // Seed the built-in, no-key `web-search` MCP connector on first run so
            // Claude Science has real web search/fetch out of the box (Anthropic's
            // hosted web_search is unavailable under CSP virtual login). One-time,
            // sentinel-guarded: a user who later disables/removes it is respected.
            mcp_manager::seed_builtin_connectors();

            // Ensure ~/.csp/network-allowlist.json exists so users can extend
            // Science egress domains (built-in web-search hosts are merged on Start).
            let _ = mcp_manager::network_allowlist::ensure_user_file();

            // Seed the built-in `csp-environment` standing-guidance Skill on
            // first run (enabled by default) so Claude Science gets the local
            // environment handbook (web lanes, filesystem, CJK, skills/env)
            // every session. Migrates legacy `csp-web-access` installs while
            // respecting sticky opt-out. Sentinel-guarded and self-healing,
            // mirroring the MCP seed.
            skill_manager::seed_builtin_skills();

            // Close window → quit: stop proxy, clear secret, keep sandbox running (spec §5.1).
            if let Some(win) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::CloseRequested { .. } = ev {
                        let state = handle.state::<SharedAppState>();
                        let mut st = lock(state.inner());
                        st.stop_proxy();
                        handle.exit(0);
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use crate::runtime::system::redact;
    use crate::AppState;

    #[test]
    fn app_state_clear_proxy_identity_removes_runtime_credentials() {
        let mut st = AppState {
            secret: "secret".into(),
            provider: "deepseek".into(),
            key_fp: 42,
            ..AppState::default()
        };
        st.clear_proxy_identity();
        assert!(st.secret.is_empty());
        assert!(st.provider.is_empty());
        assert_eq!(st.key_fp, 0);
    }

    #[test]
    fn redact_scrubs_secret_and_is_noop_when_empty() {
        assert_eq!(
            redact("推理指向 http://127.0.0.1:18991/abcd1234 尾巴", "abcd1234"),
            "推理指向 http://127.0.0.1:18991/**** 尾巴"
        );
        assert_eq!(redact("原样返回", ""), "原样返回");
        assert!(!redact("leak abcd1234 leak abcd1234", "abcd1234").contains("abcd1234"));
    }
}
