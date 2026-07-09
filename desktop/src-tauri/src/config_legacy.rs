//! Read-only copy of legacy fixed-slot config (schema v1) for one-time migration reads. Production code never writes this.
//!
//! v1 = PR #4 fixed-slot layout: top-level `provider` pointer + `providers: {slot -> {key, base_url, model}}`.
//! v2 ([`crate::config`]) uses user-named `profiles` + `active_id` pointer.
//! Migration reads here, writes the new shape, then discards; these types never participate in saves.
use serde::Deserialize;
use std::collections::BTreeMap;

/// Legacy per-slot config (equivalent to old `config::ProviderCfg`). All fields optional for older files.
#[derive(Deserialize, Clone, Default)]
pub struct ProviderCfgV1 {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
}

/// Legacy top-level config (equivalent to old `config::Config`). Port/mode defaults match the new config helpers.
#[derive(Deserialize, Clone)]
pub struct ConfigV1 {
    #[serde(default)]
    pub provider: String,
    #[serde(default = "crate::config::default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default = "crate::config::default_sandbox_port")]
    pub sandbox_port: u16,
    #[serde(default)]
    pub secret: String,
    #[serde(default = "crate::config::default_mode")]
    pub mode: String,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderCfgV1>,
}
