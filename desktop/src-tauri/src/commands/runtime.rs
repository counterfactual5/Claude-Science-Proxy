use serde::Deserialize;
use serde_json::json;
use tauri::State;

use crate::runtime::i18n::i18n_err;
use crate::runtime::science::{settings_change_needs_teardown, stop_sandbox};
use crate::runtime::settings::validate_runtime_ports;
use crate::{config, lock, run_blocking, AppState, SharedAppState, SharedLifecycle};

fn stop_sandbox_state(app: &tauri::AppHandle, st: &mut AppState) -> Result<(), String> {
    stop_sandbox(app, &mut st.sandbox, &mut st.sandbox_url)
}

#[derive(Deserialize)]
pub(crate) struct UiSettings {
    proxy_port: u16,
    sandbox_port: u16,
}

/// Port settings (provider/connection changes go through profile CRUD + set_active_profile).
/// Serialized (fix P1-c): when ports change, the running proxy binds the old port and the running sandbox bakes in the old proxy URL,
/// which no longer matches the new port; this tears down that stale chain (stop our sandbox only, never touch 8765), forcing the next
/// one-click start to rebuild on the new port—avoiding "reuse old sandbox pointing at dead port while UI says unchanged".
#[tauri::command]
pub(crate) async fn set_settings(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    cfg: UiSettings,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || set_settings_inner(app, state, lifecycle, cfg)).await
}

fn set_settings_inner(
    app: tauri::AppHandle,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    cfg: UiSettings,
) -> Result<(), String> {
    validate_runtime_ports(cfg.proxy_port, cfg.sandbox_port)?;
    lifecycle.with_serialized(|| {
        let dir = config::default_dir();
        let old = config::load_from(&dir).map_err(|e| e.to_string())?;
        let teardown = settings_change_needs_teardown(
            old.proxy_port,
            cfg.proxy_port,
            old.sandbox_port,
            cfg.sandbox_port,
        );
        // Tear down chain **before** persist; sandbox stop result must be handled (incremental P1 fix): if stop fails, **do not change ports**—
        // otherwise config has new ports while old sandbox still points at old proxy → inconsistent; next one-click would reuse dead chain.
        // Keeping ports unchanged keeps everything consistent (old sandbox → old proxy port; next one-click rebuilds proxy on old port).
        if teardown {
            let mut st = lock(&state);
            stop_sandbox_state(&app, &mut st)
                .map_err(|e| i18n_err("errPortSandboxStopFailed", json!({ "error": e })))?;
            lifecycle.bump_generation(); // After successful stop, invalidate in-flight starts
            st.stop_proxy();
        }
        // Chain torn down (or not needed) → persist new ports so config matches runtime.
        config::update(&dir, move |c| {
            c.proxy_port = cfg.proxy_port;
            c.sandbox_port = cfg.sandbox_port;
        })
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[derive(Deserialize)]
pub(crate) struct FetchModelsReq {
    /// Template id (controls builtin / base_url editability / default base_url).
    template_id: String,
    /// Actual api_format when editing a stored profile; empty uses template default.
    #[serde(default)]
    api_format: Option<String>,
    /// User-supplied base_url for custom templates (ignored for non-editable templates).
    #[serde(default)]
    base_url: String,
    /// New key from user; empty means keep stored key for profile_id (backend never returns full key).
    #[serde(default)]
    key: String,
    /// Stored profile id when editing (used to keep existing key).
    #[serde(default)]
    profile_id: Option<String>,
}

/// Fetch available models — pure scratch probe: temp proxy only, candidate base_url/key `/v1/models`,
/// never writes config, never mutates AppState, never touches the production proxy serving Science.
#[tauri::command]
pub(crate) async fn fetch_models(
    app: tauri::AppHandle,
    req: FetchModelsReq,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        crate::runtime::model_discovery::fetch_models(
            app,
            crate::runtime::model_discovery::ModelDiscoveryRequest {
                template_id: req.template_id,
                api_format: req.api_format,
                base_url: req.base_url,
                key: req.key,
                profile_id: req.profile_id,
            },
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn stop_all(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || stop_all_inner_cmd(app, state, lifecycle)).await
}

fn stop_all_inner_cmd(
    app: tauri::AppHandle,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
) -> Result<(), String> {
    lifecycle.with_serialized(|| {
        lifecycle.bump_generation(); // Invalidate any in-flight start (prevent revive with old key after stop)
        let mut st = lock(&state);
        let sandbox_res = stop_sandbox_state(&app, &mut st);
        st.stop_proxy();
        sandbox_res.map_err(|e| i18n_err("errStopSandboxFailed", json!({ "error": e })))
    })
}

#[tauri::command]
pub(crate) async fn one_click_login(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || one_click_login_cmd(app, state, lifecycle)).await
}

fn one_click_login_cmd(
    app: tauri::AppHandle,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
) -> Result<serde_json::Value, String> {
    lifecycle.with_serialized(|| {
        crate::runtime::sandbox_session::one_click_login(app, state, lifecycle.as_ref())
    })
}

/// Compact runtime lights for the panel status row (`proxy` / `sandbox`: `green` | `amber`).
#[tauri::command]
pub(crate) async fn get_runtime_status(
    state: State<'_, SharedAppState>,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    run_blocking(move || Ok(crate::runtime::diagnostics::runtime_status_snapshot(&state))).await
}

#[cfg(test)]
mod tests {
    use crate::runtime::diagnostics::runtime_status_snapshot;
    use crate::{
        config::{self, Config, Profile},
        lifecycle, lock,
        runtime::{sandbox_session, science},
        AppState, SharedAppState,
    };
    use std::{
        env, fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct EnvGuard {
        saved: Vec<(String, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self { saved: Vec::new() }
        }

        fn set(&mut self, key: &str, value: impl AsRef<std::ffi::OsStr>) {
            self.saved.push((key.to_string(), env::var_os(key)));
            env::set_var(key, value);
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.saved.iter().rev() {
                match value {
                    Some(v) => env::set_var(key, v),
                    None => env::remove_var(key),
                }
            }
        }
    }

    fn tmpdir(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("csp-{label}-{}-{now}", std::process::id()))
    }

    fn free_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_ne!(port, 8765);
        port
    }

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn write_test_bins(dir: &Path) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        write_executable(
            &dir.join("open"),
            r#"#!/bin/sh
if [ -n "${CSP_FAKE_OPEN_LOG:-}" ]; then
  printf '%s\n' "$*" >> "$CSP_FAKE_OPEN_LOG"
fi
exit 0
"#,
        );
        write_executable(
            &dir.join("security"),
            r#"#!/bin/sh
exit 0
"#,
        );
        let science_bin = dir.join("claude-science");
        write_executable(
            &science_bin,
            r#"#!/bin/sh
set -eu
cmd="${1:-}"
if [ "$#" -gt 0 ]; then shift; fi
data_dir=""
port=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --data-dir) data_dir="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    *) shift ;;
  esac
done
state="$data_dir/fake-science"
mkdir -p "$state"
case "$cmd" in
  serve)
    count="$(cat "$state/serve-count" 2>/dev/null || echo 0)"
    count=$((count + 1))
    printf '%s' "$count" > "$state/serve-count"
    printf '%s' "$port" > "$state/port"
    python3 - "$port" "$state/pid" >/dev/null 2>&1 <<'PY' &
import http.server
import os
import socketserver
import sys
port = int(sys.argv[1])
pidfile = sys.argv[2]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def do_GET(self):
        if self.path.startswith("/health"):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"fake science")
with open(pidfile, "w", encoding="utf-8") as f:
    f.write(str(os.getpid()))
with socketserver.TCPServer(("127.0.0.1", port), Handler) as httpd:
    httpd.serve_forever()
PY
    exit 0
    ;;
  status)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      echo '{"running":true}'
    else
      echo '{"running":false}'
      exit 1
    fi
    ;;
  url)
    p="$(cat "$state/port")"
    echo "http://127.0.0.1:$p"
    ;;
  stop)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
    rm -f "$state/pid"
    echo "stopped"
    ;;
  *)
    echo "unsupported fake science command: $cmd" >&2
    exit 2
    ;;
esac
"#,
        );
        science_bin
    }

    fn start_mock_upstream() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_ne!(port, 8765);
        thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut buf = [0; 512];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
            }
        });
        port
    }

    fn wait_http_health(port: u16) {
        for _ in 0..50 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("mock service on port {port} did not become reachable");
    }

    #[test]
    #[ignore = "explicit isolated runtime smoke; uses fake Science and local loopback ports"]
    fn isolated_one_click_reuse_status_smoke_with_fake_science() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let tmp = tmpdir("isolated-runtime-smoke");
        let home = tmp.join("home");
        let bin_dir = tmp.join("bin");
        fs::create_dir_all(&home).unwrap();
        let fake_science = write_test_bins(&bin_dir);
        let open_log = tmp.join("open.log");
        let mock_upstream_port = start_mock_upstream();
        let proxy_port = free_port();
        let sandbox_port = free_port();
        assert_ne!(proxy_port, sandbox_port);

        let mut env_guard = EnvGuard::new();
        env_guard.set("HOME", &home);
        env_guard.set("CSP_REPO", &root);
        env_guard.set("SCIENCE_BIN", &fake_science);
        env_guard.set("CSP_FAKE_OPEN_LOG", &open_log);
        env_guard.set("CSP_DOCTOR_CHECK_REAL_HOME", "0");
        env_guard.set(
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                bin_dir.to_string_lossy()
            ),
        );

        let fake_key = "csp-isolated-fake-key-never-log";
        let profile = Profile {
            id: "mock-relay".into(),
            name: "Mock Relay".into(),
            template_id: "custom".into(),
            category: "custom".into(),
            api_format: "anthropic".into(),
            base_url: format!("http://127.0.0.1:{mock_upstream_port}/anthropic"),
            api_key: fake_key.into(),
            model: "mock-model".into(),
            ..Default::default()
        };
        let cfg = Config {
            profiles: vec![profile],
            active_id: "mock-relay".into(),
            proxy_port,
            sandbox_port,
            ..Default::default()
        };
        let config_dir = config::default_dir();
        config::save_to(&config_dir, &cfg).unwrap();

        let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
        let lifecycle = Arc::new(lifecycle::Lifecycle::new());
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .manage(lifecycle.clone())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let handle = app.handle().clone();

        let first =
            sandbox_session::one_click_login(handle.clone(), state.clone(), lifecycle.as_ref())
                .expect("first one-click should start proxy and sandbox");
        assert_eq!(first["action"], "started");
        assert_eq!(first["url"], format!("http://127.0.0.1:{sandbox_port}"));
        wait_http_health(sandbox_port);
        let fake_state_dir = home
            .join(".csp")
            .join("sandbox")
            .join("home")
            .join(".claude-science")
            .join("fake-science");
        let first_pid = fs::read_to_string(fake_state_dir.join("pid")).unwrap();
        assert_eq!(
            fs::read_to_string(fake_state_dir.join("serve-count")).unwrap(),
            "1"
        );

        let second =
            sandbox_session::one_click_login(handle.clone(), state.clone(), lifecycle.as_ref())
                .expect("second one-click should reuse running sandbox");
        assert_eq!(second["action"], "reopened");
        assert_eq!(second["url"], format!("http://127.0.0.1:{sandbox_port}"));
        assert_eq!(
            fs::read_to_string(fake_state_dir.join("pid")).unwrap(),
            first_pid
        );
        assert_eq!(
            fs::read_to_string(fake_state_dir.join("serve-count")).unwrap(),
            "1"
        );

        let status = runtime_status_snapshot(&state);
        assert_eq!(status["proxy"], "green");
        assert_eq!(status["sandbox"], "green");
        assert_eq!(status["upstream"], "green");
        assert_eq!(status["active_profile"]["id"], "mock-relay");
        assert_eq!(status["science"]["sandbox"]["port"], sandbox_port);
        assert_eq!(status["science"]["auth"]["real_account_verified"], false);
        assert_eq!(status["science"]["auth"]["real_home_verified"], false);
        assert!(status["last_error"].is_null());

        let csp_config = config_dir.join("CSP.json");
        let doctor = std::process::Command::new(root.join("scripts/maintenance/doctor.sh"))
            .env("HOME", &home)
            .env("SCIENCE_BIN", &fake_science)
            .env("CSP_CONFIG", &csp_config)
            .env("CSP_PROXY_PORT", proxy_port.to_string())
            .env("CSP_SANDBOX_PORT", sandbox_port.to_string())
            .output()
            .expect("doctor should run");
        assert!(doctor.status.success());
        let doctor_out = String::from_utf8_lossy(&doctor.stdout);
        assert!(doctor_out.contains("真实 HOME 检查默认跳过"));
        assert!(!doctor_out.contains(&format!("{}/.claude-science", home.display())));

        let cfg_after = config::load_from(&config_dir).unwrap();
        let secret = cfg_after.secret;
        assert!(!secret.is_empty());
        let doctor_err = String::from_utf8_lossy(&doctor.stderr);
        assert!(!doctor_out.contains(fake_key));
        assert!(!doctor_out.contains(&secret));
        assert!(!doctor_err.contains(fake_key));
        assert!(!doctor_err.contains(&secret));
        assert!(!first.to_string().contains(fake_key));
        assert!(!first.to_string().contains(&secret));
        assert!(!second.to_string().contains(fake_key));
        assert!(!second.to_string().contains(&secret));
        let opened = fs::read_to_string(&open_log).unwrap_or_default();
        assert!(!opened.contains(fake_key));
        assert!(!opened.contains(&secret));
        for name in ["proxy.log", "sandbox.log", "operation.log"] {
            let body = fs::read_to_string(config_dir.join("logs").join(name))
                .unwrap_or_else(|e| panic!("expected {name} to exist: {e}"));
            assert!(!body.contains(fake_key), "{name} leaked fake key");
            assert!(!body.contains(&secret), "{name} leaked path secret");
        }

        {
            let mut st = lock(&state);
            let AppState {
                sandbox,
                sandbox_url,
                ..
            } = &mut *st;
            let _ = science::stop_sandbox(&handle, sandbox, sandbox_url);
            st.stop_proxy();
        }
        let _ = fs::remove_dir_all(&tmp);
    }
}
