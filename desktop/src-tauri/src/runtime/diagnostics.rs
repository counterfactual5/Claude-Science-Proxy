//! Runtime status lights and diagnostic snapshot for the panel status row and unit tests.
#![allow(dead_code)]

use serde_json::json;

use crate::runtime::capability_catalog::diagnostics_for_profile;
use crate::runtime::operation;
use crate::runtime::profile::profile_capabilities;
use crate::runtime::provider::{
    adapter_for_profile, current_shim_mode_for_adapter, gateway_kind_for_adapter, upstream_endpoint,
};
use crate::{config, lock, proc, SharedAppState};

pub(crate) fn config_last_error_json(error: &dyn std::fmt::Display) -> serde_json::Value {
    json!({
        "type": "config_error",
        "message": error.to_string(),
    })
}

pub(crate) fn status_response_for_config_error(error: &dyn std::fmt::Display) -> serde_json::Value {
    build_status_response(
        status_lights(StatusProbeInput {
            proxy_ok: false,
            sandbox_ok: false,
            upstream_ok: false,
            egress_ok: false,
        }),
        serde_json::Value::Null,
        "",
        "off",
        diagnostics_for_profile(None, "off"),
        science_diagnostics(ScienceDiagnosticsInput {
            sandbox_port: 0,
            sandbox_ok: false,
        }),
        egress_diagnostics(&crate::proc::EgressDnsProbe {
            ok: false,
            fake_ip: false,
            host: String::new(),
            sample: String::new(),
        }),
        Some(config_last_error_json(error)),
    )
}

/// Runtime status snapshot (`proxy` / `sandbox` / `upstream` lights). Exposed via `get_runtime_status`.
pub(crate) fn runtime_status_snapshot(state: &SharedAppState) -> serde_json::Value {
    let (pport, secret, sport, adapter, base_url, active_profile, catalog_profile) = {
        let st = lock(state);
        let cfg = match config::load_from(&config::default_dir()) {
            Ok(cfg) => cfg,
            Err(e) => return status_response_for_config_error(&e),
        };
        let pport = if st.proxy_port != 0 {
            st.proxy_port
        } else {
            cfg.proxy_port
        };
        let sport = if st.sandbox_port != 0 {
            st.sandbox_port
        } else {
            cfg.sandbox_port
        };
        // Profile mode: the exclusive active profile.
        // Platter mode: active_profile() is None by design; surface the first platter
        // host profile so status/upstream lights still reflect a real endpoint.
        let (adapter, base_url, active_profile, catalog_profile) =
            if let Some(p) = cfg.active_profile() {
                let adapter = adapter_for_profile(p).to_string();
                (
                    adapter,
                    p.base_url.clone(),
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "template_id": p.template_id,
                        "api_format": p.api_format,
                        "model": p.model,
                        "capabilities": profile_capabilities(p),
                    }),
                    Some(p.clone()),
                )
            } else if cfg.is_platter_active() {
                let host = cfg
                    .model_platter
                    .entries
                    .first()
                    .and_then(|e| cfg.profile_by_id(&e.profile_id));
                match host {
                    Some(p) => {
                        let entry_model = cfg
                            .model_platter
                            .entries
                            .first()
                            .map(|e| e.model.as_str())
                            .unwrap_or(p.model.as_str());
                        let n = cfg.model_platter.entries.len();
                        let providers = cfg
                            .model_platter
                            .entries
                            .iter()
                            .map(|e| e.profile_id.as_str())
                            .collect::<std::collections::BTreeSet<_>>()
                            .len();
                        (
                            adapter_for_profile(p).to_string(),
                            p.base_url.clone(),
                            json!({
                                "id": config::PLATTER_ACTIVE_ID,
                                "name": format!("Platter ({n} models · {providers} providers)"),
                                "template_id": p.template_id,
                                "api_format": p.api_format,
                                "model": entry_model,
                                "capabilities": profile_capabilities(p),
                                "mode": "platter",
                                "host_profile_id": p.id,
                            }),
                            Some(p.clone()),
                        )
                    }
                    None => (String::new(), String::new(), serde_json::Value::Null, None),
                }
            } else {
                (String::new(), String::new(), serde_json::Value::Null, None)
            };
        (
            pport,
            st.secret.clone(),
            sport,
            adapter,
            base_url,
            active_profile,
            catalog_profile,
        )
    };
    let upstream = upstream_endpoint(&adapter, &base_url);
    let proxy_ok = !secret.is_empty()
        && proc::http_health(pport, Some(&secret), operation::STATUS_HEALTH_TIMEOUT_MS);
    // Identity-aware: bare /health on the port can be a foreign process.
    let sandbox_ok = crate::runtime::science::sandbox_running_ours(sport);
    let upstream_ok = upstream
        .as_ref()
        .map(|e| proc::tcp_reachable(&e.host, e.port, operation::STATUS_UPSTREAM_TIMEOUT_MS))
        .unwrap_or(false);
    let egress = proc::egress_dns_probe();
    let lights = status_lights(StatusProbeInput {
        proxy_ok,
        sandbox_ok,
        upstream_ok,
        egress_ok: egress.ok,
    });
    let shim_mode = current_shim_mode_for_adapter(&adapter);
    build_status_response(
        lights,
        active_profile,
        gateway_kind_for_adapter(&adapter),
        shim_mode,
        diagnostics_for_profile(catalog_profile.as_ref(), shim_mode),
        science_diagnostics(ScienceDiagnosticsInput {
            sandbox_port: sport,
            sandbox_ok,
        }),
        egress_diagnostics(&egress),
        None,
    )
}

pub(crate) struct StatusProbeInput {
    pub(crate) proxy_ok: bool,
    pub(crate) sandbox_ok: bool,
    pub(crate) upstream_ok: bool,
    pub(crate) egress_ok: bool,
}

pub(crate) struct ScienceDiagnosticsInput {
    pub(crate) sandbox_port: u16,
    pub(crate) sandbox_ok: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct StatusLights {
    pub(crate) proxy: &'static str,
    pub(crate) sandbox: &'static str,
    pub(crate) upstream: &'static str,
    pub(crate) egress: &'static str,
}

fn light(ok: bool) -> &'static str {
    if ok {
        "green"
    } else {
        "amber"
    }
}

/// Preserve the current status contract: each light is either `green` or `amber`.
pub(crate) fn status_lights(input: StatusProbeInput) -> StatusLights {
    StatusLights {
        proxy: light(input.proxy_ok),
        sandbox: light(input.sandbox_ok),
        upstream: light(input.upstream_ok),
        egress: light(input.egress_ok),
    }
}

pub(crate) fn science_diagnostics(input: ScienceDiagnosticsInput) -> serde_json::Value {
    json!({
        "schema_version": 1,
        "sandbox": {
            "port": input.sandbox_port,
            "health": light(input.sandbox_ok),
        },
        "auth": {
            "mode": "virtual_oauth",
            "real_account_verified": false,
            "real_home_verified": false,
            "known_boundary_rule_ids": [
                "science.auth.virtual-oauth-scope-boundary",
                "science.auth.refresh-hardcoded-0_1_15",
            ],
        },
        "version": {
            "status_probe": "not_run_in_status_poll",
            "known_rule_ids": [
                "science.version.0_1_15_dev.route-diff",
                "science.auth.refresh-hardcoded-0_1_15",
            ],
            "note": "status() does not run claude-science binary/version probes; use isolated HOME and non-8765 ports before making Science-version or real-account claims.",
        },
    })
}

pub(crate) fn egress_diagnostics(probe: &crate::proc::EgressDnsProbe) -> serde_json::Value {
    json!({
        "schema_version": 1,
        "health": light(probe.ok),
        "fake_ip": probe.fake_ip,
        "host": if probe.host.is_empty() { serde_json::Value::Null } else { json!(probe.host) },
        "sample": if probe.sample.is_empty() { serde_json::Value::Null } else { json!(probe.sample) },
        "note": if probe.fake_ip {
            "System DNS returned Fake-IP/private ranges; Science Operon denies CONNECT with 403 (private/reserved range). Disable VPN/Clash Fake-IP (or bypass Claude Science), then Stop → Start."
        } else {
            "DNS probe did not see Fake-IP/private ranges for egress sample hosts."
        },
    })
}

pub(crate) fn build_status_response(
    lights: StatusLights,
    active_profile: serde_json::Value,
    gateway_kind: &str,
    shim_mode: &str,
    catalog: serde_json::Value,
    science: serde_json::Value,
    egress: serde_json::Value,
    last_error: Option<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "proxy": lights.proxy,
        "sandbox": lights.sandbox,
        "upstream": lights.upstream,
        "egress": lights.egress,
        "active_profile": active_profile,
        "runtime": {
            "gateway_kind": gateway_kind,
            "shim_mode": shim_mode,
        },
        "catalog": catalog,
        "science": science,
        "egress_detail": egress,
        "last_error": last_error.unwrap_or(serde_json::Value::Null),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_status_response, config_last_error_json, egress_diagnostics, science_diagnostics,
        status_lights, ScienceDiagnosticsInput, StatusProbeInput,
    };
    use serde_json::json;

    #[test]
    fn status_lights_map_bools_to_existing_strings() {
        let all_green = status_lights(StatusProbeInput {
            proxy_ok: true,
            sandbox_ok: true,
            upstream_ok: true,
            egress_ok: true,
        });
        assert_eq!(all_green.proxy, "green");
        assert_eq!(all_green.sandbox, "green");
        assert_eq!(all_green.upstream, "green");
        assert_eq!(all_green.egress, "green");

        let all_amber = status_lights(StatusProbeInput {
            proxy_ok: false,
            sandbox_ok: false,
            upstream_ok: false,
            egress_ok: false,
        });
        assert_eq!(all_amber.proxy, "amber");
        assert_eq!(all_amber.sandbox, "amber");
        assert_eq!(all_amber.upstream, "amber");
        assert_eq!(all_amber.egress, "amber");
    }

    #[test]
    fn status_response_preserves_legacy_lights_and_adds_route_contract() {
        let lights = status_lights(StatusProbeInput {
            proxy_ok: true,
            sandbox_ok: false,
            upstream_ok: true,
            egress_ok: true,
        });
        let v = build_status_response(
            lights,
            json!({
                "id": "p1",
                "name": "GLM",
                "template_id": "glm",
                "api_format": "anthropic",
                "model": "glm-5.2",
            }),
            "python",
            "off",
            json!({
                "schema_version": 1,
                "status": "loaded",
                "active_rules": [],
                "boundary_rules": [],
            }),
            science_diagnostics(ScienceDiagnosticsInput {
                sandbox_port: 8990,
                sandbox_ok: false,
            }),
            egress_diagnostics(&crate::proc::EgressDnsProbe {
                ok: true,
                fake_ip: false,
                host: String::new(),
                sample: String::new(),
            }),
            None,
        );
        assert_eq!(v["proxy"], "green");
        assert_eq!(v["sandbox"], "amber");
        assert_eq!(v["upstream"], "green");
        assert_eq!(v["egress"], "green");
        assert_eq!(v["active_profile"]["template_id"], "glm");
        assert_eq!(v["runtime"]["gateway_kind"], "python");
        assert_eq!(v["runtime"]["shim_mode"], "off");
        assert_eq!(v["catalog"]["schema_version"], 1);
        assert_eq!(v["science"]["schema_version"], 1);
        assert_eq!(v["science"]["sandbox"]["port"], 8990);
        assert_eq!(v["science"]["sandbox"]["health"], "amber");
        assert_eq!(v["science"]["auth"]["real_account_verified"], false);
        assert_eq!(
            v["science"]["version"]["known_rule_ids"][1],
            "science.auth.refresh-hardcoded-0_1_15"
        );
        assert!(v["last_error"].is_null());
    }

    #[test]
    fn config_last_error_json_preserves_typed_config_error() {
        let err = config_last_error_json(&"bad config");
        assert_eq!(
            err.get("type").and_then(|v| v.as_str()),
            Some("config_error")
        );
        assert_eq!(
            err.get("message").and_then(|v| v.as_str()),
            Some("bad config")
        );
    }

    #[test]
    fn platter_active_profile_json_uses_host_profile() {
        // Pure helper contract: platter status surfaces a non-null active_profile
        // with mode=platter and the host profile's template/model (not empty).
        let lights = status_lights(StatusProbeInput {
            proxy_ok: true,
            sandbox_ok: true,
            upstream_ok: true,
            egress_ok: true,
        });
        let active = json!({
            "id": "__platter__",
            "name": "Platter (2 models · 1 providers)",
            "template_id": "glm",
            "api_format": "anthropic",
            "model": "glm-5.2",
            "mode": "platter",
            "host_profile_id": "p1",
        });
        let v = build_status_response(
            lights,
            active,
            "python",
            "off",
            json!({"schema_version": 1}),
            science_diagnostics(ScienceDiagnosticsInput {
                sandbox_port: 8990,
                sandbox_ok: true,
            }),
            egress_diagnostics(&crate::proc::EgressDnsProbe {
                ok: true,
                fake_ip: false,
                host: String::new(),
                sample: String::new(),
            }),
            None,
        );
        assert_eq!(v["active_profile"]["id"], "__platter__");
        assert_eq!(v["active_profile"]["mode"], "platter");
        assert_eq!(v["active_profile"]["model"], "glm-5.2");
        assert_eq!(v["science"]["sandbox"]["health"], "green");
    }

    #[test]
    fn status_response_for_config_error_is_fail_closed() {
        let v = super::status_response_for_config_error(&"bad config");
        assert_eq!(v["proxy"], "amber");
        assert_eq!(v["sandbox"], "amber");
        assert_eq!(v["upstream"], "amber");
        assert_eq!(v["egress"], "amber");
        assert_eq!(v["active_profile"], serde_json::Value::Null);
        assert_eq!(v["science"]["sandbox"]["port"], 0);
        assert_eq!(v["last_error"]["type"], "config_error");
        assert_eq!(v["last_error"]["message"], "bad config");
    }

    #[test]
    fn status_response_can_surface_typed_last_error() {
        let v = build_status_response(
            status_lights(StatusProbeInput {
                proxy_ok: false,
                sandbox_ok: false,
                upstream_ok: false,
                egress_ok: false,
            }),
            serde_json::Value::Null,
            "python",
            "off",
            json!({"schema_version": 1}),
            science_diagnostics(ScienceDiagnosticsInput {
                sandbox_port: 8990,
                sandbox_ok: false,
            }),
            egress_diagnostics(&crate::proc::EgressDnsProbe {
                ok: false,
                fake_ip: true,
                host: "api.duckduckgo.com".into(),
                sample: "198.18.0.141".into(),
            }),
            Some(json!({
                "type": "config_error",
                "message": "config unreadable",
            })),
        );
        assert_eq!(v["last_error"]["type"], "config_error");
        assert_eq!(v["last_error"]["message"], "config unreadable");
        assert_eq!(v["egress"], "amber");
        assert_eq!(v["egress_detail"]["fake_ip"], true);
        assert_eq!(v["egress_detail"]["sample"], "198.18.0.141");
    }
}
