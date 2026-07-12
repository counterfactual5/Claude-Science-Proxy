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
    modelHintNative: "由 Science 选择器 + 内置映射自动选择（opus 深度 / haiku 快速）。",
    modelHintFixed: "勾选要在 Science 中启用的模型；列表第一个用于后台任务兜底。",
    metaManyModels: "{n} 个模型已启用",
    metaOneModel: "1 个模型已启用",
    metaBuiltinMap: "内置映射",
    metaNoModel: "未选模型",
    loadConfigFail: "读取配置失败：{err}",
    confirmRetry: "{prompt} —— 再点一次同一按钮确认（4 秒内）。",
    createFail: "创建失败：{err}",
    anthropicBaseErr: "这个地址看起来是 Anthropic 兼容端点，请填写 OpenAI 兼容 base root（如 https://api.example.com/v1）。",
    editConnActive: "编辑（当前生效）",
    connBaseHintDefault: "官方默认地址，可改到 token 套餐 / 区域端点。",
    connBaseHintOpenAIChat: "OpenAI 兼容 base root，代理自动补 /chat/completions。",
    connBaseHintOpenAIResp: "OpenAI 兼容 base root，代理自动补 /responses。",
    connBaseHintCustom: "自定义端点根地址。",
    connBaseHintNative: "模板地址（只读），模型由内置映射自动选择。",
    connBaseHintReadonly: "模板地址（只读）。",
    connKeySaved: "已保存（留空不改）",
    modelsFetchFail: "拉取模型失败：{err}",
    profileMissing: "配置不存在。",
    providerEmpty: "Provider 不能为空。",
    needModel: "该来源需要至少选一个模型才能保存。",
    saveConnFail: "连接未保存：{err}",
    confirmDelete: "将删除配置「{name}」",
    deleteFail: "删除失败：{err}",
    switchRejected: "校验未通过，未切换。",
    switchFail: "切换失败：{err}",
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
    errProxyScriptMissing: "找不到代理脚本 proxy/core/csp_proxy.py。",
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
    errSandboxScriptMissing: "找不到 scripts/sandbox/launch-virtual-sandbox.sh。",
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
    errBackgroundTaskFailed: "后台任务失败：{detail}",
    errCatalogParseFailed: "catalog JSON 解析失败：{detail}",
    errCatalogSchemaUnsupported: "不支持的 catalog schema_version：{version}",
    errCatalogRuleIdEmpty: "catalog rule id 不能为空",
    errCatalogRuleIdDuplicate: "catalog rule id 重复：{id}",
    errCatalogRuleScopeInvalid: "catalog rule scope 非法：{scope} ({id})",
    errCatalogRuleStatusInvalid: "catalog rule status 非法：{status} ({id})",
    errCatalogRuleActionInvalid: "catalog rule action 非法：{action} ({id})",
    errCatalogRuleReasonEmpty: "catalog rule reason 不能为空：{id}",
    errCatalogRuleEvidenceEmpty: "catalog rule evidence 不能为空：{id}",
    errCspJsonParse: "CSP.json 解析失败：{detail}",
    errSymlinkRejected: "拒绝符号链接（防跟随写/读到别处）：{path}",
    errConfigDirNotDir: "配置目录不是目录：{path}",
    errSchemaTooNew: "CSP.json 由更新版本（schema {v}）写入，请升级 Claude Science Proxy 后再打开。",
    errLegacyConfigParse: "旧 config 解析失败：{detail}",
    errConfigSerialize: "config 序列化失败：{detail}",
    errOauthKeyInvalidBase64: "OAUTH_ENCRYPTION_KEY 非法 base64：{detail}",
    errOauthHkdfFailed: "hkdf expand 失败",
    errOauthEncryptFailed: "aes-gcm 加密失败",
    errOauthDecryptFailed: "aes-gcm 解密/验签失败",
    errOauthV2PrefixMissing: "缺 v2: 前缀",
    errOauthV2BodyInvalid: "v2 体非法 base64：{detail}",
    errOauthV2CiphertextTooShort: "v2 密文过短",
    errOauthSymlinkRejected: "拒绝：{path} 是符号链接，绝不跟随写入。",
    errOauthNoParentDir: "目标无父目录",
    errOauthTempFileFailed: "建临时文件失败：{detail}",
    errOauthWriteTempFailed: "写临时文件失败：{detail}",
    errOauthRenameFailed: "rename 失败：{detail}",
    errOauthChmodFailed: "chmod 失败：{detail}",
    errOauthRealScienceDirRejected: "拒绝：auth_dir 解析到真实 Science 目录（{path}）之内或本身，铁律禁止触碰。",
    errOauthOutsideSandboxRejected: "拒绝：auth_dir 解析到沙箱根之外（{path} 不在 {root} 下），疑似符号链接重定向。",
    errOauthEmailInvalid: "拒绝：email 必须以 localhost.invalid 结尾（当前 {email}），确保是假账号。",
    errOauthMkdirFailed: "建 auth_dir 失败：{detail}",
    errOauthReadEncryptionKeyFailed: "读 encryption.key 失败：{detail}",
    errOauthSerializeBlobFailed: "序列化 blob 失败：{detail}",
    errOauthEncryptionKeyMissing: "缺 OAUTH_ENCRYPTION_KEY",
    errOauthMkdirTokensFailed: "建 .oauth-tokens 失败：{detail}",
    errOauthDeleteOldTokenFailed: "删除旧令牌 {path} 失败：{detail}（需目录内恰好一个 .enc）",
    errOauthSelfVerifyParseFailed: "自校验解析失败：{detail}",
    errOauthSelfVerifyEmailMismatch: "自校验失败：解密回读的 email 不符",
    errOauthNoHome: "无 HOME 环境变量",
    errOauthOrphanOrgs: "检测到 {count} 个历史组织，但 active-org.json 缺失且无可解令牌，无法确定当前活动组织；为避免旧对话被孤儿化已中止。数据都在 {orgs_dir}，请把想要的 org_uuid 写回 {active_org_path} 后重试。",
    errPortReserved8765: "端口 8765 是真实 Science 实例保留端口，不能用。",
    errPortZero: "端口不能为 0。",
    errPortSame: "代理端口与沙箱端口不能相同。",
    errPortSandboxStopFailed: "端口未更改：无法停止指向旧端口的沙箱（{error}）。为避免留下失效链路，端口保持不变。请手动停止沙箱或重启 app 后重试。（真实实例 8765 未受影响）",
    errStopSandboxFailed: "代理已停；但{error}（真实实例 8765 未受影响）。",
    errScratchNoPort: "无法分配临时端口",
    errScratchNoSecret: "无法生成 secret",
    errScratchSpawnFailed: "起临时代理失败：{detail}",
    errScratchNotReady: "临时代理未就绪（多为 key/base_url 无效或依赖缺失）",
    noActiveProfile: "还没有「当前生效」的配置。请先「＋ 新建」或点击一条配置切换，再点「启动 Claude Science」。",
    oneClickFail: "启动失败：{err}",
    stopFail: "停止失败：{err}",
    openCspFail: "打开 CSP.json 失败：{err}",
    tplName_deepseek: "DeepSeek",
    tplName_glm: "智谱 GLM",
    tplName_xiaomi: "小米 MiMo",
    tplName_kimi: "Kimi（Moonshot）",
    tplName_minimax: "MiniMax",
    tplName_openrouter: "OpenRouter",
    tplName_custom_openai: "自定义 OpenAI",
    tplName_custom_openai_responses: "自定义 OpenAI Responses",
    tplName_custom: "自定义 Anthropic",
    wizPresetLabel_glm_coding: "智谱 Coding Plan",
    wizPresetLabel_xiaomi_token: "小米 MiMo · Token 套餐",
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
    modelHintNative: "Auto-mapped via Science picker + built-in routing (opus for depth, haiku for speed).",
    modelHintFixed: "Check models to enable in Science; the first is the fallback for background tasks.",
    metaManyModels: "{n} models enabled",
    metaOneModel: "1 model enabled",
    metaBuiltinMap: "built-in mapping",
    metaNoModel: "no model selected",
    loadConfigFail: "Failed to load config: {err}",
    confirmRetry: "{prompt} — click the same button again to confirm (within 4s).",
    createFail: "Create failed: {err}",
    anthropicBaseErr: "This URL looks Anthropic-compatible. Use an OpenAI-compatible base root (e.g. https://api.example.com/v1).",
    editConnActive: "Edit (active)",
    connBaseHintDefault: "Official default; change for token plans / regional endpoints.",
    connBaseHintOpenAIChat: "OpenAI-compatible base root; proxy appends /chat/completions.",
    connBaseHintOpenAIResp: "OpenAI-compatible base root; proxy appends /responses.",
    connBaseHintCustom: "Custom endpoint root URL.",
    connBaseHintNative: "Template URL (read-only); models use built-in mapping.",
    connBaseHintReadonly: "Template URL (read-only).",
    connKeySaved: "Saved (leave blank to keep)",
    modelsFetchFail: "Fetch models failed: {err}",
    profileMissing: "Profile not found.",
    providerEmpty: "Provider name is required.",
    needModel: "Select at least one model for this provider.",
    saveConnFail: "Not saved: {err}",
    confirmDelete: "Delete profile \"{name}\"",
    deleteFail: "Delete failed: {err}",
    switchRejected: "Verify failed; not switched.",
    switchFail: "Switch failed: {err}",
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
    errProxyScriptMissing: "Proxy script proxy/core/csp_proxy.py not found.",
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
    errSandboxScriptMissing: "Sandbox script scripts/sandbox/launch-virtual-sandbox.sh not found.",
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
    errBackgroundTaskFailed: "Background task failed: {detail}",
    errCatalogParseFailed: "Failed to parse catalog JSON: {detail}",
    errCatalogSchemaUnsupported: "Unsupported catalog schema_version: {version}",
    errCatalogRuleIdEmpty: "Catalog rule id cannot be empty.",
    errCatalogRuleIdDuplicate: "Duplicate catalog rule id: {id}",
    errCatalogRuleScopeInvalid: "Invalid catalog rule scope: {scope} ({id})",
    errCatalogRuleStatusInvalid: "Invalid catalog rule status: {status} ({id})",
    errCatalogRuleActionInvalid: "Invalid catalog rule action: {action} ({id})",
    errCatalogRuleReasonEmpty: "Catalog rule reason cannot be empty: {id}",
    errCatalogRuleEvidenceEmpty: "Catalog rule evidence cannot be empty: {id}",
    errCspJsonParse: "Failed to parse CSP.json: {detail}",
    errSymlinkRejected: "Symlinks are rejected (prevent follow attacks on read/write): {path}",
    errConfigDirNotDir: "Config path is not a directory: {path}",
    errSchemaTooNew: "CSP.json was written by a newer app (schema {v}). Please upgrade Claude Science Proxy before opening.",
    errLegacyConfigParse: "Failed to parse legacy config: {detail}",
    errConfigSerialize: "Failed to serialize config: {detail}",
    errOauthKeyInvalidBase64: "OAUTH_ENCRYPTION_KEY is not valid base64: {detail}",
    errOauthHkdfFailed: "HKDF expand failed.",
    errOauthEncryptFailed: "AES-GCM encryption failed.",
    errOauthDecryptFailed: "AES-GCM decryption or authentication failed.",
    errOauthV2PrefixMissing: "Missing v2: prefix.",
    errOauthV2BodyInvalid: "v2 body is not valid base64: {detail}",
    errOauthV2CiphertextTooShort: "v2 ciphertext is too short.",
    errOauthSymlinkRejected: "Rejected: {path} is a symlink; writes never follow symlinks.",
    errOauthNoParentDir: "Target has no parent directory.",
    errOauthTempFileFailed: "Failed to create temp file: {detail}",
    errOauthWriteTempFailed: "Failed to write temp file: {detail}",
    errOauthRenameFailed: "rename failed: {detail}",
    errOauthChmodFailed: "chmod failed: {detail}",
    errOauthRealScienceDirRejected: "Rejected: auth_dir resolves inside or equals the real Science directory ({path}); touching it is forbidden.",
    errOauthOutsideSandboxRejected: "Rejected: auth_dir resolves outside the sandbox root ({path} is not under {root}); possible symlink redirect.",
    errOauthEmailInvalid: "Rejected: email must end with localhost.invalid (current: {email}) to ensure a fake account.",
    errOauthMkdirFailed: "Failed to create auth_dir: {detail}",
    errOauthReadEncryptionKeyFailed: "Failed to read encryption.key: {detail}",
    errOauthSerializeBlobFailed: "Failed to serialize token blob: {detail}",
    errOauthEncryptionKeyMissing: "Missing OAUTH_ENCRYPTION_KEY.",
    errOauthMkdirTokensFailed: "Failed to create .oauth-tokens: {detail}",
    errOauthDeleteOldTokenFailed: "Failed to delete old token {path}: {detail} (directory must contain exactly one .enc).",
    errOauthSelfVerifyParseFailed: "Self-check parse failed: {detail}",
    errOauthSelfVerifyEmailMismatch: "Self-check failed: decrypted email does not match.",
    errOauthNoHome: "HOME environment variable is not set.",
    errOauthOrphanOrgs: "Found {count} historical orgs but active-org.json is missing and no decryptable token exists; cannot determine the active org. Aborted to avoid orphaning old conversations. Data is under {orgs_dir}; write the desired org_uuid to {active_org_path} and retry.",
    errPortReserved8765: "Port 8765 is reserved for the real Science instance and cannot be used.",
    errPortZero: "Port cannot be 0.",
    errPortSame: "Proxy port and sandbox port must differ.",
    errPortSandboxStopFailed: "Ports unchanged: could not stop sandbox on old port ({error}). Ports kept to avoid a broken chain. Stop sandbox manually or restart the app. (Real instance on 8765 unaffected.)",
    errStopSandboxFailed: "Proxy stopped; but {error} (real instance on 8765 unaffected).",
    errScratchNoPort: "Could not allocate a scratch port.",
    errScratchNoSecret: "Could not generate secret.",
    errScratchSpawnFailed: "Failed to start scratch proxy: {detail}",
    errScratchNotReady: "Scratch proxy not ready (often invalid key/base_url or missing deps).",
    noActiveProfile: "No active profile. Create or select one, then Start Claude Science.",
    oneClickFail: "Start failed: {err}",
    stopFail: "Stop failed: {err}",
    openCspFail: "Failed to open CSP.json: {err}",
    tplName_deepseek: "DeepSeek",
    tplName_glm: "GLM",
    tplName_xiaomi: "MiMo",
    tplName_kimi: "Kimi (Moonshot)",
    tplName_minimax: "MiniMax",
    tplName_openrouter: "OpenRouter",
    tplName_custom_openai: "Custom OpenAI",
    tplName_custom_openai_responses: "Custom OpenAI Responses",
    tplName_custom: "Custom Anthropic",
    wizPresetLabel_xiaomi_token: "MiMo · Token Plan",
  },
};
function S() { return I18N[EDITION]; }
function templateDisplayName(id, fallback) {
  const key = `tplName_${id.replace(/-/g, "_")}`;
  const t = S()[key];
  return t || fallback || id;
}
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
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "glm", templateId: "glm", name: "GLM", baseUrl: "https://open.bigmodel.cn/api/anthropic" },
  { id: "glm-coding", templateId: "custom-openai", name: "GLM Coding Plan", baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4" },
  { id: "kimi", templateId: "kimi", name: "Moonshot", baseUrl: "https://api.moonshot.cn/anthropic" },
  { id: "minimax", templateId: "minimax", name: "MiniMax", baseUrl: "https://api.minimaxi.com/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token", templateId: "xiaomi", name: "MiMo", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
];

/** Intl edition: common overseas endpoints. */
const WIZ_PRESETS_INTL = [
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "glm", templateId: "glm", name: "ZAI", baseUrl: "https://api.z.ai/api/anthropic" },
  { id: "kimi", templateId: "kimi", name: "Moonshot", baseUrl: "https://api.moonshot.ai/anthropic" },
  { id: "minimax", templateId: "minimax", name: "MiniMax", baseUrl: "https://api.minimax.io/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token", templateId: "xiaomi", name: "MiMo", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
  { id: "openrouter", templateId: "openrouter", name: "OpenRouter", baseUrl: "https://openrouter.ai/api" },
];

function wizPresets() {
  return EDITION === "cn" ? WIZ_PRESETS_CN : WIZ_PRESETS_INTL;
}

function wizPresetLabel(preset) {
  const presetKey = `wizPresetLabel_${preset.id.replace(/-/g, "_")}`;
  const localized = S()[presetKey];
  if (localized) return localized;
  return templateDisplayName(preset.templateId, preset.name);
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
  schema_version: 4,
  active_id: "",
  proxy_port: 18991,
  sandbox_port: 8990,
  profiles: [
    { id: "p-demo1", name: "GLM", template_id: "glm", api_format: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", model: "glm-5.2", active_models: ["glm-5.2"], key: "••••••1234", icon: "glm", icon_color: "#2E6BE6", notes: "" },
  ],
  skills: [
    { id: "sk_1", name: "AlphaFold Database Fetch & Analyze", description: "Retrieve and analyze AlphaFold predicted structures for a protein.", enabled: true, sizeBytes: 12450, importedAt: "2026-07-12T02:30:00Z", requirements: ["python"] }
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
        id, name: args.name || templateDisplayName(t.id, t.name) || "Profile", template_id: args.templateId,
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
      return Promise.resolve({ committed: true, active_id: args.id });
    }
    case "fetch_models":
      return Promise.resolve({ models: [{ id: "glm-4.6", supports_tools: true }, { id: "glm-5", supports_tools: null }], source: "live", error_kind: null, upstream_status: 200 });
    case "set_settings":
      if (args.cfg) { mockStore.proxy_port = args.cfg.proxy_port; mockStore.sandbox_port = args.cfg.sandbox_port; }
      return Promise.resolve(null);
    case "one_click_login":
      return Promise.resolve({ url: "http://127.0.0.1:8990", action: "started" });
    case "open_csp_json":
      return Promise.resolve("~/.csp/CSP.json");
    case "list_skills":
      return Promise.resolve(mockStore.skills.map((s) => ({ ...s })));
    case "inspect_skill_source": {
      const path = (args.input && args.input.sourcePath) || "";
      const valid = path.length > 0;
      if (!valid) {
        return Promise.reject("Invalid path supplied");
      }
      return Promise.resolve({
        valid,
        name: "Local Skill",
        description: "Inspected skill from path: " + path,
        fileCount: 3,
        totalSizeBytes: 89000,
        requirements: ["python", "mcp"],
        warnings: [],
        errors: [],
      });
    }
    case "import_skill": {
      const path = (args.input && args.input.sourcePath) || "";
      const id = "sk_" + Math.random().toString(16).slice(2, 10);
      const newSkill = {
        id,
        name: "Imported Skill",
        description: "Skill imported from: " + path,
        storePath: "/mock/store/" + id,
        sourcePath: path,
        enabled: true,
        sizeBytes: 89000,
        importedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        requirements: ["python", "mcp"],
      };
      mockStore.skills.push(newSkill);
      return Promise.resolve(newSkill);
    }
    case "set_skill_enabled": {
      const input = args.input || {};
      const s = mockStore.skills.find((x) => x.id === input.skillId);
      if (s) s.enabled = !!input.enabled;
      return Promise.resolve(s);
    }
    case "remove_skill": {
      const input = args.input || {};
      mockStore.skills = mockStore.skills.filter((x) => x.id !== input.skillId);
      return Promise.resolve(null);
    }
    default:
      return Promise.resolve(null);
  }
}

const $ = (id) => document.getElementById(id);
const els = {};
let busy = false;
let busyOp = null;
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
  if (t.adapter === "deepseek") return CAP.NATIVE;
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

function setMsg(text) {
  // Feedback area shows errors only; success/progress/neutral messages take no space.
  const t = text ? String(text) : "";
  els.msg.textContent = t;
  els.msg.className = "msg" + (t ? " err" : "");
  els.msg.parentElement.hidden = !t;
  if (t && els.panel && els.panel.classList.contains("view-form")) {
    els.msg.scrollIntoView({ block: "nearest" });
  }
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

function setBusy(on, op) {
  busy = on;
  busyOp = on ? (op || { kind: "global" }) : null;
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
function cancelForm() { showView("list"); }

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
    timer: setTimeout(() => { pendingConfirm = null; }, 4000),
  };
  setMsg(T("confirmRetry", { prompt: promptText }));
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
  } catch (e) {
    setMsg(T("loadConfigFail", { err: resolveBackendErr(e) }));
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
  // Keep busy for entire port submit: initial `if (busy) return` only blocks re-entry while already busy,
  // not other ops (profile activate / one-click / connection edit) started while this call is in flight. Busy + disabled controls enforce order. GPT round-3 P2
  setBusy(true, { kind: "ports" });
  try {
    await call("set_settings", { cfg: { proxy_port: p, sandbox_port: s } });
    configState.proxy_port = p;
    configState.sandbox_port = s;
  } catch (e) {
    // Error = ports not persisted (validation failed / stop old sandbox failed): reset inputs to effective values so UI doesn't show unsaved numbers.
    els.proxyPort.value = configState.proxy_port;
    els.sandboxPort.value = configState.sandbox_port;
    setMsg(resolveBackendErr(e));
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
    (item) => item.name.toLowerCase() === q || wizPresetLabel(item).toLowerCase() === q
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
    opt.textContent = wizPresetLabel(item);
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
  if ((norm.includes("open.bigmodel.cn") || norm.includes("api.z.ai")) && norm.includes("/anthropic")) return "glm";
  if (norm.includes("xiaomimimo.com") || norm.includes("token-plan")) return "xiaomi";
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
  if (!name) { setMsg(S().fillProvider); return; }
  if (!base) { setMsg(S().fillBaseUrl); return; }
  if (!key) { setMsg(S().fillApiKey); return; }
  const templateId = inferTemplateId(base);
  const t = tplById(templateId);
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) { setMsg(baseErr); return; }
  setBusy(true);
  try {
    const id = await call("create_profile", { templateId, name, key, baseUrl: base, model: "" });
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
  } catch (e) {
    setMsg(T("createFail", { err: resolveBackendErr(e) }));
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
  // native (deepseek): hide "fetch models" button—don't hint at a nonexistent action (#5).
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
    applyFetchToPick(r, {
      pick: els.connModelPick,
      builtin: ((t && t.builtin_models) || []).slice(),
      onPickChange: refreshConnGate,
    }, profileModels(p));
  } catch (e) {
    setMsg(T("modelsFetchFail", { err: resolveBackendErr(e) }));
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
  if (!p) { setMsg(T("profileMissing")); return; }
  const name = els.connName.value.trim();
  if (!name) { setMsg(T("providerEmpty")); return; }
  const t = tplById(p.template_id);
  const capSrc = profileCapabilitySource(p, t);
  const req = p.capabilities ? modelRequired(p) : (t ? modelRequired(t) : true);
  const cap = capSrc ? modelCapability(capSrc) : CAP.FIXED;
  const resolved = resolvedConnectionModels(cap, els.connModelPick);
  if (req && !resolved.models.length) { setMsg(T("needModel")); return; }
  const model = resolved.defaultModel || resolved.models[0] || "";
  const editable = t ? t.base_url_editable : true;
  const base = editable ? els.connBase.value.trim() : (t ? t.base_url : els.connBase.value.trim());
  // Editable-url templates are relay/custom endpoints and require base_url; saving empty yields unusable connection (activation fails).
  // Block before save (backend has matching guard; P2).
  if (editable && !base) { setMsg(S().fillBaseUrl); return; }
  const baseErr = openaiCustomAnthropicBaseMessage(t, base);
  if (baseErr) { setMsg(baseErr); return; }
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
  try {
    await call("update_profile_connection", args);
    if (name !== p.name) {
      await call("update_profile_metadata", { id: p.id, name, notes: p.notes || "" });
    }
    els.connKey.value = "";
    await loadConfig();
  } catch (e) {
    setMsg(T("saveConnFail", { err: resolveBackendErr(e) }));
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
  setBusy(true);
  try {
    await call("delete_profile", { id });
    await loadConfig();
  } catch (e) {
    setMsg(T("deleteFail", { err: resolveBackendErr(e) }));
  } finally {
    setBusy(false);
  }
}

// Click card to switch active profile: backend switch transaction (verify → start production → health check before commit).
// Response committed:true = applied; committed:false = not applied (may allow skip); throw = rollback/abort.
async function activate(id, skipVerify) {
  hideSkip();
  setBusy(true, { kind: "activate", id });
  try {
    const r = await call("set_active_profile", { id, skipVerify: !!skipVerify });
    if (r && r.committed) {
      await loadConfig();
    } else {
      await loadConfig();
      setMsg(resolveHint(r, "switchRejected"));
      if (r && r.can_skip) { pendingSkipActivateId = id; showSkip(); }
    }
  } catch (e) {
    await loadConfig();
    setMsg(T("switchFail", { err: resolveBackendErr(e) }));
  } finally {
    setBusy(false);
  }
}

// ── Launch Claude Science: read active profile. If none active, prompt user to create/select first. ──
async function oneClick() {
  if (!configState.active_id) {
    setMsg(T("noActiveProfile"));
    return;
  }
  setBusy(true, { kind: "oneClick" });
  try {
    await call("one_click_login");
  } catch (e) {
    setMsg(T("oneClickFail", { err: resolveBackendErr(e) }));
  } finally {
    setBusy(false);
  }
}

async function stopAll() {
  setBusy(true);
  try {
    await call("stop_all");
  } catch (e) {
    setMsg(T("stopFail", { err: resolveBackendErr(e) }));
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
    "tabProfiles", "tabSkills", "skillPane",
    "skillImportBtn", "skillEmpty", "skillList", "skillMsg",
    "skillImportModal", "skillSourcePath", "skillInspectionPreview",
    "inspName", "inspDesc", "inspStats", "inspReqs", "inspWarnings", "inspErrors",
    "skillInspectBtn", "skillImportConfirmBtn", "skillImportCancelBtn",
  ].forEach((id) => (els[id] = $(id)));
  els.panel = document.querySelector(".panel");
  els.panelBody = document.querySelector(".panel-body");

  applyEditionUI();
  els.proxyPort.addEventListener("change", persistPorts);
  els.sandboxPort.addEventListener("change", persistPorts);

  // List: click card to switch active profile; ⋯ menu holds edit/clear/delete.
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
      setMsg(T("openCspFail", { err: resolveBackendErr(e) }));
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

  // Tabs navigation
  els.tabProfiles.addEventListener("click", () => switchTab("profiles"));
  els.tabSkills.addEventListener("click", () => switchTab("skills"));

  // Skills panel actions
  els.skillImportBtn.addEventListener("click", openSkillImport);
  els.skillInspectBtn.addEventListener("click", inspectSkillSource);
  els.skillImportConfirmBtn.addEventListener("click", importSkillConfirm);
  els.skillImportCancelBtn.addEventListener("click", closeSkillImport);

  // Skill list interactions
  els.skillList.addEventListener("click", (e) => {
    if (busy) return;
    const btn = e.target.closest("[data-act]");
    const row = e.target.closest(".skill-row[data-id]");
    if (row) {
      const id = row.getAttribute("data-id");
      const name = row.getAttribute("data-name");
      if (btn) {
        const act = btn.getAttribute("data-act");
        if (act === "delete") {
          removeSkill(id, name);
        }
      } else if (e.target.type === "checkbox") {
        toggleSkill(id, e.target.checked);
      }
    }
  });

  els.skillSourcePath.addEventListener("input", () => {
    els.skillImportConfirmBtn.disabled = true;
    els.skillInspectionPreview.hidden = true;
  });
}

// ── Tabs Navigation ──
function switchTab(tab) {
  if (busy) return;
  hideSkip();
  closeAllMenus();
  closeListhdMenu();
  setMsg("");
  setSkillMsg("");

  if (tab === "profiles") {
    els.tabProfiles.classList.add("active");
    els.tabProfiles.setAttribute("aria-selected", "true");
    els.tabSkills.classList.remove("active");
    els.tabSkills.setAttribute("aria-selected", "false");
    els.panelBody.hidden = false;
    els.skillPane.hidden = true;
    els.advSec.hidden = false;
    showView("list");
  } else if (tab === "skills") {
    els.tabProfiles.classList.remove("active");
    els.tabProfiles.setAttribute("aria-selected", "false");
    els.tabSkills.classList.add("active");
    els.tabSkills.setAttribute("aria-selected", "true");
    els.panelBody.hidden = true;
    els.skillPane.hidden = false;
    els.advSec.hidden = true;
    loadSkills();
  }
}

// ── Skill Manager ──
async function loadSkills() {
  try {
    const list = await call("list_skills");
    renderSkills(list || []);
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  }
}

function setSkillMsg(text) {
  const t = text ? String(text) : "";
  els.skillMsg.textContent = t;
  els.skillMsg.className = "msg" + (t ? " err" : "");
  els.skillMsg.parentElement.hidden = !t;
}

function renderSkills(list) {
  if (!list.length) {
    els.skillEmpty.hidden = false;
    els.skillList.innerHTML = "";
    return;
  }
  els.skillEmpty.hidden = true;
  els.skillList.innerHTML = list.map((s) => {
    const enabledClass = s.enabled ? "" : " disabled";
    const checked = s.enabled ? " checked" : "";
    const reqTags = (s.requirements || []).map(r => `<span class="skill-req-tag">${escapeHtml(r)}</span>`).join("");
    const dateText = formatImportedAt(s.importedAt);
    return `
      <div class="skill-row${enabledClass}" data-id="${escapeHtml(s.id)}" data-name="${escapeHtml(s.name)}">
        <div class="skill-row-top">
          <div class="skill-title-group">
            <input type="checkbox"${checked} />
            <span class="skill-name" title="${escapeHtml(s.name)}">${escapeHtml(s.name)}</span>
          </div>
          <button class="abtn pmenu-item danger small" data-act="delete" style="padding: 2px 6px;">${escapeHtml(S().delete || "删除")}</button>
        </div>
        ${s.description ? `<div class="skill-desc">${escapeHtml(s.description)}</div>` : ""}
        <div class="skill-meta">
          <span>大小: ${escapeHtml(formatBytes(s.sizeBytes))}</span>
          ${dateText ? `· <span>导入: ${escapeHtml(dateText)}</span>` : ""}
          ${reqTags ? `· <div class="skill-reqs-list">${reqTags}</div>` : ""}
        </div>
      </div>
    `;
  }).join("");
}

// Backend now emits ISO 8601 (e.g. 2026-07-12T10:30:00Z). Older inventories may
// still hold the legacy `epoch:<secs>` form, so handle both.
// Backend now emits ISO 8601 (e.g. 2026-07-12T10:30:00Z). Older inventories may
// still hold the legacy `epoch:<secs>` form, so handle both.
function formatImportedAt(raw) {
  if (!raw) return "";
  let d = null;
  if (raw.startsWith("epoch:")) {
    const secs = parseInt(raw.split(":")[1], 10);
    if (!Number.isNaN(secs)) d = new Date(secs * 1000);
  } else {
    const parsed = new Date(raw);
    if (!Number.isNaN(parsed.getTime())) d = parsed;
  }
  return d ? d.toLocaleDateString() : raw;
}

function formatBytes(bytes) {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

async function toggleSkill(id, enabled) {
  setBusy(true);
  try {
    await call("set_skill_enabled", { input: { skillId: id, enabled } });
    await loadSkills();
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

function removeSkill(id, name) {
  confirmAction("remove-skill:" + id, T("confirmDelete", { name }) || `将删除 Skill「${name}」`, () => doRemoveSkill(id));
}

async function doRemoveSkill(id) {
  setBusy(true);
  try {
    await call("remove_skill", { input: { skillId: id } });
    await loadSkills();
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

// ── Skill Import Modal ──
// ── Skill Import Modal ──
function openSkillImport() {
  if (busy) return;
  els.skillSourcePath.value = "";
  els.skillImportConfirmBtn.disabled = true;
  els.skillInspectionPreview.hidden = true;
  els.skillImportModal.hidden = false;
  els.skillSourcePath.focus();
}

function closeSkillImport() {
  els.skillImportModal.hidden = true;
}

async function inspectSkillSource() {
  const path = els.skillSourcePath.value.trim();
  if (!path) return;
  setBusy(true);
  try {
    const data = await call("inspect_skill_source", { input: { sourcePath: path } });
    els.inspName.textContent = data.name || "未命名 Skill";
    els.inspDesc.textContent = data.description || "无描述";
    els.inspStats.textContent = `文件数: ${data.fileCount} · 大小: ${formatBytes(data.totalSizeBytes)}`;
    els.inspReqs.textContent = `依赖环境: ${(data.requirements || []).join(", ") || "无"}`;
    
    if (data.warnings && data.warnings.length) {
      els.inspWarnings.hidden = false;
      els.inspWarnings.textContent = `警告: ${data.warnings.join("; ")}`;
    } else {
      els.inspWarnings.hidden = true;
    }
    
    if (data.errors && data.errors.length) {
      els.inspErrors.hidden = false;
      els.inspErrors.textContent = `错误: ${data.errors.join("; ")}`;
    } else {
      els.inspErrors.hidden = true;
    }

    els.skillInspectionPreview.hidden = false;
    els.skillImportConfirmBtn.disabled = !data.valid;
  } catch (e) {
    els.inspName.textContent = "";
    els.inspDesc.textContent = "";
    els.inspStats.textContent = "";
    els.inspReqs.textContent = "";
    els.inspWarnings.hidden = true;
    els.inspErrors.hidden = false;
    els.inspErrors.textContent = resolveBackendErr(e);
    els.skillInspectionPreview.hidden = false;
    els.skillImportConfirmBtn.disabled = true;
  } finally {
    setBusy(false);
  }
}

async function importSkillConfirm() {
  const path = els.skillSourcePath.value.trim();
  if (!path) return;
  setBusy(true);
  try {
    await call("import_skill", { input: { sourcePath: path } });
    closeSkillImport();
    await loadSkills();
  } catch (e) {
    els.inspErrors.hidden = false;
    els.inspErrors.textContent = resolveBackendErr(e);
  } finally {
    setBusy(false);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  wire();
  await loadConfig();
  window.addEventListener("focus", () => {
    if (!busy) {
      loadConfig().catch(() => {});
      if (!els.skillPane.hidden) loadSkills().catch(() => {});
    }
  });
});
