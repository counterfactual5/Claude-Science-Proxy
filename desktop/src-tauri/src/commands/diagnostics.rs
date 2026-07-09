use std::process::Command;

use crate::runtime::provider::adapter_for_profile;
use crate::runtime::system::asset_root;
use crate::{config, run_blocking};

#[tauri::command]
pub(crate) async fn run_doctor(app: tauri::AppHandle) -> Result<String, String> {
    run_blocking(move || run_doctor_inner_cmd(app)).await
}

fn run_doctor_inner_cmd(app: tauri::AppHandle) -> Result<String, String> {
    let root = asset_root(&app).ok_or("找不到 scripts/doctor.sh（打包资源或仓库根均未命中）。")?;
    let cfg = doctor_config_from(&config::default_dir())?;
    let doctor = root.join("scripts/doctor.sh");
    // 生效 profile 的展示名（template_id）+ adapter + 有无 key；无生效配置则留空。
    let (provider_label, adapter, has_key) = match cfg.active_profile() {
        Some(p) => (
            p.template_id.clone(),
            adapter_for_profile(p),
            !p.api_key.is_empty(),
        ),
        None => (String::new(), "", false),
    };
    let mut cmd = Command::new("bash");
    // 多 profile：传 template_id + adapter + key 有无（布尔）。doctor 不再按 provider 名写死、
    // 不再去 shell 环境找 key（key 存 config.json）。绝不把真实 key 值传进其环境。
    cmd.arg(&doctor)
        .env("CSSWITCH_PROVIDER", &provider_label)
        .env("CSSWITCH_ADAPTER", adapter)
        .env("CSSWITCH_KEY_PRESENT", if has_key { "1" } else { "0" })
        .env("CSSWITCH_PROXY_PORT", cfg.proxy_port.to_string())
        .env("CSSWITCH_SANDBOX_PORT", cfg.sandbox_port.to_string());
    let out = cmd.output().map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        text.push_str("\n[stderr] ");
        text.push_str(err.trim());
    }
    Ok(text)
}

fn doctor_config_from(dir: &std::path::Path) -> Result<config::Config, String> {
    config::load_from(dir).map_err(|e| format!("读取配置失败，无法运行自检：{e}"))
}

/// 在访达里打开日志目录 `~/.csswitch/logs`。
#[tauri::command]
pub(crate) fn open_logs() -> Result<(), String> {
    let dir = config::default_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    Command::new("open")
        .arg(&dir)
        .status()
        .map_err(|e| format!("打开日志目录失败：{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::doctor_config_from;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("csswitch-doctor-{name}-{nanos}"))
    }

    #[test]
    fn doctor_config_rejects_reserved_port_instead_of_defaulting() {
        let dir = tmpdir("reserved-port");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("config.json"),
            br#"{"schema_version":2,"profiles":[],"active_id":"","proxy_port":8765,"sandbox_port":8990}"#,
        )
        .unwrap();

        let err = doctor_config_from(&dir).unwrap_err();
        assert!(err.contains("读取配置失败"));
        assert!(err.contains("8765"));
    }
}
