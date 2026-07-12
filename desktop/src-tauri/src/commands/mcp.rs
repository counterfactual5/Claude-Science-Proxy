//! Tauri commands for the local MCP Manager (frontend-facing).

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::mcp_manager::model::{McpInspection, McpServerSummary};
use crate::mcp_manager::store::{McpServerInput, McpStore};
use crate::run_blocking;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerInputDto {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl From<McpServerInputDto> for McpServerInput {
    fn from(d: McpServerInputDto) -> Self {
        McpServerInput {
            name: d.name,
            description: d.description,
            command: d.command,
            args: d.args,
            env: d.env,
        }
    }
}

#[tauri::command]
pub async fn list_mcp_servers() -> Result<Vec<McpServerSummary>, String> {
    run_blocking(|| {
        let store = McpStore::open()?;
        store.list()
    })
    .await
}

#[tauri::command]
pub async fn inspect_mcp_server(input: McpServerInputDto) -> Result<McpInspection, String> {
    run_blocking(move || Ok(McpStore::inspect(&input.into()))).await
}

#[tauri::command]
pub async fn create_mcp_server(input: McpServerInputDto) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.create(input.into())
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateMcpServerInput {
    pub server_id: String,
    pub server: McpServerInputDto,
}

#[tauri::command]
pub async fn update_mcp_server(input: UpdateMcpServerInput) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.update(&input.server_id, input.server.into())
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetMcpEnabledInput {
    pub server_id: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_mcp_server_enabled(input: SetMcpEnabledInput) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.set_enabled(&input.server_id, input.enabled)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveMcpServerInput {
    pub server_id: String,
}

#[tauri::command]
pub async fn remove_mcp_server(input: RemoveMcpServerInput) -> Result<(), String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.remove(&input.server_id)
    })
    .await
}
