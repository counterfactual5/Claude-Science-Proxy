// Claude Science Proxy 桌面面板前端。只调用后端 Tauri command，绝不碰任何密钥落盘逻辑。
// 后端只把 key 的【掩码】回显给这里；完整 key 永不进前端。
//
// ── Tauri 参数键约定（务必遵守）──────────────────────────────────────────────
// 本项目所有命令都是裸 `#[tauri::command]`（无 rename_all）。tauri-macros 默认
// `ArgumentCase::Camel`，会把 Rust 蛇形【顶层参数名】转成 lowerCamelCase 交给 JS：
//   template_id→templateId、base_url→baseUrl、api_format→apiFormat、skip_verify→skipVerify。
// 所以 invoke 顶层 args 用【小驼峰】。而 serde 结构体入参（`req`=FetchModelsReq、
// `cfg`=UiSettings）内部字段按结构体字段名（蛇形）：proxy_port/sandbox_port、
// template_id/base_url/key/profile_id。
//
// 预览兜底：在普通浏览器（没有 Tauri 后端）里打开时用 mockInvoke 返回假数据，
// 让界面能完整渲染。真实 app 里 window.__TAURI__ 存在，走真后端，此兜底不生效。
const PREVIEW = !window.__TAURI__;
const invoke = PREVIEW
  ? (cmd, args) => mockInvoke(cmd, args)
  : window.__TAURI__.core.invoke;

// ── 版本：国内 cn / 国际 intl（语言 + 提供商列表）────────────────────────────
function detectEdition() {
  const q = new URLSearchParams(location.search).get("edition");
  if (q === "cn" || q === "intl") return q;
  const lang = (navigator.language || "en").toLowerCase();
  return lang.startsWith("zh") ? "cn" : "intl";
}
const EDITION = detectEdition();

const I18N = {
  cn: {
    myConfigs: "我的配置",
    newBtn: "＋ 新建",
    presetPicker: "预设…",
    presetTitle: "快速填入名称与地址",
    wizNamePlaceholder: "DeepSeek",
    wizBasePlaceholder: "https://open.bigmodel.cn/api/anthropic",
    provider: "Provider",
    baseUrl: "Base_URL",
    apiKey: "API Key",
    create: "创建",
    cancel: "取消",
    save: "保存",
    edit: "编辑",
    delete: "删除",
    models: "启用模型",
    modelSingle: "模型",
    ports: "端口管理",
    proxyPort: "代理",
    sandboxPort: "沙箱",
    startScience: "启动 Claude Science",
    stop: "停止",
    stopTitle: "停止本地代理与 Science 沙箱",
    editCspJson: "Edit CSP.json",
    activeBadge: "当前生效",
    emptyTitle: "还没有模型配置",
    emptyHint: "点右上「＋ 新建」添加一条连接",
    noUrl: "（未填地址）",
    keyLabel: "Key",
    keySep: "：",
    keyMissing: "未填 key",
    fillProvider: "请填写 Provider。",
    fillBaseUrl: "请填写 Base_URL。",
    fillApiKey: "请填写 API Key。",
    skipActivate: "校验没过，仍要启用这条",
    menuMore: "更多",
    ready: "就绪。",
    modelHintNative: "由 Science 选择器 + 内置映射自动选择（opus 深度 / haiku 快速）。",
    modelHintFollow: "留空＝跟随 Science 选择器（保留 opus/haiku 各档）；选一个＝固定用于所有请求。",
    modelHintFixed: "勾选要在 Science 中启用的模型；列表第一个用于后台任务兜底。",
    modelBuiltin: "内置",
    toolsYes: " ·工具✓",
    toolsNo: " ·无工具",
    metaManyModels: "{n} 个模型已启用",
    metaOneModel: "1 个模型已启用",
    metaBuiltinMap: "内置映射",
    metaFollowScience: "跟随 Science",
    metaNoModel: "未选模型",
    loadConfigFail: "读取配置失败：{err}",
    portSavedReset: "端口已保存。改端口会重置正在运行的代理/沙箱，请重新启动 Claude Science。",
    portUnchanged: "端口未变化。",
    activateSkip: "正在启用「{name}」：已跳过上游校验，正在启动正式代理并探活…",
    activateSkipWait: "仍在等待正式代理探活完成。完成后会自动应用，失败会保留原配置。",
    activateVerify: "正在启用「{name}」：先用临时代理校验上游，网络慢时可能需要约 20 秒…",
    activateVerifyWait1: "仍在等待上游校验响应。可以继续查看日志/报告，请不要重复切换配置。",
    activateVerifyWait2: "上游校验仍未返回，接近本次等待上限。完成后会自动切换，或给出重试/跳过验证提示。",
    saveConnActive: "正在保存当前生效配置：先校验新连接，再重启正式代理并探活…",
    saveConnActiveWait1: "仍在等待新连接上游校验。失败会保留原连接和原代理。",
    saveConnActiveWait2: "上游校验接近等待上限。完成后才会写盘并应用，失败会据实回滚。",
    saveConnIdle: "保存连接中：正在做候选上游校验；无法确认时会保存但标记为未校验…",
    saveConnIdleWait: "仍在等待候选连接校验。不会影响当前正在运行的代理。",
    oneClickStart: "正在启动：检查代理 → 准备虚拟登录 → 启动/复用沙箱 → 探活…",
    oneClickWait1: "仍在准备代理或沙箱。若代理配置已变更，可能需要重启本地代理。",
    oneClickWait2: "仍在等待沙箱就绪。完成后会自动打开 Science；失败会显示日志摘要。",
    portSaveChanged: "正在保存端口设置：端口变化会先重置当前代理/沙箱链路…",
    portSaveChangedWait: "仍在应用端口设置。若旧沙箱无法停止，端口会保持原值并显示错误。",
    portSaving: "正在保存端口设置…",
    cancelled: "已取消。",
    confirmRetry: "{prompt} —— 再点一次同一按钮确认（4 秒内）。",
    creating: "创建中…",
    fetchingModels: "正在拉取模型…",
    createdMany: "已创建「{name}」，发现 {total} 个模型，已启用前 {enabled} 个。其余可在编辑里勾选，或直接改 CSP.json。",
    createdSome: "已创建「{name}」，已启用 {enabled} 个模型。更多可在编辑里勾选，或直接改 CSP.json。",
    createdNoModels: "已创建「{name}」，未能拉取模型列表。请在编辑里重试或手动配置。",
    createFail: "创建失败：{err}",
    anthropicBaseErr: "这个地址看起来是 Anthropic 兼容端点，请填写 OpenAI 兼容 base root（如 https://api.example.com/v1）。",
    editConnActive: "编辑（当前生效）",
    editConnHintActive: "编辑当前生效配置：保存会先校验→切换，失败自动回退到原配置（不谎报生效）。",
    editConnHint: "编辑连接后点「保存」。",
    connBaseHintDefault: "官方默认地址，可改到 token 套餐 / 区域端点。",
    connBaseHintOpenAIChat: "OpenAI 兼容 base root，代理自动补 /chat/completions。",
    connBaseHintOpenAIResp: "OpenAI 兼容 base root，代理自动补 /responses。",
    connBaseHintCustom: "自定义端点根地址。",
    connBaseHintNative: "模板地址（只读），模型由内置映射自动选择。",
    connBaseHintReadonly: "模板地址（只读）。",
    connKeySaved: "已保存（留空不改）",
    modelsRefreshed: "已刷新 {n} 个可用模型，勾选要启用的项。",
    modelsFetchFallback: "未能拉取模型列表，已显示已保存的模型。",
    modelsFetchFail: "拉取模型失败：{err}",
    profileMissing: "配置不存在。",
    providerEmpty: "Provider 不能为空。",
    needModel: "该来源需要至少选一个模型才能保存。",
    savedApplied: "已保存并应用新连接。",
    savedValidated: "已保存连接（已通过上游校验）。",
    savedUnvalidated: "已保存连接（未能连通上游校验，激活时会再验）。",
    saveConnFail: "连接未保存：{err}",
    confirmDelete: "将删除配置「{name}」",
    deleting: "删除中…",
    deletedWasActive: "已删除。删掉的是当前生效配置，请点击另一条配置切换。",
    deleted: "已删除。",
    deleteFail: "删除失败：{err}",
    switched: "已切换为当前配置。",
    switchRejected: "校验未通过，未切换。",
    switchFail: "切换失败：{err}",
    noActiveProfile: "还没有「当前生效」的配置。请先「＋ 新建」或点击一条配置切换，再点「启动 Claude Science」。",
    oneClickReady: "已就绪，正在打开面板…",
    oneClickFail: "启动失败：{err}",
    stopping: "停止中…",
    stopped: "已停止代理与沙箱。",
    stopFail: "停止失败：{err}",
    openCspFail: "打开 CSP.json 失败：{err}",
    previewMode: "预览模式：仅看界面，按钮不连后端（真实 app 里会连进程管家）。",
  },
  intl: {
    myConfigs: "Profiles",
    newBtn: "+ New",
    presetPicker: "Preset…",
    presetTitle: "Fill name and URL",
    wizNamePlaceholder: "ZAI",
    wizBasePlaceholder: "https://api.z.ai/api/anthropic",
    provider: "Provider",
    baseUrl: "Base URL",
    apiKey: "API Key",
    create: "Create",
    cancel: "Cancel",
    save: "Save",
    edit: "Edit",
    delete: "Delete",
    models: "Enabled models",
    modelSingle: "Model",
    ports: "Ports",
    proxyPort: "Proxy",
    sandboxPort: "Sandbox",
    startScience: "Start Claude Science",
    stop: "Stop",
    stopTitle: "Stop local proxy and Science sandbox",
    editCspJson: "Edit CSP.json",
    activeBadge: "Active",
    emptyTitle: "No profiles yet",
    emptyHint: "Tap + New to add a connection",
    noUrl: "(no URL)",
    keyLabel: "Key",
    keySep: ": ",
    keyMissing: "no key",
    fillProvider: "Enter a provider name.",
    fillBaseUrl: "Enter a base URL.",
    fillApiKey: "Enter an API key.",
    skipActivate: "Activate anyway (skip verify)",
    menuMore: "More",
    ready: "Ready.",
    modelHintNative: "Auto-mapped via Science picker + built-in routing (opus for depth, haiku for speed).",
    modelHintFollow: "Leave empty to follow Science picker (opus/haiku tiers); pick one to pin all requests.",
    modelHintFixed: "Check models to enable in Science; the first is the fallback for background tasks.",
    modelBuiltin: "builtin",
    toolsYes: " ·tools",
    toolsNo: " ·no tools",
    metaManyModels: "{n} models enabled",
    metaOneModel: "1 model enabled",
    metaBuiltinMap: "built-in mapping",
    metaFollowScience: "follows Science",
    metaNoModel: "no model selected",
    loadConfigFail: "Failed to load config: {err}",
    portSavedReset: "Ports saved. Changing ports resets the running proxy/sandbox — restart Claude Science.",
    portUnchanged: "Ports unchanged.",
    activateSkip: "Activating \"{name}\": skipped upstream verify, starting proxy and health check…",
    activateSkipWait: "Still waiting for proxy health check. Will apply on success or keep previous config on failure.",
    activateVerify: "Activating \"{name}\": probing upstream via scratch proxy (may take ~20s on slow networks)…",
    activateVerifyWait1: "Still waiting for upstream probe. You can check logs; avoid switching profiles again.",
    activateVerifyWait2: "Probe still pending, near timeout. Will switch or offer retry/skip when done.",
    saveConnActive: "Saving active profile: verify new connection, restart proxy, then health check…",
    saveConnActiveWait1: "Still verifying new connection. On failure, previous connection and proxy are kept.",
    saveConnActiveWait2: "Verify near timeout. Will commit only after probe completes; rolls back on failure.",
    saveConnIdle: "Saving connection: probing candidate upstream; may save as unverified if inconclusive…",
    saveConnIdleWait: "Still probing candidate. Current running proxy is unaffected.",
    oneClickStart: "Starting: check proxy → virtual login → start/reuse sandbox → health check…",
    oneClickWait1: "Still preparing proxy or sandbox. Config changes may require a proxy restart.",
    oneClickWait2: "Still waiting for sandbox. Science opens on success; errors show in logs.",
    portSaveChanged: "Saving ports: port change resets proxy/sandbox chain…",
    portSaveChangedWait: "Still applying ports. If old sandbox cannot stop, ports stay unchanged and an error is shown.",
    portSaving: "Saving ports…",
    cancelled: "Cancelled.",
    confirmRetry: "{prompt} — click the same button again to confirm (within 4s).",
    creating: "Creating…",
    fetchingModels: "Fetching models…",
    createdMany: "Created \"{name}\": found {total} models, enabled top {enabled}. Enable more in edit or edit CSP.json.",
    createdSome: "Created \"{name}\": enabled {enabled} models. Enable more in edit or edit CSP.json.",
    createdNoModels: "Created \"{name}\" but could not fetch models. Retry in edit or configure manually.",
    createFail: "Create failed: {err}",
    anthropicBaseErr: "This URL looks Anthropic-compatible. Use an OpenAI-compatible base root (e.g. https://api.example.com/v1).",
    editConnActive: "Edit (active)",
    editConnHintActive: "Editing active profile: save verifies then switches; failure rolls back (no false success).",
    editConnHint: "Edit connection, then Save.",
    connBaseHintDefault: "Official default; change for token plans / regional endpoints.",
    connBaseHintOpenAIChat: "OpenAI-compatible base root; proxy appends /chat/completions.",
    connBaseHintOpenAIResp: "OpenAI-compatible base root; proxy appends /responses.",
    connBaseHintCustom: "Custom endpoint root URL.",
    connBaseHintNative: "Template URL (read-only); models use built-in mapping.",
    connBaseHintReadonly: "Template URL (read-only).",
    connKeySaved: "Saved (leave blank to keep)",
    modelsRefreshed: "Refreshed {n} models — check the ones to enable.",
    modelsFetchFallback: "Could not fetch models; showing saved selection.",
    modelsFetchFail: "Fetch models failed: {err}",
    profileMissing: "Profile not found.",
    providerEmpty: "Provider name is required.",
    needModel: "Select at least one model for this provider.",
    savedApplied: "Saved and applied.",
    savedValidated: "Saved (upstream verified).",
    savedUnvalidated: "Saved (upstream not verified; will re-check on activate).",
    saveConnFail: "Not saved: {err}",
    confirmDelete: "Delete profile \"{name}\"",
    deleting: "Deleting…",
    deletedWasActive: "Deleted active profile. Switch to another profile.",
    deleted: "Deleted.",
    deleteFail: "Delete failed: {err}",
    switched: "Switched to this profile.",
    switchRejected: "Verify failed; not switched.",
    switchFail: "Switch failed: {err}",
    noActiveProfile: "No active profile. Create or select one, then Start Claude Science.",
    oneClickReady: "Ready, opening panel…",
    oneClickFail: "Start failed: {err}",
    stopping: "Stopping…",
    stopped: "Proxy and sandbox stopped.",
    stopFail: "Stop failed: {err}",
    openCspFail: "Failed to open CSP.json: {err}",
    previewMode: "Preview mode: UI only, no backend (real app uses Tauri).",
  },
};
function S() { return I18N[EDITION]; }
function T(key, vars) {
  const raw = S()[key];
  if (!raw) return key;
  return Object.entries(vars || {}).reduce(
    (s, [k, v]) => s.replaceAll(`{${k}}`, String(v)),
    raw
  );
}
function modelHints() {
  const t = S();
  return { native: t.modelHintNative, fixed: t.modelHintFixed };
}

/** 国内版：每家一个默认端点，仅保留国内常用线路。 */
const WIZ_PRESETS_CN = [
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", label: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "glm", templateId: "glm", name: "GLM", label: "智谱 GLM", baseUrl: "https://open.bigmodel.cn/api/anthropic" },
  { id: "glm-coding", templateId: "custom-openai", name: "GLM Coding Plan", label: "智谱 Coding Plan", baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4" },
  { id: "kimi", templateId: "kimi", name: "Moonshot", label: "Moonshot", baseUrl: "https://api.moonshot.cn/anthropic" },
  { id: "minimax", templateId: "minimax", name: "MiniMax", label: "MiniMax", baseUrl: "https://api.minimaxi.com/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", label: "小米 MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token", templateId: "xiaomi", name: "MiMo", label: "小米 MiMo · Token 套餐", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
];

/** 国际版：海外常用端点。 */
const WIZ_PRESETS_INTL = [
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", label: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "glm", templateId: "glm", name: "ZAI", label: "ZAI", baseUrl: "https://api.z.ai/api/anthropic" },
  { id: "kimi", templateId: "kimi", name: "Moonshot", label: "Moonshot", baseUrl: "https://api.moonshot.ai/anthropic" },
  { id: "minimax", templateId: "minimax", name: "MiniMax", label: "MiniMax", baseUrl: "https://api.minimax.io/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", label: "Xiaomi MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token", templateId: "xiaomi", name: "MiMo", label: "MiMo · Token Plan", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
  { id: "openrouter", templateId: "openrouter", name: "OpenRouter", label: "OpenRouter", baseUrl: "https://openrouter.ai/api" },
];

function wizPresets() {
  return EDITION === "cn" ? WIZ_PRESETS_CN : WIZ_PRESETS_INTL;
}

/** 向导里最近一次自动填入的 base URL；用户手改后清空，避免覆盖自定义地址。 */
let wizLastAutoBase = "";

function applyEditionUI() {
  const t = S();
  document.documentElement.lang = EDITION === "cn" ? "zh-CN" : "en";
  if (els.i18nMyConfigs) els.i18nMyConfigs.textContent = t.myConfigs;
  if (els.newBtn) els.newBtn.textContent = t.newBtn;
  if (els.i18nLabelProvider) els.i18nLabelProvider.textContent = t.provider;
  if (els.i18nLabelBase) els.i18nLabelBase.textContent = t.baseUrl;
  if (els.i18nLabelKey) els.i18nLabelKey.textContent = t.apiKey;
  if (els.wizSaveBtn) els.wizSaveBtn.textContent = t.create;
  if (els.wizCancelBtn) els.wizCancelBtn.textContent = t.cancel;
  if (els.connTitle) els.connTitle.textContent = t.edit;
  if (els.i18nConnName) els.i18nConnName.textContent = t.provider;
  if (els.i18nConnBase) els.i18nConnBase.textContent = t.baseUrl;
  if (els.i18nConnKey) els.i18nConnKey.textContent = t.apiKey;
  if (els.connModelLabel) els.connModelLabel.textContent = t.models;
  if (els.connSaveBtn) els.connSaveBtn.textContent = t.save;
  if (els.connCancelBtn) els.connCancelBtn.textContent = t.cancel;
  if (els.i18nPorts) els.i18nPorts.textContent = t.ports;
  if (els.i18nProxyPort) els.i18nProxyPort.textContent = t.proxyPort;
  if (els.i18nSandboxPort) els.i18nSandboxPort.textContent = t.sandboxPort;
  if (els.oneClickBtn) els.oneClickBtn.textContent = t.startScience;
  if (els.stopBtn) { els.stopBtn.textContent = t.stop; els.stopBtn.title = t.stopTitle; }
  if (els.editCspJsonBtn) els.editCspJsonBtn.textContent = t.editCspJson;
  if (els.skipActivateBtn) els.skipActivateBtn.textContent = t.skipActivate;
  if (els.listhdMoreBtn) els.listhdMoreBtn.title = t.menuMore;
  populateWizPresetSelect();
  refreshWizPlaceholders();
}

function refreshWizPlaceholders() {
  const t = S();
  if (els.wizName) els.wizName.placeholder = t.wizNamePlaceholder || "DeepSeek";
  if (els.wizBase) els.wizBase.placeholder = t.wizBasePlaceholder || "https://api.example.com/v1";
}

// ── 预览兜底 mock（仅浏览器预览用） ──
const MOCK_TEMPLATES = [
  { id: "deepseek", name: "DeepSeek", api_format: "anthropic", adapter: "deepseek", base_url: "https://api.deepseek.com/anthropic", base_url_editable: false, requires_model_override: false, builtin_models: ["claude-opus-4-8", "claude-haiku-4-5"], icon: "deepseek", icon_color: "#1E88E5" },
  { id: "glm", name: "GLM", api_format: "anthropic", adapter: "relay", base_url: "https://open.bigmodel.cn/api/anthropic", base_url_editable: true, requires_model_override: true, builtin_models: ["glm-5.2", "glm-4.7"], icon: "glm", icon_color: "#2E6BE6" },
  { id: "xiaomi", name: "MiMo", api_format: "anthropic", adapter: "relay", base_url: "https://api.xiaomimimo.com/anthropic", base_url_editable: true, requires_model_override: true, builtin_models: ["mimo-v2.5-pro"], icon: "xiaomi", icon_color: "#FF6900" },
  { id: "kimi", name: "Moonshot", api_format: "anthropic", adapter: "relay", base_url: "https://api.moonshot.cn/anthropic", base_url_editable: true, requires_model_override: true, builtin_models: ["kimi-k2.7-code"], icon: "kimi", icon_color: "#16182F" },
  { id: "openrouter", name: "OpenRouter", api_format: "anthropic", adapter: "relay", base_url: "https://openrouter.ai/api", base_url_editable: true, requires_model_override: true, builtin_models: ["anthropic/claude-sonnet-5"], icon: "openrouter", icon_color: "#6467F2" },
  { id: "custom", name: "Custom", api_format: "anthropic", adapter: "relay", base_url: "", base_url_editable: true, requires_model_override: true, builtin_models: [], icon: "custom", icon_color: "#6B7280" },
];
const mockStore = { 
  schema_version: 2,
  active_id: "",
  proxy_port: 18991,
  sandbox_port: 8990,
  profiles: [
    { id: "p-demo1", name: "GLM", template_id: "glm", api_format: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", model: "glm-5.2", active_models: ["glm-5.2"], key: "••••••1234", icon: "glm", icon_color: "#2E6BE6", notes: "" },
  ],
};
function mockMask(k) { return k ? "••••" + String(k).slice(-4) : ""; }
function mockInvoke(cmd, args) {
  args = args || {};
  switch (cmd) {
    case "get_config":
      return Promise.resolve({
        schema_version: mockStore.schema_version, active_id: mockStore.active_id,
        proxy_port: mockStore.proxy_port, sandbox_port: mockStore.sandbox_port,
        templates: MOCK_TEMPLATES,
        profiles: mockStore.profiles.map((p) => ({ ...p })),
      });
    case "create_profile": {
      const t = MOCK_TEMPLATES.find((x) => x.id === args.templateId) || {};
      const id = "p-" + Math.random().toString(16).slice(2, 10);
      mockStore.profiles.push({
        id, name: args.name || t.name || "Profile", template_id: args.templateId,
        api_format: t.api_format || "anthropic",
        base_url: args.baseUrl || t.base_url || "", model: args.model || "",
        key: mockMask(args.key || ""), icon: t.icon, icon_color: t.icon_color, notes: "",
      });
      return Promise.resolve(id);
    }
    case "update_profile_metadata": {
      const p = mockStore.profiles.find((x) => x.id === args.id);
      if (!p) return Promise.reject("找不到 profile：" + args.id);
      p.name = args.name; p.notes = args.notes || "";
      return Promise.resolve(null);
    }
    case "update_profile_connection": {
      const p = mockStore.profiles.find((x) => x.id === args.id);
      if (!p) return Promise.reject("找不到 profile：" + args.id);
      if (args.baseUrl != null) p.base_url = args.baseUrl;
      if (args.model != null) p.model = args.model;
      if (args.key) p.key = mockMask(args.key);
      return Promise.resolve({ validated: true });
    }
    case "delete_profile":
      mockStore.profiles = mockStore.profiles.filter((x) => x.id !== args.id);
      if (mockStore.active_id === args.id) mockStore.active_id = "";
      return Promise.resolve(null);
    case "set_active_profile": {
      const p = mockStore.profiles.find((x) => x.id === args.id);
      if (!p) return Promise.reject("找不到 profile：" + args.id);
      mockStore.active_id = args.id;
      return Promise.resolve({ committed: true, active_id: args.id, hint: "（预览：已设为当前）" });
    }
    case "fetch_models":
      return Promise.resolve({ models: [{ id: "glm-4.6", supports_tools: true }, { id: "glm-5", supports_tools: null }], source: "live", error_kind: null, upstream_status: 200 });
    case "set_settings":
      if (args.cfg) { mockStore.proxy_port = args.cfg.proxy_port; mockStore.sandbox_port = args.cfg.sandbox_port; }
      return Promise.resolve(null);
    case "one_click_login":
      return Promise.resolve({ url: "http://127.0.0.1:8990", msg: "（预览模式：假装已就绪）", action: "started" });
    case "open_csp_json":
      return Promise.resolve("~/.csp/CSP.json");
    default:
      return Promise.resolve(null);
  }
}

const $ = (id) => document.getElementById(id);
const els = {};
let busy = false;
let busyOp = null;
let busyMsgTimers = [];
// 当前配置快照（get_config 结果）。全 key 绝不在此，只有掩码。
let configState = { profiles: [], templates: [], active_id: "", proxy_port: 18991, sandbox_port: 8990 };
let pendingSkipActivateId = null;   // set_active 校验含糊时，允许「跳过验证」再切
let pendingConfirm = null;          // 危险操作（清 key / 删除）的「再点一次确认」态

// ── 模型能力：native 内置映射 / relay 多选固定模型 ──
const CAP = { NATIVE: "native", FIXED: "fixed" };
function templateCaps(t) { return (t && t.capabilities) || {}; }
function modelCapability(t) {
  if (!t) return CAP.FIXED;
  const caps = templateCaps(t);
  if (caps.model_discovery === "builtin_static" && caps.model_required === false) return CAP.NATIVE;
  if (t.adapter === "deepseek" || t.adapter === "qwen") return CAP.NATIVE;
  return CAP.FIXED;
}
function modelRequired(t) {
  if (!t) return true;
  const caps = templateCaps(t);
  if (Object.prototype.hasOwnProperty.call(caps, "model_required")) return !!caps.model_required;
  return modelCapability(t) === CAP.FIXED;
}
function profileCapabilitySource(p, t) {
  if (!p || !p.capabilities) return t;
  return {
    ...(t || {}),
    ...p,
    adapter: (t && t.adapter) || p.adapter,
    builtin_models: (t && t.builtin_models) || [],
    base_url_editable: t ? t.base_url_editable : true,
    capabilities: p.capabilities,
  };
}

function profileModels(p) {
  const active = (p && p.active_models) || [];
  if (active.length) return active.slice();
  if (p && p.model) return [p.model];
  return [];
}

function collectCheckedModels(container) {
  if (!container || container.hidden) return [];
  return [...container.querySelectorAll('input[type="checkbox"]:checked')]
    .map((el) => el.getAttribute("data-model"))
    .filter(Boolean);
}

function renderModelPick(container, builtin, selected, onChange) {
  if (!container) return;
  const pool = [];
  for (const id of builtin || []) if (!pool.includes(id)) pool.push(id);
  for (const id of selected || []) if (id && !pool.includes(id)) pool.unshift(id);
  if (!pool.length) {
    container.hidden = true;
    container.innerHTML = "";
    return;
  }
  container.hidden = false;
  const selSet = new Set(selected && selected.length ? selected : pool);
  container.innerHTML = pool.map((id) => {
    const checked = selSet.has(id) ? " checked" : "";
    return '<label class="model-pick-item"><input type="checkbox" data-model="' +
      escapeHtml(id) + '"' + checked + '><span class="model-pick-label">' + escapeHtml(id) + "</span></label>";
  }).join("");
  container.querySelectorAll('input[type="checkbox"]').forEach((cb) => {
    cb.addEventListener("change", () => { if (onChange) onChange(); });
  });
}

// native：只读说明；relay：checkbox 多选。
function applyModelCapability(t, ui, profileOrModel) {
  const p = profileOrModel && typeof profileOrModel === "object" ? profileOrModel : null;
  const currentModel = p ? (p.default_model || p.model || "") : (profileOrModel || "");
  const selected = p ? profileModels(p) : (currentModel ? [currentModel] : []);
  const cap = modelCapability(t);
  const onPickChange = ui.onPickChange || (() => {});
  const hints = modelHints();
  if (cap === CAP.NATIVE) {
    ui.info.textContent = hints.native;
    ui.info.hidden = false;
    if (ui.pick) { ui.pick.hidden = true; ui.pick.innerHTML = ""; }
    ui.hint.textContent = "";
    return cap;
  }
  ui.info.hidden = true;
  const builtin = ((t && t.builtin_models) || []).slice();
  if (currentModel && !builtin.includes(currentModel)) builtin.unshift(currentModel);
  ui.hint.textContent = hints.fixed;
  if (ui.modelLabel) ui.modelLabel.textContent = S().models;
  if (ui.pick) {
    renderModelPick(ui.pick, builtin, selected, onPickChange);
  }
  return cap;
}

function setMsg(text, kind) {
  // 反馈区仅展示错误；成功/进度/中性提示一律不占位。
  const t = kind === "err" && text && text !== T("ready") ? text : "";
  els.msg.textContent = t;
  els.msg.className = "msg" + (t ? " err" : "");
  els.msg.parentElement.hidden = !t;
  if (t && els.panel && els.panel.classList.contains("view-form")) {
    els.msg.scrollIntoView({ block: "nearest" });
  }
}

function clearBusyMsgTimers() {
  busyMsgTimers.forEach((t) => clearTimeout(t));
  busyMsgTimers = [];
}

function profileName(id) {
  const p = (configState.profiles || []).find((x) => x.id === id);
  return p ? p.name : id;
}

function closeAllMenus() {
  if (!els.profileList) return;
  els.profileList.querySelectorAll(".pmenu").forEach((m) => {
    m.hidden = true;
    m.classList.remove("pmenu-up");
  });
  els.profileList.querySelectorAll(".pmenu-btn").forEach((b) => {
    b.setAttribute("aria-expanded", "false");
  });
}

function closeListhdMenu() {
  if (!els.listhdMenu) return;
  els.listhdMenu.hidden = true;
  if (els.listhdMoreBtn) els.listhdMoreBtn.setAttribute("aria-expanded", "false");
}

function toggleListhdMenu() {
  if (!els.listhdMenu || !els.listhdMoreBtn) return;
  const wasOpen = !els.listhdMenu.hidden;
  closeListhdMenu();
  closeAllMenus();
  if (!wasOpen) {
    els.listhdMenu.hidden = false;
    els.listhdMoreBtn.setAttribute("aria-expanded", "true");
  }
}

function positionProfileMenu(menu, btn) {
  menu.classList.remove("pmenu-up");
  menu.hidden = false;
  const scrollEl = els.panelBody || els.profileList;
  const containerRect = scrollEl.getBoundingClientRect();
  const btnRect = btn.getBoundingClientRect();
  const menuHeight = menu.offsetHeight;
  const gap = 4;
  const spaceBelow = containerRect.bottom - btnRect.bottom - gap;
  const spaceAbove = btnRect.top - containerRect.top - gap;
  if (menuHeight > spaceBelow && spaceAbove >= menuHeight) {
    menu.classList.add("pmenu-up");
  } else if (menuHeight > spaceBelow && spaceAbove > spaceBelow) {
    menu.classList.add("pmenu-up");
  }
}

function syncProfileBusyState() {
  if (!els.profileList) return;
  els.profileList.querySelectorAll(".prow").forEach((row) => {
    const isTarget = !!(busyOp && busyOp.kind === "activate" && row.getAttribute("data-id") === busyOp.id);
    row.classList.toggle("pworking", isTarget);
    row.querySelectorAll("button[data-act]").forEach((btn) => {
      btn.disabled = busy;
    });
    if (busy) closeAllMenus();
  });
}

function scheduleBusyMsg(ms, op, text) {
  const timer = setTimeout(() => {
    if (busy && busyOp && busyOp.kind === op.kind && busyOp.id === op.id) setMsg(text);
  }, ms);
  busyMsgTimers.push(timer);
}



function startActivateFeedback(id, skipVerify) {
  const name = profileName(id);
  clearBusyMsgTimers();
  if (skipVerify) {
    setMsg(T("activateSkip", { name }));
    scheduleBusyMsg(3500, { kind: "activate", id }, T("activateSkipWait"));
    return;
  }
  setMsg(T("activateVerify", { name }));
  scheduleBusyMsg(4500, { kind: "activate", id }, T("activateVerifyWait1"));
  scheduleBusyMsg(18000, { kind: "activate", id }, T("activateVerifyWait2"));
}

function startSaveConnectionFeedback(id, active) {
  clearBusyMsgTimers();
  if (active) {
    setMsg(T("saveConnActive"));
    scheduleBusyMsg(4500, { kind: "saveConnection", id }, T("saveConnActiveWait1"));
    scheduleBusyMsg(18000, { kind: "saveConnection", id }, T("saveConnActiveWait2"));
    return;
  }
  setMsg(T("saveConnIdle"));
  scheduleBusyMsg(4500, { kind: "saveConnection", id }, T("saveConnIdleWait"));
}

function startOneClickFeedback() {
  clearBusyMsgTimers();
  setMsg(T("oneClickStart"));
  scheduleBusyMsg(3500, { kind: "oneClick" }, T("oneClickWait1"));
  scheduleBusyMsg(9000, { kind: "oneClick" }, T("oneClickWait2"));
}

function startPortSaveFeedback(changed) {
  clearBusyMsgTimers();
  if (changed) {
    setMsg(T("portSaveChanged"));
    scheduleBusyMsg(3500, { kind: "ports" }, T("portSaveChangedWait"));
    return;
  }
  setMsg(T("portSaving"));
}


function setBusy(on, op) {
  busy = on;
  busyOp = on ? (op || { kind: "global" }) : null;
  if (!on) clearBusyMsgTimers();
  [
    els.oneClickBtn, els.stopBtn, els.newBtn, els.listhdMoreBtn,
    els.wizSaveBtn, els.wizCancelBtn,
    els.connSaveBtn, els.connCancelBtn,
    els.skipActivateBtn,
    // 端口输入也纳入忙碌禁用：忙碌中改端口会与在途操作竞态（修 P1-c 前端侧）。
    els.proxyPort, els.sandboxPort,
  ].forEach((b) => b && (b.disabled = on));
  syncProfileBusyState();
  // 松开忙碌时，把模型必填保存门控交回门（避免 setBusy(false) 覆盖门控）。
  if (!on) { refreshWizGate(); refreshConnGate(); }
}

async function call(cmd, args) {
  return await invoke(cmd, args);
}

function escapeHtml(s) {
  return String(s == null ? "" : s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])
  );
}

function tplById(id) {
  return (configState.templates || []).find((t) => t.id === id) || null;
}

// ── 视图切换：列表 / 新建向导 / 连接编辑。一次只显示一个表单（列表隐去减少高度）。──
function showView(v) {
  els.listSec.hidden = v !== "list";
  els.advSec.hidden = v !== "list";
  els.wizSec.hidden = v !== "wizard";
  els.connSec.hidden = v !== "conn";
  els.panel.classList.toggle("view-form", v !== "list");
  if (v === "list") hideSkip();
}
function cancelForm() { showView("list"); setMsg(T("ready")); }

function showSkip() { els.skipActivateBtn.hidden = false; }
function hideSkip() { els.skipActivateBtn.hidden = true; pendingSkipActivateId = null; }

// 危险操作「再点一次确认」（避免依赖 window.confirm，Tauri webview 里不可靠）。
function confirmAction(token, promptText, fn) {
  if (pendingConfirm && pendingConfirm.token === token) {
    clearTimeout(pendingConfirm.timer);
    pendingConfirm = null;
    fn();
    return;
  }
  if (pendingConfirm) clearTimeout(pendingConfirm.timer);
  pendingConfirm = {
    token,
    timer: setTimeout(() => { pendingConfirm = null; setMsg(T("cancelled")); }, 4000),
  };
  setMsg(T("confirmRetry", { prompt: promptText }), "err");
}

// ── 加载配置 + 渲染列表 ──
async function loadConfig() {
  try {
    const cfg = await call("get_config");
    configState.profiles = cfg.profiles || [];
    configState.templates = cfg.templates || [];
    configState.active_id = cfg.active_id || "";
    configState.proxy_port = cfg.proxy_port ?? 18991;
    configState.sandbox_port = cfg.sandbox_port ?? 8990;
    els.proxyPort.value = configState.proxy_port;
    els.sandboxPort.value = configState.sandbox_port;
    renderList();
    showView("list");
    // 一次性迁移提示（#9 甲）：后端 get_config 读后已清盘，只会出现一次。
    if (cfg.pending_notice) setMsg(cfg.pending_notice, "ok");
  } catch (e) {
    setMsg(T("loadConfigFail", { err: e }), "err");
  }
}

// 列表卡片第二行：弱化模型 ID，突出能力与 key 状态。
function profileMetaLine(p) {
  const models = profileModels(p);
  const cap = modelCapability(p.capabilities ? p : tplById(p.template_id));
  if (models.length > 1) return escapeHtml(T("metaManyModels", { n: models.length }));
  if (models.length === 1) {
    if (cap === CAP.NATIVE) return T("metaBuiltinMap");
    return T("metaOneModel");
  }
  if (cap === CAP.NATIVE) return T("metaBuiltinMap");
  return T("metaNoModel");
}

function renderList() {
  const list = els.profileList;
  const ps = configState.profiles || [];
  if (!ps.length) {
    const t = S();
    list.innerHTML = '<div class="empty"><span class="empty-icon" aria-hidden="true">☁</span>' +
      escapeHtml(t.emptyTitle) + '<span class="empty-hint">' + escapeHtml(t.emptyHint) + "</span></div>";
    return;
  }
  list.innerHTML = ps.map((p) => {
    const active = p.id === configState.active_id;
    const hasKey = typeof p.has_key === "boolean" ? p.has_key : !!p.key;
    const keyMask = hasKey ? escapeHtml(p.key_masked || p.key || S().keyLabel) : S().keyMissing;
    const metaTxt = profileMetaLine(p);
    const dotStyle = p.icon_color ? ' style="background:' + escapeHtml(p.icon_color) + '"' : "";
    return (
      '<div class="prow' + (active ? " pactive" : "") + '" data-id="' + escapeHtml(p.id) + '">' +
        '<div class="prow-top">' +
          '<span class="pico"' + dotStyle + "></span>" +
          '<span class="pname">' + escapeHtml(p.name) + "</span>" +
          (active ? '<span class="badge on">' + escapeHtml(S().activeBadge) + "</span>" : "") +
        "</div>" +
        '<div class="pmeta">' + escapeHtml(p.base_url || S().noUrl) + "</div>" +
        '<div class="pmeta">' + metaTxt + " · " + escapeHtml(S().keyLabel) + escapeHtml(S().keySep) + keyMask + "</div>" +
        '<div class="prow-acts">' +
          '<div class="pmenu-wrap">' +
            '<button type="button" class="abtn pmenu-btn" data-act="menu" aria-haspopup="true" aria-expanded="false" title="' + escapeHtml(S().menuMore) + '">⋯</button>' +
            '<div class="pmenu" hidden role="menu">' +
              '<button type="button" class="pmenu-item" data-act="editconn" role="menuitem">' + escapeHtml(S().edit) + "</button>" +
              '<button type="button" class="pmenu-item danger" data-act="delete" role="menuitem">' + escapeHtml(S().delete) + "</button>" +
            "</div>" +
          "</div>" +
        "</div>" +
      "</div>"
    );
  }).join("");
  syncProfileBusyState();
}

// ── 端口设置（替旧 set_config；纯端口，不含 provider/连接）──
async function persistPorts() {
  if (busy) return; // 忙碌中不改端口（防与在途操作竞态；输入亦已禁用，此为双保险）。修 P1-c
  const p = parseInt(els.proxyPort.value, 10) || 18991;
  const s = parseInt(els.sandboxPort.value, 10) || 8990;
  const changed = p !== configState.proxy_port || s !== configState.sandbox_port;
  // 本次端口提交全程置忙：仅靠开头的 `if (busy) return` 只挡「已在忙时进入」，挡不住本函数在途
  // 时其它操作（切模式/一键/连接编辑）启动。置忙 + 禁用控件才能保证操作顺序符合用户预期。修 GPT 三轮 P2
  setBusy(true, { kind: "ports" });
  startPortSaveFeedback(changed);
  try {
    await call("set_settings", { cfg: { proxy_port: p, sandbox_port: s } });
    configState.proxy_port = p;
    configState.sandbox_port = s;
    // 后端在端口变化时会拆掉旧代理/沙箱（否则会复用指向旧端口的死链路），如实告知需重开。修 P1-c
    if (changed) {
      setMsg(T("portSavedReset"), "ok");
    } else {
      setMsg(T("portUnchanged"), "ok");
    }
  } catch (e) {
    // 出错＝端口未落盘（校验不过 / 停旧沙箱失败）：把输入框还原成实际生效值，避免显示未保存的数字。
    els.proxyPort.value = configState.proxy_port;
    els.sandboxPort.value = configState.sandbox_port;
    setMsg(String(e), "err");
  } finally {
    setBusy(false);
  }
}

// fetch_models 返回体 → 刷新 checkbox 池（编辑页自动拉取用）。
async function discoverModelIds({ templateId, baseUrl, key, profileId, builtin }) {
  const t = tplById(templateId);
  const cap = modelCapability(t);
  const fallback = (builtin || []).slice();
  if (cap === CAP.NATIVE && fallback.length) return fallback;
  try {
    const r = await call("fetch_models", {
      req: {
        template_id: templateId,
        api_format: t ? t.api_format : "",
        base_url: baseUrl,
        key: key || "",
        profile_id: profileId || "",
      },
    });
    const ids = ((r && r.models) || []).map((m) => m.id).filter(Boolean);
    if (ids.length) return ids;
  } catch (_) { /* 回落内置 */ }
  return fallback;
}

/** 新建 profile 时默认启用前 N 个模型；其余在编辑页勾选或改 CSP.json。 */
const MAX_AUTO_ENABLE_MODELS = 8;

function modelsToEnableOnCreate(discoveredIds, builtin) {
  const ids = (discoveredIds || []).filter(Boolean);
  const builtins = (builtin || []).filter(Boolean);
  const pool = ids.length ? ids : builtins;
  return pool.slice(0, MAX_AUTO_ENABLE_MODELS);
}

// fetch_models 返回体 → 刷新 checkbox 池（编辑页自动拉取用）。
function applyFetchToPick(r, pickUi, selected) {
  const models = (r && r.models) || [];
  const ids = models.map((m) => m.id).filter(Boolean);
  const pool = ids.length ? ids : (pickUi.builtin || []);
  const sel = (selected && selected.length) ? selected : pool;
  if (pickUi.pick) renderModelPick(pickUi.pick, pool, sel, pickUi.onPickChange);
  return ids.length;
}

// ── C2：新建（Provider 预设 + base_url + key）──

function wizPresetById(id) {
  return wizPresets().find((item) => item.id === id) || null;
}

function wizPresetByName(name) {
  const q = (name || "").trim().toLowerCase();
  if (!q) return null;
  return wizPresets().find(
    (item) => item.name.toLowerCase() === q || item.label.toLowerCase() === q
  ) || null;
}

function applyWizPresetFields(preset) {
  els.wizName.value = preset.name;
  els.wizBase.value = preset.baseUrl;
  els.wizBase.readOnly = !!preset.lockUrl;
  wizLastAutoBase = preset.baseUrl || "";
  if (!preset.baseUrl) els.wizBase.focus();
}

function applyWizNameAutofill() {
  const name = els.wizName.value.trim();
  const preset = wizPresetByName(name);
  if (!preset) {
    if (wizLastAutoBase && els.wizBase.value.trim() === wizLastAutoBase) {
      els.wizBase.value = "";
      els.wizBase.readOnly = false;
      wizLastAutoBase = "";
    }
    if (els.wizPreset.value) els.wizPreset.value = "";
    refreshWizGate();
    return;
  }
  if (els.wizPreset.value !== preset.id) els.wizPreset.value = preset.id;
  const currentBase = els.wizBase.value.trim();
  const canAutoFill = !currentBase || currentBase === wizLastAutoBase;
  if (canAutoFill && preset.baseUrl) {
    els.wizBase.value = preset.baseUrl;
    wizLastAutoBase = preset.baseUrl;
    els.wizBase.readOnly = !!preset.lockUrl;
  }
  refreshWizGate();
}

function populateWizPresetSelect() {
  const sel = els.wizPreset;
  if (!sel) return;
  const t = S();
  sel.title = t.presetTitle;
  sel.innerHTML = '<option value="">' + escapeHtml(t.presetPicker) + "</option>";
  for (const item of wizPresets()) {
    const opt = document.createElement("option");
    opt.value = item.id;
    opt.textContent = item.label;
    sel.appendChild(opt);
  }
}

function applyWizPreset() {
  const preset = wizPresetById(els.wizPreset.value);
  if (!preset) {
    refreshWizGate();
    return;
  }
  applyWizPresetFields(preset);
  refreshWizGate();
}

function normBaseUrl(url) {
  return (url || "").trim().replace(/\/+$/, "").toLowerCase();
}

function inferTemplateId(baseUrl) {
  const norm = normBaseUrl(baseUrl);
  if (!norm) return "custom";
  for (const t of configState.templates || []) {
    const tb = normBaseUrl(t.base_url);
    if (tb && tb === norm) return t.id;
  }
  if (norm.includes("deepseek.com")) return "deepseek";
  if (norm.includes("dashscope.aliyuncs.com")) return "qwen";
  if ((norm.includes("open.bigmodel.cn") || norm.includes("api.z.ai")) && norm.includes("/anthropic")) return "glm";
  if (norm.includes("xiaomimimo.com") || norm.includes("token-plan")) return "xiaomi";
  if (norm.includes("siliconflow.cn") || norm.includes("siliconflow.com")) return "siliconflow";
  if (norm.includes("moonshot.cn") || norm.includes("moonshot.ai")) return "kimi";
  if (norm.includes("minimaxi.com") || norm.includes("minimax.io")) return "minimax";
  if (norm.includes("openrouter.ai")) return "openrouter";
  if (norm.includes("/responses") && !norm.includes("/anthropic")) return "custom-openai-responses";
  if (norm.includes("/v1") || norm.includes("/paas/") || norm.includes("compatible-mode") || norm.includes("/coding/")) {
    return "custom-openai";
  }
  return "custom";
}

function openWizard() {
  hideSkip();
  wizLastAutoBase = "";
  els.wizName.value = "";
  els.wizPreset.value = "";
  els.wizBase.value = "";
  els.wizBase.readOnly = false;
  els.wizKey.value = "";
  refreshWizPlaceholders();
  refreshWizGate();
  showView("wizard");
}

function refreshWizGate() {
  const ok = els.wizName.value.trim() && els.wizBase.value.trim() && els.wizKey.value.trim();
  els.wizSaveBtn.disabled = busy || !ok;
}

function openaiCustomAnthropicBaseMessage(t, base) {
  if (t && (t.id === "custom-openai" || t.id === "custom-openai-responses") && (base || "").trim().toLowerCase().includes("/anthropic")) {
    return T("anthropicBaseErr");
  }
  return "";
}

async function wizSave() {
  const name = els.wizName.value.trim();
  const base = els.wizBase.value.trim();
  const key = els.wizKey.value.trim();
  if (!name) { setMsg(S().fillProvider, "err"); return; }
  if (!base) { setMsg(S().fillBaseUrl, "err"); return; }
  if (!key) { setMsg(S().fillApiKey, "err"); return; }
  const templateId = inferTemplateId(base);
  const t = tplById(templateId);
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) { setMsg(baseErr, "err"); return; }
  setBusy(true);
  setMsg(T("creating"));
  try {
    const id = await call("create_profile", { templateId, name, key, baseUrl: base, model: "" });
    setMsg(T("fetchingModels"));
    const builtin = (t && t.builtin_models) || [];
    const discovered = await discoverModelIds({ templateId, baseUrl: base, key, builtin });
    const modelIds = modelsToEnableOnCreate(discovered, builtin);
    if (modelIds.length) {
      await call("update_profile_connection", {
        id,
        baseUrl: base,
        model: modelIds[0],
        activeModels: modelIds,
        defaultModel: modelIds[0],
        key: "",
      });
    }
    els.wizKey.value = "";
    await loadConfig();
    if (modelIds.length) {
      const total = discovered.length || modelIds.length;
      if (total > modelIds.length) {
        setMsg(T("createdMany", { name, total, enabled: modelIds.length }), "ok");
      } else {
        setMsg(T("createdSome", { name, enabled: modelIds.length }), "ok");
      }
    } else {
      setMsg(T("createdNoModels", { name }), "ok");
    }
  } catch (e) {
    setMsg(T("createFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

function resolvedConnectionModels(cap, pickContainer) {
  if (cap === CAP.NATIVE) return { models: [], defaultModel: "" };
  const checked = collectCheckedModels(pickContainer);
  return { models: checked, defaultModel: checked[0] || "" };
}

// ── C3：连接编辑（base_url/model/key）+ 清 key ──
function currentConn() {
  const id = els.connSec.dataset.id;
  return (configState.profiles || []).find((x) => x.id === id) || null;
}

function openConn(id) {
  const p = (configState.profiles || []).find((x) => x.id === id);
  if (!p) return;
  const t = tplById(p.template_id);
  const capSrc = profileCapabilitySource(p, t);
  const editable = t ? t.base_url_editable : true;
  const active = id === configState.active_id;
  els.connSec.dataset.id = id;
  els.connTitle.textContent = active ? T("editConnActive") : S().edit;
  els.connName.value = p.name;
  els.connBase.value = p.base_url || (t ? t.base_url : "");
  els.connBase.readOnly = !editable;
  els.connBase.placeholder = capSrc && (capSrc.api_format === "openai_chat" || capSrc.api_format === "openai_responses")
    ? "https://open.bigmodel.cn/api/paas/v4"
    : "https://your-relay/claude";
  // native（deepseek/qwen）隐藏「获取模型」按钮，别再提示一个不存在的操作（修 #5）。
  els.connBaseHint.textContent = editable
    ? (t && t.base_url
        ? T("connBaseHintDefault")
        : (capSrc && capSrc.api_format === "openai_chat"
          ? T("connBaseHintOpenAIChat")
          : capSrc && capSrc.api_format === "openai_responses"
          ? T("connBaseHintOpenAIResp")
          : T("connBaseHintCustom")))
    : (modelCapability(capSrc) === CAP.NATIVE
        ? T("connBaseHintNative")
        : T("connBaseHintReadonly"));
  const cap = applyModelCapability(capSrc, {
    info: els.connModelInfo, hint: els.connModelHint,
    pick: els.connModelPick, modelLabel: els.connModelLabel, onPickChange: refreshConnGate,
  }, p);
  els.connKey.value = "";
  els.connKey.placeholder = p.key ? T("connKeySaved") : "sk-...";
  showView("conn");
  refreshConnGate();
  setMsg(active ? T("editConnHintActive") : T("editConnHint"));
  if (cap !== CAP.NATIVE) autoFetchConnModels();
}

async function autoFetchConnModels() {
  const p = currentConn();
  if (!p) return;
  const t = tplById(p.template_id);
  const capSrc = profileCapabilitySource(p, t);
  if (modelCapability(capSrc) === CAP.NATIVE) return;
  const editable = t ? t.base_url_editable : true;
  const base = editable ? els.connBase.value.trim() : (t ? t.base_url : els.connBase.value.trim());
  if (!base) return;
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) return;
  setBusy(true, { kind: "fetchModels", id: p.id });
  try {
    const key = els.connKey.value.trim();
    const r = await call("fetch_models", {
      req: {
        template_id: p.template_id,
        api_format: p.api_format || (t ? t.api_format : ""),
        base_url: base,
        key,
        profile_id: p.id,
      },
    });
    const n = applyFetchToPick(r, {
      pick: els.connModelPick,
      builtin: ((t && t.builtin_models) || []).slice(),
      onPickChange: refreshConnGate,
    }, profileModels(p));
    if (n) setMsg(T("modelsRefreshed", { n }), "ok");
    else setMsg(T("modelsFetchFallback"), "ok");
  } catch (e) {
    setMsg(T("modelsFetchFail", { err: e }), "err");
  } finally {
    setBusy(false);
    refreshConnGate();
  }
}

function refreshConnGate() {
  const p = currentConn();
  const t = p ? tplById(p.template_id) : null;
  const capSrc = p ? profileCapabilitySource(p, t) : t;
  const need = p ? modelRequired(p.capabilities ? p : t) : false;
  const cap = capSrc ? modelCapability(capSrc) : CAP.FIXED;
  const resolved = resolvedConnectionModels(cap, els.connModelPick);
  els.connSaveBtn.disabled = busy || !!(need && !resolved.models.length);
}

async function connSave() {
  const p = currentConn();
  if (!p) { setMsg(T("profileMissing"), "err"); return; }
  const name = els.connName.value.trim();
  if (!name) { setMsg(T("providerEmpty"), "err"); return; }
  const t = tplById(p.template_id);
  const capSrc = profileCapabilitySource(p, t);
  const req = p.capabilities ? modelRequired(p) : (t ? modelRequired(t) : true);
  const cap = capSrc ? modelCapability(capSrc) : CAP.FIXED;
  const resolved = resolvedConnectionModels(cap, els.connModelPick);
  if (req && !resolved.models.length) { setMsg(T("needModel"), "err"); return; }
  const model = resolved.defaultModel || resolved.models[0] || "";
  const editable = t ? t.base_url_editable : true;
  const base = editable ? els.connBase.value.trim() : (t ? t.base_url : els.connBase.value.trim());
  // 可编辑地址的模板都是中转/自定义端点，必须带 base_url；清空后保存会得到不可用连接（激活必失败）。
  // 保存前就拦（后端也有同款守卫兜底，修 P2）。
  if (editable && !base) { setMsg(S().fillBaseUrl, "err"); return; }
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) { setMsg(baseErr, "err"); return; }
  const active = p.id === configState.active_id;
  // key 留空＝不改（后端语义）；base_url/model 照传。api_format 不在此改（保留模板值）。
  const args = {
    id: p.id,
    baseUrl: base,
    model,
    activeModels: resolved.models,
    defaultModel: resolved.defaultModel || model,
    key: els.connKey.value.trim(),
  };
  setBusy(true, { kind: "saveConnection", id: p.id });
  startSaveConnectionFeedback(p.id, active);
  try {
    const r = await call("update_profile_connection", args);
    if (name !== p.name) {
      await call("update_profile_metadata", { id: p.id, name, notes: p.notes || "" });
    }
    els.connKey.value = "";
    await loadConfig();
    // 非 active：后端如实回传 validated，连不通/native 也保存，但据实说明未校验（修 P2-d truthful-save）。
    if (active) {
      setMsg(T("savedApplied"), "ok");
    } else if (r && r.validated) {
      setMsg(T("savedValidated"), "ok");
    } else {
      setMsg(T("savedUnvalidated"), "ok");
    }
  } catch (e) {
    setMsg(T("saveConnFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

function del(id) {
  const p = (configState.profiles || []).find((x) => x.id === id);
  const nm = p ? p.name : id;
  confirmAction("delete:" + id, T("confirmDelete", { name: nm }), () => doDelete(id));
}
async function doDelete(id) {
  const wasActive = id === configState.active_id;
  setBusy(true);
  setMsg(T("deleting"));
  try {
    await call("delete_profile", { id });
    await loadConfig();
    setMsg(wasActive ? T("deletedWasActive") : T("deleted"), "ok");
  } catch (e) {
    setMsg(T("deleteFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

// 点击卡片切换当前配置：走后端切换事务（校验→起正式→健康才提交）。
// 返回体 committed:true=已生效；committed:false=未生效（可能可 skip）；抛错=回滚/中止。
async function activate(id, skipVerify) {
  hideSkip();
  setBusy(true, { kind: "activate", id });
  startActivateFeedback(id, !!skipVerify);
  try {
    const r = await call("set_active_profile", { id, skipVerify: !!skipVerify });
    if (r && r.committed) {
      await loadConfig();
      setMsg(r.hint || T("switched"), "ok");
    } else {
      await loadConfig();
      setMsg((r && r.hint) || T("switchRejected"), "err");
      if (r && r.can_skip) { pendingSkipActivateId = id; showSkip(); }
    }
  } catch (e) {
    await loadConfig();
    setMsg(T("switchFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

// ── 启动 Claude Science：读 active profile。无生效则引导先建/选一条。──
async function oneClick() {
  if (!configState.active_id) {
    setMsg(T("noActiveProfile"), "err");
    return;
  }
  setBusy(true, { kind: "oneClick" });
  startOneClickFeedback();
  try {
    const r = await call("one_click_login");
    setMsg((r.msg || T("oneClickReady")) + "\n" + (r.url || ""), "ok");
  } catch (e) {
    setMsg(T("oneClickFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

async function stopAll() {
  setBusy(true);
  setMsg(T("stopping"));
  try {
    await call("stop_all");
    setMsg(T("stopped"), "ok");
  } catch (e) {
    setMsg(T("stopFail", { err: e }), "err");
  } finally {
    setBusy(false);
  }
}

function wire() {
  [
    "oneClickBtn", "stopBtn",
    "msg", "proxyPort", "sandboxPort", "advSec",
    "listSec", "profileList", "newBtn", "listhdMoreBtn", "listhdMenu", "editCspJsonBtn", "skipActivateBtn",
    "i18nMyConfigs", "i18nLabelProvider", "i18nLabelBase", "i18nLabelKey",
    "i18nConnName", "i18nConnBase", "i18nConnKey", "i18nPorts", "i18nProxyPort", "i18nSandboxPort",
    "wizSec", "wizName", "wizPreset", "wizBase", "wizKey", "wizSaveBtn", "wizCancelBtn",
    "connSec", "connTitle", "connName", "connBase", "connBaseHint",
    "connModelInfo", "connModelHint", "connModelPick", "connKey", "connSaveBtn", "connCancelBtn",
  ].forEach((id) => (els[id] = $(id)));
  els.panel = document.querySelector(".panel");
  els.panelBody = document.querySelector(".panel-body");

  applyEditionUI();
  els.proxyPort.addEventListener("change", persistPorts);
  els.sandboxPort.addEventListener("change", persistPorts);

  // 列表：点击卡片切换当前配置；⋯ 菜单收纳编辑/清除/删除。
  els.profileList.addEventListener("click", (e) => {
    if (busy) return;
    const btn = e.target.closest("[data-act]");
    const row = e.target.closest(".prow[data-id]");
    if (btn && row) {
      const id = row.getAttribute("data-id");
      const act = btn.getAttribute("data-act");
      if (act === "menu") {
        const wrap = btn.closest(".pmenu-wrap");
        const menu = wrap && wrap.querySelector(".pmenu");
        if (!menu) return;
        const wasOpen = !menu.hidden;
        closeListhdMenu();
        closeAllMenus();
        if (!wasOpen) {
          positionProfileMenu(menu, btn);
          btn.setAttribute("aria-expanded", "true");
        }
        return;
      }
      closeAllMenus();
      if (act === "editconn") openConn(id);
      else if (act === "delete") del(id);
      return;
    }
    if (row && !e.target.closest(".pmenu-wrap")) {
      const id = row.getAttribute("data-id");
      if (id && id !== configState.active_id) activate(id, false);
    }
  });
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".pmenu-wrap")) closeAllMenus();
    if (!e.target.closest(".listhd-more")) closeListhdMenu();
  });

  els.newBtn.addEventListener("click", openWizard);
  els.listhdMoreBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    if (busy) return;
    toggleListhdMenu();
  });
  els.editCspJsonBtn.addEventListener("click", async () => {
    if (busy) return;
    closeListhdMenu();
    try {
      await call("open_csp_json");
    } catch (e) {
      setMsg(T("openCspFail", { err: e }), "err");
    }
  });
  els.skipActivateBtn.addEventListener("click", () => {
    const id = pendingSkipActivateId;
    if (id) activate(id, true);
  });

  els.wizPreset.addEventListener("change", applyWizPreset);
  els.wizName.addEventListener("input", applyWizNameAutofill);
  els.wizBase.addEventListener("input", () => {
    const base = els.wizBase.value.trim();
    if (wizLastAutoBase && base !== wizLastAutoBase) wizLastAutoBase = "";
    refreshWizGate();
  });
  els.wizKey.addEventListener("input", refreshWizGate);
  els.wizSaveBtn.addEventListener("click", wizSave);
  els.wizCancelBtn.addEventListener("click", cancelForm);

  els.connSaveBtn.addEventListener("click", connSave);
  els.connCancelBtn.addEventListener("click", cancelForm);

  els.oneClickBtn.addEventListener("click", oneClick);
  els.stopBtn.addEventListener("click", stopAll);
}

window.addEventListener("DOMContentLoaded", async () => {
  wire();
  await loadConfig();
  window.addEventListener("focus", () => {
    if (!busy) loadConfig().catch(() => {});
  });
  if (PREVIEW) {
    setMsg(T("previewMode"));
  }
});
