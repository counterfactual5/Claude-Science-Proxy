// Claude Science Proxy desktop panel frontend. Only calls backend Tauri commands; never touches key persistence logic.
// Backend echoes only masked keys here; full keys never enter the frontend.
//
// ── Tauri argument key conventions (must follow) ───────────────────────────────────────────────
// All commands use bare `#[tauri::command]` (no rename_all). tauri-macros defaults to
// `ArgumentCase::Camel`, converting Rust snake_case top-level param names to lowerCamelCase for JS:
//   template_id→templateId、base_url→baseUrl、api_format→apiFormat、skip_verify→skipVerify。
// So invoke top-level args use camelCase. Serde struct args (`req`=FetchModelsReq,
// `cfg`=UiSettings) keep snake_case field names internally: proxy_port/sandbox_port,
// template_id/base_url/key/profile_id.
//
// Preview fallback: when opened in a regular browser (no Tauri backend), mockInvoke returns fake data
// so the UI renders fully. In the real app, window.__TAURI__ exists and uses the real backend; this fallback does not run.
const PREVIEW = !window.__TAURI__;
const invoke = PREVIEW
  ? (cmd, args) => mockInvoke(cmd, args)
  : window.__TAURI__.core.invoke;

// ── Edition: domestic cn / international intl (language + provider list) ─────────────────────────────
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
    switchCommitted: "已切到「{name}」。",
    switchConnSavedApplied: "已保存并应用「{name}」的新连接。",
    switchUpstreamAuthEdit: "上游拒绝（{code}），key/权限有误，未保存（仍在用原配置运行）。",
    switchUpstreamAuthSwitch: "上游拒绝（{code}），key/权限有误，未切换（当前配置不变）。",
    switchUpstreamModelEdit: "上游拒绝该模型（{code}），未保存。请换一个模型或核对 base_url。",
    switchUpstreamModelSwitch: "上游拒绝该模型（{code}），未切换。请换一个模型或核对 base_url。",
    switchUpstreamAmbiguousEdit: "无法确认（网络/上游繁忙），未保存。可重试，或用「跳过验证」。",
    switchUpstreamAmbiguousSwitch: "无法确认（网络/上游繁忙），未切换。可重试，或用「跳过验证」。",
    switchProxyRollbackEdit: "连接已校验通过，但正式代理启动/探活失败，连接未保存，{rollback}。",
    switchProxyRollbackSwitch: "候选配置校验通过，但正式代理启动/探活失败，{rollback}。",
    switchAbortBeforeStartEdit: "连接上游校验失败（key/base_url/网络？），连接未保存。",
    switchAbortBeforeStartSwitch: "候选上游校验失败（key/base_url/网络？），未切换。",
    rollbackRestored: "已回滚到原配置（沙箱未受影响）",
    rollbackProxyStopped: "回滚未成功：代理当前已停，请重试或手动「一键开始」（沙箱未受影响）",
    errProfileNotFound: "找不到 profile：{id}",
    errMissingApiKey: "「{name}」还没填 API key，请先填写。",
    errMissingBaseUrl: "该配置需要填 base_url（http:// 或 https:// 开头）。",
    errMissingModel: "该配置需要选择或填写一个模型（中转/自定义端点必填），请在连接编辑里补上。",
    errProxyScriptMissing: "找不到代理脚本 proxy/csp_proxy.py。",
    errPythonMissing: "缺少依赖 python3（起临时代理需要）。",
    errConfigWriteFailed: "校验通过、代理已起，但写盘失败（{error}），{rollback}。请检查磁盘空间/权限后重试。",
    errRelayMissingBaseUrl: "中转 / 自定义端点必须填写连接地址（base_url），连接未保存。",
    errRelayMissingModel: "中转 / 自定义端点必须选择或填写一个模型，连接未保存。",
    errMissingApiKeyPanel: "「{name}」还没填 API key，请先在面板填写并保存。",
    errMissingBaseUrlPanel: "「{name}」需要填 base_url（如 https://your-relay/claude），请先在面板填写并保存。",
    errProxyStartFailed: "启动代理失败：{error}",
    errConnValidateFailed: "连接校验未通过，连接未保存。",
    errProxyStartSuperseded: "代理启动期间配置已变更（被更晚的操作取代），本次启动未生效。",
    errProxyPortOccupied: "端口 {port} 已被占用，换个端口或先停掉占用进程后重试。",
    errProxyHealthTimeout: "代理起后探活超时（端口 {port}）：多为 python 依赖缺失或代理脚本异常，请查看代理日志。",
    errSandboxScriptMissing: "找不到 scripts/launch-virtual-sandbox.sh。",
    errSandboxVirtualLoginFailed: "写虚拟登录失败：{error}",
    errSandboxLogOpenFailed: "建沙箱日志失败：{error}",
    errSandboxSpawnFailed: "起沙箱失败：{error}",
    errSandboxLaunchScriptFailed: "起沙箱脚本失败。\n{tail}",
    errSandboxHealthTimeout: "沙箱起后探活超时（端口 {port}）。已尝试停掉刚起的沙箱。\n{tail}",
    errSandboxIdentityMismatch: "端口 {port} 有服务响应，但按 data-dir 确认不是本沙箱 Science（疑似被其它服务占用）。已尝试停掉刚起的沙箱。",
    errStopSandboxScriptExit: "停止沙箱脚本非零退出（{code}）。",
    errStopSandboxScriptInvokeFailed: "调用停止沙箱脚本失败：{error}",
    errStopSandboxScriptMissing: "找不到停止脚本 {path}，无法确认沙箱已停止（沙箱可能仍在运行）。",
    errStopSandboxAssetRootMissing: "定位不到资源根，取不到停止脚本，无法确认沙箱已停止（沙箱可能仍在运行）。",
    errGenSecretFailed: "无法生成安全 secret：{error}",
    errLogOpenFailed: "建日志失败：{error}",
    errOpenEditorFailed: "打开编辑器失败：{error}",
    errOpenBrowserFailed: "打开浏览器失败：{error}",
    errOpenCommandFailed: "open 非零退出（{code}）",
    errMissingApiKeyToken: "请先填写 API Key / Token。",
    errUnknownTemplate: "未知模板：{id}",
    errParseModelListFailed: "解析模型列表失败：{error}",
    errUpstreamAuthRejected: "上游拒绝（{code}），key 或权限可能有误。",
    errUpstreamAuthConnNotSaved: "上游拒绝（{code}），key/权限有误，连接未保存。",
    errUpstreamModelRejected: "上游拒绝该模型（{code}），连接未保存。请换一个模型或核对 base_url。",
    errApiFormatUnsupported: "api_format `{format}` 暂不支持（待 Rust 代理），请选 anthropic、openai_chat 或 openai_responses。",
    errAnthropicBaseUrlHint: "这个地址看起来是 Anthropic 兼容端点。请改选「自定义 Anthropic」，或使用 OpenAI 兼容 base root（如 https://api.moonshot.cn/v1）。",
    errPortReserved8765: "端口 8765 是真实 Science 实例保留端口，不能用。",
    errPortZero: "端口不能为 0。",
    errPortSame: "代理端口与沙箱端口不能相同。",
    previewSwitched: "（预览：已切换为当前）",
    errPortSandboxStopFailed: "端口未更改：无法停止指向旧端口的沙箱（{error}）。为避免留下失效链路，端口保持不变。请手动停止沙箱或重启 app 后重试。（真实实例 8765 未受影响）",
    errStopSandboxFailed: "代理已停；但{error}（真实实例 8765 未受影响）。",
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
    switchCommitted: "Switched to \"{name}\".",
    switchConnSavedApplied: "Saved and applied new connection for \"{name}\".",
    switchUpstreamAuthEdit: "Upstream rejected ({code}). Check API key; connection not saved (still using previous connection).",
    switchUpstreamAuthSwitch: "Upstream rejected ({code}). Check API key; not switched (current profile unchanged).",
    switchUpstreamModelEdit: "Upstream rejected model ({code}); connection not saved. Try another model or check base_url.",
    switchUpstreamModelSwitch: "Upstream rejected model ({code}); not switched. Try another model or check base_url.",
    switchUpstreamAmbiguousEdit: "Could not verify (network/upstream busy); connection not saved. Retry or use Skip verify.",
    switchUpstreamAmbiguousSwitch: "Could not verify (network/upstream busy); not switched. Retry or use Skip verify.",
    switchProxyRollbackEdit: "Connection verified but formal proxy failed; connection not saved. {rollback}",
    switchProxyRollbackSwitch: "Candidate verified but formal proxy failed. {rollback}",
    switchAbortBeforeStartEdit: "Upstream verify failed (key/base_url/network?); connection not saved.",
    switchAbortBeforeStartSwitch: "Candidate upstream verify failed (key/base_url/network?); not switched.",
    rollbackRestored: "Rolled back to previous connection (sandbox unaffected).",
    rollbackProxyStopped: "Rollback incomplete: proxy is stopped. Retry or use Start Claude Science (sandbox unaffected).",
    errProfileNotFound: "Profile not found: {id}",
    errMissingApiKey: "\"{name}\" has no API key. Add one first.",
    errMissingBaseUrl: "This profile needs a base_url (http:// or https://).",
    errMissingModel: "Select or enter a model for this relay/custom endpoint in connection edit.",
    errProxyScriptMissing: "Proxy script proxy/csp_proxy.py not found.",
    errPythonMissing: "python3 is required to start the scratch proxy.",
    errConfigWriteFailed: "Verified and proxy started, but save failed ({error}). {rollback} Check disk space/permissions.",
    errRelayMissingBaseUrl: "Relay/custom endpoint requires base_url; connection not saved.",
    errRelayMissingModel: "Relay/custom endpoint requires a model; connection not saved.",
    errMissingApiKeyPanel: "\"{name}\" has no API key. Add one in the panel and save.",
    errMissingBaseUrlPanel: "\"{name}\" needs a base_url (e.g. https://your-relay/claude). Add one in the panel and save.",
    errProxyStartFailed: "Failed to start proxy: {error}",
    errConnValidateFailed: "Connection verify failed; connection not saved.",
    errProxyStartSuperseded: "Config changed while starting proxy; this start was superseded and did not apply.",
    errProxyPortOccupied: "Port {port} is already in use. Pick another port or stop the process using it.",
    errProxyHealthTimeout: "Proxy health check timed out (port {port}). Often missing python deps or a proxy script error—check proxy logs.",
    errSandboxScriptMissing: "Sandbox script scripts/launch-virtual-sandbox.sh not found.",
    errSandboxVirtualLoginFailed: "Virtual login write failed: {error}",
    errSandboxLogOpenFailed: "Failed to open sandbox log: {error}",
    errSandboxSpawnFailed: "Failed to start sandbox: {error}",
    errSandboxLaunchScriptFailed: "Sandbox launch script failed.\n{tail}",
    errSandboxHealthTimeout: "Sandbox health check timed out (port {port}). Tried to stop the sandbox just started.\n{tail}",
    errSandboxIdentityMismatch: "Port {port} responds but data-dir check shows it is not our sandbox Science (possibly another service). Tried to stop the sandbox just started.",
    errStopSandboxScriptExit: "Stop sandbox script exited non-zero ({code}).",
    errStopSandboxScriptInvokeFailed: "Failed to invoke stop sandbox script: {error}",
    errStopSandboxScriptMissing: "Stop script {path} not found; cannot confirm sandbox stopped (it may still be running).",
    errStopSandboxAssetRootMissing: "Asset root not found; cannot locate stop script; sandbox may still be running.",
    errGenSecretFailed: "Failed to generate secure secret: {error}",
    errLogOpenFailed: "Failed to open log: {error}",
    errOpenEditorFailed: "Failed to open editor: {error}",
    errOpenBrowserFailed: "Failed to open browser: {error}",
    errOpenCommandFailed: "open exited non-zero ({code})",
    errMissingApiKeyToken: "Enter an API Key / Token first.",
    errUnknownTemplate: "Unknown template: {id}",
    errParseModelListFailed: "Failed to parse model list: {error}",
    errUpstreamAuthRejected: "Upstream rejected ({code}); key or permissions may be wrong.",
    errUpstreamAuthConnNotSaved: "Upstream rejected ({code}); key/permissions invalid; connection not saved.",
    errUpstreamModelRejected: "Upstream rejected model ({code}); connection not saved. Try another model or check base_url.",
    errApiFormatUnsupported: "api_format `{format}` is not supported yet (Rust proxy pending). Use anthropic, openai_chat, or openai_responses.",
    errAnthropicBaseUrlHint: "This URL looks like an Anthropic-compatible endpoint. Use Custom Anthropic, or an OpenAI-compatible base root (e.g. https://api.moonshot.cn/v1).",
    errPortReserved8765: "Port 8765 is reserved for the real Science instance and cannot be used.",
    errPortZero: "Port cannot be 0.",
    errPortSame: "Proxy port and sandbox port must differ.",
    previewSwitched: "(Preview: switched to active)",
    errPortSandboxStopFailed: "Ports unchanged: could not stop sandbox on old port ({error}). Ports kept to avoid a broken chain. Stop sandbox manually or restart the app. (Real instance on 8765 unaffected.)",
    errStopSandboxFailed: "Proxy stopped; but {error} (real instance on 8765 unaffected).",
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

function resolveBackendVars(vars) {
  const out = { ...(vars || {}) };
  if (out.rollback_key) {
    out.rollback = T(out.rollback_key);
    delete out.rollback_key;
  }
  if (out.error != null) out.error = resolveBackendErr(out.error);
  return out;
}

function resolveBackendErr(e) {
  const s = String(e);
  try {
    const o = JSON.parse(s);
    if (o && typeof o.i18n === "string") return T(o.i18n, resolveBackendVars(o.vars));
  } catch (_) { /* plain string */ }
  return s;
}

function resolveHint(r, fallbackKey) {
  if (r && r.hint_key) return T(r.hint_key, resolveBackendVars(r.hint_vars));
  if (r && r.hint) return r.hint;
  return T(fallbackKey);
}

function modelHints() {
  const t = S();
  return { native: t.modelHintNative, fixed: t.modelHintFixed };
}

/** CN edition: one default endpoint per provider; domestic routes only. */
const WIZ_PRESETS_CN = [
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", label: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "glm", templateId: "glm", name: "GLM", label: "智谱 GLM", baseUrl: "https://open.bigmodel.cn/api/anthropic" },
  { id: "glm-coding", templateId: "custom-openai", name: "GLM Coding Plan", label: "智谱 Coding Plan", baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4" },
  { id: "kimi", templateId: "kimi", name: "Moonshot", label: "Moonshot", baseUrl: "https://api.moonshot.cn/anthropic" },
  { id: "minimax", templateId: "minimax", name: "MiniMax", label: "MiniMax", baseUrl: "https://api.minimaxi.com/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", label: "小米 MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token", templateId: "xiaomi", name: "MiMo", label: "小米 MiMo · Token 套餐", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
];

/** Intl edition: common overseas endpoints. */
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

/** Last auto-filled base URL in the wizard; cleared after manual edits to avoid overwriting custom URLs. */
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

// ── Preview fallback mock (browser preview only) ──
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
      return Promise.resolve({ committed: true, active_id: args.id, hint_key: "previewSwitched", hint_vars: {} });
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
// Current config snapshot (get_config result). Full keys never stored here—only masks.
let configState = { profiles: [], templates: [], active_id: "", proxy_port: 18991, sandbox_port: 8990 };
let pendingSkipActivateId = null;   // when set_active validation is ambiguous, allow switch via "skip verify"
let pendingConfirm = null;          // dangerous ops (clear key / delete): "click again to confirm" state

// ── Model capability: native builtin mapping / relay multi-select fixed models ──
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
  const candidates = [];
  for (const id of builtin || []) if (!candidates.includes(id)) candidates.push(id);
  for (const id of selected || []) if (id && !candidates.includes(id)) candidates.unshift(id);
  if (!candidates.length) {
    container.hidden = true;
    container.innerHTML = "";
    return;
  }
  container.hidden = false;
  const selSet = new Set(selected && selected.length ? selected : candidates);
  container.innerHTML = candidates.map((id) => {
    const checked = selSet.has(id) ? " checked" : "";
    return '<label class="model-pick-item"><input type="checkbox" data-model="' +
      escapeHtml(id) + '"' + checked + '><span class="model-pick-label">' + escapeHtml(id) + "</span></label>";
  }).join("");
  container.querySelectorAll('input[type="checkbox"]').forEach((cb) => {
    cb.addEventListener("change", () => { if (onChange) onChange(); });
  });
}

// native: read-only hint; relay: checkbox multi-select.
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
  // Feedback area shows errors only; success/progress/neutral messages take no space.
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
    // Disable port inputs while busy: changing ports mid-operation races in-flight work (P1-c frontend).
    els.proxyPort, els.sandboxPort,
  ].forEach((b) => b && (b.disabled = on));
  syncProfileBusyState();
  // On busy release, hand model-required save gating back to gates (avoid setBusy(false) overwriting gate).
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

// ── View switching: list / new wizard / connection edit. One form visible at a time (list hidden to reduce height). ──
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

// Dangerous ops "click again to confirm" (avoid window.confirm; unreliable in Tauri webview).
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

// ── Load config + render list ──
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
    // One-time migration notice (#9): backend clears after get_config read; appears only once.
    if (cfg.pending_notice) setMsg(cfg.pending_notice, "ok");
  } catch (e) {
    setMsg(T("loadConfigFail", { err: resolveBackendErr(e) }), "err");
  }
}

// List card second line: de-emphasize model IDs; highlight capability and key status.
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

// ── Port settings (replaces old set_config; ports only, no provider/connection) ──
async function persistPorts() {
  if (busy) return; // don't change ports while busy (prevents race with in-flight ops; inputs also disabled—belt and suspenders). P1-c
  const p = parseInt(els.proxyPort.value, 10) || 18991;
  const s = parseInt(els.sandboxPort.value, 10) || 8990;
  const changed = p !== configState.proxy_port || s !== configState.sandbox_port;
  // Keep busy for entire port submit: initial `if (busy) return` only blocks re-entry while already busy,
  // not other ops (mode switch / one-click / connection edit) started while this call is in flight. Busy + disabled controls enforce order. GPT round-3 P2
  setBusy(true, { kind: "ports" });
  startPortSaveFeedback(changed);
  try {
    await call("set_settings", { cfg: { proxy_port: p, sandbox_port: s } });
    configState.proxy_port = p;
    configState.sandbox_port = s;
    // Backend tears down old proxy/sandbox on port change (otherwise stale dead links reuse old ports); tell user to restart. P1-c
    if (changed) {
      setMsg(T("portSavedReset"), "ok");
    } else {
      setMsg(T("portUnchanged"), "ok");
    }
  } catch (e) {
    // Error = ports not persisted (validation failed / stop old sandbox failed): reset inputs to effective values so UI doesn't show unsaved numbers.
    els.proxyPort.value = configState.proxy_port;
    els.sandboxPort.value = configState.sandbox_port;
    setMsg(resolveBackendErr(e), "err");
  } finally {
    setBusy(false);
  }
}

// fetch_models response → refresh checkbox pool (auto-fetch on edit page).
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
  } catch (_) { /* fall back to builtins */ }
  return fallback;
}

/** On new profile, enable first N models by default; rest via edit page or CSP.json. */
const MAX_AUTO_ENABLE_MODELS = 8;

function modelsToEnableOnCreate(discoveredIds, builtin) {
  const ids = (discoveredIds || []).filter(Boolean);
  const builtins = (builtin || []).filter(Boolean);
  const candidates = ids.length ? ids : builtins;
  return candidates.slice(0, MAX_AUTO_ENABLE_MODELS);
}

// fetch_models response → refresh checkbox pool (auto-fetch on edit page).
function applyFetchToPick(r, pickUi, selected) {
  const models = (r && r.models) || [];
  const ids = models.map((m) => m.id).filter(Boolean);
  const candidates = ids.length ? ids : (pickUi.builtin || []);
  const sel = (selected && selected.length) ? selected : candidates;
  if (pickUi.pick) renderModelPick(pickUi.pick, candidates, sel, pickUi.onPickChange);
  return ids.length;
}

// ── C2: create (provider preset + base_url + key) ──

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
    if (els.wizPreset.value) els.wizPreset.value = "";
    if (els.wizBase.readOnly) els.wizBase.readOnly = false;
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
    setMsg(T("createFail", { err: resolveBackendErr(e) }), "err");
  } finally {
    setBusy(false);
  }
}

function resolvedConnectionModels(cap, pickContainer) {
  if (cap === CAP.NATIVE) return { models: [], defaultModel: "" };
  const checked = collectCheckedModels(pickContainer);
  return { models: checked, defaultModel: checked[0] || "" };
}

// ── C3: connection edit (base_url/model/key) + clear key ──
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
  // native (deepseek/qwen): hide "fetch models" button—don't hint at a nonexistent action (#5).
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
    setMsg(T("modelsFetchFail", { err: resolveBackendErr(e) }), "err");
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
  // Editable-url templates are relay/custom endpoints and require base_url; saving empty yields unusable connection (activation fails).
  // Block before save (backend has matching guard; P2).
  if (editable && !base) { setMsg(S().fillBaseUrl, "err"); return; }
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) { setMsg(baseErr, "err"); return; }
  const active = p.id === configState.active_id;
  // empty key = unchanged (backend semantics); base_url/model sent as-is. api_format not edited here (keep template value).
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
    // non-active: backend returns validated truthfully; saves even if unreachable/native but reports unvalidated (P2-d truthful-save).
    if (active) {
      setMsg(T("savedApplied"), "ok");
    } else if (r && r.validated) {
      setMsg(T("savedValidated"), "ok");
    } else {
      setMsg(T("savedUnvalidated"), "ok");
    }
  } catch (e) {
    setMsg(T("saveConnFail", { err: resolveBackendErr(e) }), "err");
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
    setMsg(T("deleteFail", { err: resolveBackendErr(e) }), "err");
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
      setMsg(resolveHint(r, "switched"), "ok");
    } else {
      await loadConfig();
      setMsg(resolveHint(r, "switchRejected"), "err");
      if (r && r.can_skip) { pendingSkipActivateId = id; showSkip(); }
    }
  } catch (e) {
    await loadConfig();
    setMsg(T("switchFail", { err: resolveBackendErr(e) }), "err");
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
    setMsg(T("oneClickFail", { err: resolveBackendErr(e) }), "err");
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
    setMsg(T("stopFail", { err: resolveBackendErr(e) }), "err");
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
      setMsg(T("openCspFail", { err: resolveBackendErr(e) }), "err");
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
