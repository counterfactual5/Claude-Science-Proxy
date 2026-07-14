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
    newBtn: "+ 新建",
    presetPicker: "选择预设",
    presetTitle: "快速填入名称与地址",
    wizNamePlaceholder: "点右侧 ▾ 选预设，或直接输入名称",
    wizBasePlaceholder: "https://api.z.ai/api/anthropic",
    wizPresetLabel_glm_cn: "智谱 GLM",
    wizPresetLabel_zai: "Z.AI",
    wizPresetLabel_glm_coding_cn: "智谱 Coding Plan",
    wizPresetLabel_zai_coding: "Z.AI Coding Plan",
    wizPresetLabel_kimi_cn: "Kimi",
    wizPresetLabel_kimi_intl: "Moonshot",
    wizPresetLabel_minimax_cn: "MiniMax（国内）",
    wizPresetLabel_minimax_intl: "MiniMax（海外）",
    wizPresetLabel_xiaomi: "小米 MiMo",
    wizPresetLabel_xiaomi_token_cn: "小米 MiMo Token（中国）",
    wizPresetLabel_xiaomi_token_sgp: "小米 MiMo Token（新加坡）",
    wizPresetLabel_xiaomi_token_ams: "小米 MiMo Token（欧洲）",
    wizPresetLabel_deepseek: "DeepSeek",
    wizPresetLabel_openai: "OpenAI",
    wizPresetLabel_anthropic: "Anthropic",
    wizPresetLabel_xai: "xAI Grok",
    wizPresetLabel_groq: "Groq",
    wizPresetLabel_gemini: "Google Gemini",
    wizPresetLabel_together: "Together AI",
    wizPresetLabel_fireworks: "Fireworks",
    wizPresetLabel_openrouter: "OpenRouter",
    wizPresetLabel_siliconflow: "硅基流动",
    wizPresetLabel_dashscope_cn: "通义千问（国内）",
    wizPresetLabel_dashscope_intl: "通义千问（国际）",
    wizPresetLabel_doubao: "豆包 / 火山方舟",
    wizPresetLabel_doubao_coding: "豆包 Coding Plan",
    provider: "Provider",
    baseUrl: "Base_URL",
    apiKey: "API Key",
    create: "创建",
    cancel: "取消",
    save: "保存",
    edit: "编辑",
    delete: "删除",
    confirmDeleteShort: "确认删除？",
    openFolder: "打开文件夹",
    models: "启用模型",
    ports: "端口管理",
    runtimeStatus: "运行状态",
    runStatusOff: "未运行",
    runStatusProxy: "代理运行中",
    runStatusScience: "Science 运行中",
    runStatusBoth: "代理+Science 运行中",
    runStatusStarting: "启动中",
    runStatusStopping: "停止中",
    proxyPort: "代理",
    sandboxPort: "沙箱",
    startScience: "启动 Claude Science",
    stop: "停止",
    stopTitle: "停止本地代理与 Science 沙箱",
    editCspJson: "Edit CSP.json",
    activeBadge: "当前生效",
    mcpBuiltinBadge: "内置",
    mcpBuiltinHint: "CSP 内置联网搜索（无需 API Key）。通用公共方法名：csp_web_search（auto → 可选密钥引擎 → DuckDuckGo Instant Answer → DuckDuckGo Lite → 维基百科）；学术：search_literature → 维基百科/Crossref/arXiv/PubMed。CSP 启动时会预授权检索域名。Brave/Serper/Tavily 密钥可选，用于提高可靠性，不是必需。切勿调用 Anthropic 原生 web_search。",
    skillBuiltinHint: "CSP 内置环境手册 Skill（csp-environment）：常驻指引本地与托管 Claude 的差异——双车道联网搜索、search_skills 必须带 query/prefix、禁止 /mnt/data、save_artifacts、CJK 字体、Skills 采纳与 network-allowlist。可像其他 Skill 一样停用或删除（会被记住）。",
    emptyTitle: "还没有模型配置",
    emptyHint: "点右上「+ 新建」添加一条连接",
    noUrl: "（未填地址）",
    keyLabel: "Key",
    keySep: "：",
    keyMissing: "未填 key",
    fillProvider: "请填写 Provider。",
    fillBaseUrl: "请填写 Base_URL。",
    fillApiKey: "请填写 API Key。",
    skipActivate: "校验没过，仍要启用这条",
    menuMore: "更多",
    descExpand: "更多",
    descCollapse: "收起",
    createSkill: "+ 新建",
    addMcp: "+ 新建",
    scanImport: "扫描导入",
    adoptFromScience: "从 Science 采纳",
    skillsManageTitle: "Skills 管理",
    skillApplyHint: "启用/停用的改动会在下次点击「启动 Claude Science」时生效；若沙箱正在运行，会自动重启以应用。",
    skillEmptyTitle: "还没有 Skill。",
    skillEmptyHint: "点「新建」从零创建，或在「⋯」里选「扫描导入」从本机常见 Skill 目录勾选导入。",
    skillDiscoverTitle: "扫描本地 Skill",
    skillDiscoverHintHtml: "扫描 <code>~/.agents/skills</code>、<code>~/.codex/skills</code>、<code>~/.claude/skills</code>、<code>~/.cursor/skills</code>、<code>~/.cursor/skills-cursor</code>，以及国产 <code>~/.trae/skills</code> / <code>~/.trae-cn/skills</code>（字节）、<code>~/.codebuddy/skills</code>（腾讯），勾选后复制到 <code>~/.csp/skills/</code>。沙箱自带的科学 Skill 不在此列；Science 工作区草稿请用「从 Science 采纳」。",
    skillDiscoverEmpty: "没有扫描到可导入的 Skill。",
    skillDiscoverImport: "导入所选",
    skillPathAdvSummary: "手动路径导入（高级）",
    skillPathLabel: "源目录路径",
    skillPathHintHtml: "含 <code>SKILL.md</code> 的目录；检查通过后复制到 <code>~/.csp/skills/</code>。",
    skillInspPreviewTitle: "检查预览",
    skillInspect: "检查",
    skillImportPath: "导入",
    skillAlreadyImported: "已导入",
    skillScanning: "扫描中…",
    skillAdoptTitle: "从 Science 工作区采纳",
    skillAdoptHintHtml: "扫描沙盒内 <code>workspaces/</code> 下的 Skill 草稿（<code>*.skill.md</code>、<code>*_SKILL.md</code> 或含 <code>SKILL.md</code> 的目录），连同 <code>kernel.py</code> 等伴随文件复制到 <code>~/.csp/skills/</code>。Science 无法直接发布技能时请用此功能。",
    skillAdoptEmpty: "没有扫描到可采纳的 Skill 草稿。",
    skillAdoptConfirm: "采纳所选",
    skillAdoptedBadge: "已采纳",
    mcpDiscoverBtn: "扫描导入",
    modelHintNative: "由 Science 选择器 + 内置映射自动选择（opus 深度 / haiku 快速）。",
    modelHintFixed: "勾选要在 Science 中启用的模型；列表第一个用于后台任务兜底。",
    metaManyModels: "{n} 个模型已启用",
    metaOneModel: "1 个模型已启用",
    metaBuiltinMap: "内置映射",
    metaNoModel: "未选模型",
    loadConfigFail: "读取配置失败：{err}",
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
    noActiveProfile: "还没有「当前生效」的配置。请先「+ 新建」或点击一条配置切换，再点「启动 Claude Science」。",
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
  },
  intl: {
    myConfigs: "Profiles",
    newBtn: "+ New",
    presetPicker: "Choose preset",
    presetTitle: "Fill name and Base URL",
    wizNamePlaceholder: "Pick ▾ for a preset, or type a name",
    wizBasePlaceholder: "https://api.z.ai/api/anthropic",
    wizPresetLabel_glm_cn: "GLM",
    wizPresetLabel_zai: "Z.AI",
    wizPresetLabel_glm_coding_cn: "GLM Coding Plan",
    wizPresetLabel_zai_coding: "Z.AI Coding Plan",
    wizPresetLabel_kimi_cn: "Kimi",
    wizPresetLabel_kimi_intl: "Moonshot",
    wizPresetLabel_minimax_cn: "MiniMax (China)",
    wizPresetLabel_minimax_intl: "MiniMax (Overseas)",
    wizPresetLabel_xiaomi: "MiMo",
    wizPresetLabel_xiaomi_token_cn: "MiMo Token (China)",
    wizPresetLabel_xiaomi_token_sgp: "MiMo Token (Singapore)",
    wizPresetLabel_xiaomi_token_ams: "MiMo Token (Europe)",
    wizPresetLabel_deepseek: "DeepSeek",
    wizPresetLabel_openai: "OpenAI",
    wizPresetLabel_anthropic: "Anthropic",
    wizPresetLabel_xai: "xAI Grok",
    wizPresetLabel_groq: "Groq",
    wizPresetLabel_gemini: "Google Gemini",
    wizPresetLabel_together: "Together AI",
    wizPresetLabel_fireworks: "Fireworks",
    wizPresetLabel_openrouter: "OpenRouter",
    wizPresetLabel_siliconflow: "SiliconFlow",
    wizPresetLabel_dashscope_cn: "Qwen / DashScope (China)",
    wizPresetLabel_dashscope_intl: "Qwen / DashScope (Intl)",
    wizPresetLabel_doubao: "Doubao / Volcengine Ark",
    wizPresetLabel_doubao_coding: "Doubao Coding Plan",
    provider: "Provider",
    baseUrl: "Base URL",
    apiKey: "API Key",
    create: "Create",
    cancel: "Cancel",
    save: "Save",
    edit: "Edit",
    delete: "Delete",
    confirmDeleteShort: "Confirm delete?",
    openFolder: "Open folder",
    models: "Enabled models",
    ports: "Ports",
    runtimeStatus: "Runtime",
    runStatusOff: "Not running",
    runStatusProxy: "Proxy running",
    runStatusScience: "Science running",
    runStatusBoth: "Proxy + Science running",
    runStatusStarting: "Starting…",
    runStatusStopping: "Stopping…",
    proxyPort: "Proxy",
    sandboxPort: "Sandbox",
    startScience: "Start Claude Science",
    stop: "Stop",
    stopTitle: "Stop local proxy and Science sandbox",
    editCspJson: "Edit CSP.json",
    activeBadge: "Active",
    mcpBuiltinBadge: "Built-in",
    mcpBuiltinHint: "CSP's bundled web search (no API key required). GENERAL public method: csp_web_search — auto → optional keyed engines → DuckDuckGo Instant Answer → DuckDuckGo Lite → Wikipedia. LITERATURE: search_literature → Wikipedia/Crossref/arXiv/PubMed. CSP pre-grants search hosts on Start. Brave/Serper/Tavily keys are optional reliability upgrades, not required. Never call the native Anthropic web_search tool.",
    skillBuiltinHint: "CSP's built-in environment handbook Skill (csp-environment): standing guidance for the local sandbox vs hosted Claude — dual-lane web search, search_skills must pass query/prefix, no /mnt/data, save_artifacts, CJK fonts, skill adopt, and network-allowlist. You can disable or remove it like any Skill (the choice is remembered).",
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
    descExpand: "More",
    descCollapse: "Less",
    createSkill: "+ New",
    addMcp: "+ New",
    scanImport: "Scan & import",
    adoptFromScience: "Adopt from Science",
    skillsManageTitle: "Skills",
    skillApplyHint: "Enable/disable takes effect the next time you Start Claude Science; a running sandbox restarts automatically.",
    skillEmptyTitle: "No Skills yet.",
    skillEmptyHint: "Tap New to author one, or use ⋯ → Scan & import to pick from common local Skill folders.",
    skillDiscoverTitle: "Scan local Skills",
    skillDiscoverHintHtml: "Scans <code>~/.agents/skills</code>, <code>~/.codex/skills</code>, <code>~/.claude/skills</code>, <code>~/.cursor/skills</code>, <code>~/.cursor/skills-cursor</code>, plus domestic <code>~/.trae/skills</code> / <code>~/.trae-cn/skills</code> (ByteDance) and <code>~/.codebuddy/skills</code> (Tencent). Checked items copy into <code>~/.csp/skills/</code>. Science-bundled sandbox Skills are excluded; use Adopt from Science for workspace drafts.",
    skillDiscoverEmpty: "No importable Skills found.",
    skillDiscoverImport: "Import selected",
    skillPathAdvSummary: "Manual path import (advanced)",
    skillPathLabel: "Source directory",
    skillPathHintHtml: "A folder containing <code>SKILL.md</code>; after inspect passes, it is copied to <code>~/.csp/skills/</code>.",
    skillInspPreviewTitle: "Inspect preview",
    skillInspect: "Inspect",
    skillImportPath: "Import",
    skillAlreadyImported: "Imported",
    skillScanning: "Scanning…",
    skillAdoptTitle: "Adopt from Science workspace",
    skillAdoptHintHtml: "Scans sandbox <code>workspaces/</code> for Skill drafts (<code>*.skill.md</code>, <code>*_SKILL.md</code>, or folders with <code>SKILL.md</code>) and copies them with companion files like <code>kernel.py</code> into <code>~/.csp/skills/</code>. Use this when Science cannot publish skills directly.",
    skillAdoptEmpty: "No adoptable Skill drafts found.",
    skillAdoptConfirm: "Adopt selected",
    skillAdoptedBadge: "Adopted",
    mcpDiscoverBtn: "Scan & import",
    modelHintNative: "Auto-mapped via Science picker + built-in routing (opus for depth, haiku for speed).",
    modelHintFixed: "Check models to enable in Science; the first is the fallback for background tasks.",
    metaManyModels: "{n} models enabled",
    metaOneModel: "1 model enabled",
    metaBuiltinMap: "built-in mapping",
    metaNoModel: "no model selected",
    loadConfigFail: "Failed to load config: {err}",
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

/**
 * Unified provider presets for both UI languages.
 * Edition (cn/intl) only affects i18n copy — endpoint choice is regional, not
 * language: Chinese users often buy overseas Z.AI plans, and vice versa.
 * Labels carry region hints; profile `name` stays short for the card list.
 */
const WIZ_PRESETS = [
  { id: "deepseek", templateId: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com/anthropic", lockUrl: true },
  { id: "openai", templateId: "custom-openai", name: "OpenAI", baseUrl: "https://api.openai.com/v1" },
  { id: "anthropic", templateId: "custom", name: "Anthropic", baseUrl: "https://api.anthropic.com" },
  { id: "xai", templateId: "custom-openai", name: "xAI Grok", baseUrl: "https://api.x.ai/v1" },
  { id: "groq", templateId: "custom-openai", name: "Groq", baseUrl: "https://api.groq.com/openai/v1" },
  { id: "gemini", templateId: "custom-openai", name: "Gemini", baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { id: "together", templateId: "custom-openai", name: "Together", baseUrl: "https://api.together.ai/v1" },
  { id: "fireworks", templateId: "custom-openai", name: "Fireworks", baseUrl: "https://api.fireworks.ai/inference/v1" },
  { id: "openrouter", templateId: "openrouter", name: "OpenRouter", baseUrl: "https://openrouter.ai/api" },
  { id: "glm-cn", templateId: "glm", name: "GLM", baseUrl: "https://open.bigmodel.cn/api/anthropic" },
  { id: "zai", templateId: "glm", name: "ZAI", baseUrl: "https://api.z.ai/api/anthropic" },
  { id: "glm-coding-cn", templateId: "custom-openai", name: "GLM Coding Plan", baseUrl: "https://open.bigmodel.cn/api/coding/paas/v4" },
  { id: "zai-coding", templateId: "custom-openai", name: "ZAI Coding Plan", baseUrl: "https://api.z.ai/api/coding/paas/v4" },
  { id: "siliconflow", templateId: "custom-openai", name: "SiliconFlow", baseUrl: "https://api.siliconflow.cn/v1" },
  { id: "dashscope-cn", templateId: "custom-openai", name: "DashScope", baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
  { id: "dashscope-intl", templateId: "custom-openai", name: "DashScope", baseUrl: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1" },
  { id: "doubao", templateId: "custom-openai", name: "Doubao", baseUrl: "https://ark.cn-beijing.volces.com/api/v3" },
  { id: "doubao-coding", templateId: "custom-openai", name: "Doubao Coding Plan", baseUrl: "https://ark.cn-beijing.volces.com/api/coding/v3" },
  { id: "kimi-cn", templateId: "kimi", name: "Kimi", baseUrl: "https://api.moonshot.cn/anthropic" },
  { id: "kimi-intl", templateId: "kimi", name: "Moonshot", baseUrl: "https://api.moonshot.ai/anthropic" },
  { id: "minimax-cn", templateId: "minimax", name: "MiniMax", baseUrl: "https://api.minimaxi.com/anthropic" },
  { id: "minimax-intl", templateId: "minimax", name: "MiniMax", baseUrl: "https://api.minimax.io/anthropic" },
  { id: "xiaomi", templateId: "xiaomi", name: "MiMo", baseUrl: "https://api.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token-cn", templateId: "xiaomi", name: "MiMo Token", baseUrl: "https://token-plan-cn.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token-sgp", templateId: "xiaomi", name: "MiMo Token", baseUrl: "https://token-plan-sgp.xiaomimimo.com/anthropic" },
  { id: "xiaomi-token-ams", templateId: "xiaomi", name: "MiMo Token", baseUrl: "https://token-plan-ams.xiaomimimo.com/anthropic" },
];

/** Currently selected preset id (menu highlight); empty when typing a custom name. */
let wizSelectedPresetId = "";

function wizPresets() {
  return WIZ_PRESETS;
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
  if (els.i18nRuntimeStatus) els.i18nRuntimeStatus.textContent = t.runtimeStatus;
  if (els.i18nProxyPort) els.i18nProxyPort.textContent = t.proxyPort;
  if (els.i18nSandboxPort) els.i18nSandboxPort.textContent = t.sandboxPort;
  if (els.oneClickBtn) els.oneClickBtn.textContent = t.startScience;
  if (els.stopBtn) { els.stopBtn.textContent = t.stop; els.stopBtn.title = t.stopTitle; }
  updateRuntimeStatusUI();
  if (els.editCspJsonBtn) els.editCspJsonBtn.textContent = t.editCspJson;
  if (els.skipActivateBtn) els.skipActivateBtn.textContent = t.skipActivate;
  if (els.listhdMoreBtn) els.listhdMoreBtn.title = t.menuMore;
  if (els.skillCreateBtn) els.skillCreateBtn.textContent = t.createSkill;
  if (els.mcpAddBtn) els.mcpAddBtn.textContent = t.addMcp;
  if (els.skillDiscoverBtn) els.skillDiscoverBtn.textContent = t.scanImport;
  if (els.skillAdoptBtn) els.skillAdoptBtn.textContent = t.adoptFromScience;
  if (els.mcpDiscoverBtn) els.mcpDiscoverBtn.textContent = t.mcpDiscoverBtn;
  if (els.skillMoreBtn) els.skillMoreBtn.title = t.menuMore;
  if (els.mcpMoreBtn) els.mcpMoreBtn.title = t.menuMore;
  const skillsTitle = document.querySelector("#skillListSec > .skill-hd > .sec-title");
  if (skillsTitle) skillsTitle.textContent = t.skillsManageTitle;
  if (els.skillApplyHint) els.skillApplyHint.textContent = t.skillApplyHint;
  if (els.skillEmptyTitle) els.skillEmptyTitle.textContent = t.skillEmptyTitle;
  if (els.skillEmptyHint) els.skillEmptyHint.textContent = t.skillEmptyHint;
  if (els.skillDiscoverTitle) els.skillDiscoverTitle.textContent = t.skillDiscoverTitle;
  if (els.skillDiscoverHint) els.skillDiscoverHint.innerHTML = t.skillDiscoverHintHtml;
  if (els.skillDiscoverEmptyText) els.skillDiscoverEmptyText.textContent = t.skillDiscoverEmpty;
  if (els.skillDiscoverImportBtn) els.skillDiscoverImportBtn.textContent = t.skillDiscoverImport;
  if (els.skillDiscoverCancelBtn) els.skillDiscoverCancelBtn.textContent = t.cancel;
  if (els.skillPathAdvSummary) els.skillPathAdvSummary.textContent = t.skillPathAdvSummary;
  if (els.skillPathLabel) els.skillPathLabel.textContent = t.skillPathLabel;
  if (els.skillPathHint) els.skillPathHint.innerHTML = t.skillPathHintHtml;
  if (els.skillInspPreviewTitle) els.skillInspPreviewTitle.textContent = t.skillInspPreviewTitle;
  if (els.skillInspectBtn) els.skillInspectBtn.textContent = t.skillInspect;
  if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.textContent = t.skillImportPath;
  if (els.skillAdoptTitle) els.skillAdoptTitle.textContent = t.skillAdoptTitle;
  if (els.skillAdoptHint) els.skillAdoptHint.innerHTML = t.skillAdoptHintHtml;
  if (els.skillAdoptEmptyText) els.skillAdoptEmptyText.textContent = t.skillAdoptEmpty;
  if (els.skillAdoptConfirmBtn) els.skillAdoptConfirmBtn.textContent = t.skillAdoptConfirm;
  if (els.skillAdoptCancelBtn) els.skillAdoptCancelBtn.textContent = t.cancel;
  populateWizPresetMenu();
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
  mcpServers: [
    { id: "mcp_0000000000000000", name: "web-search", description: "Local web + literature search (no API key required). GENERAL csp_web_search → duckduckgo_ia/lite (wikipedia NOT on GENERAL); LITERATURE search_literature → wikipedia/Crossref/arXiv/PubMed. Optional BRAVE_SEARCH_API_KEY / SERPER_API_KEY / TAVILY_API_KEY improve quality only. Never call native Anthropic web_search.", command: "python3", args: ["/Users/me/.csp/sandbox/home/.claude-science/mcp/csp-web-search-server.py"], env: { BRAVE_SEARCH_API_KEY: "", SERPER_API_KEY: "", TAVILY_API_KEY: "" }, enabled: true, builtin: true, createdAt: "2026-07-12T02:30:00Z", updatedAt: "2026-07-12T02:30:00Z" },
    { id: "mcp_0000000000000001", name: "local-fs", description: "本地文件系统工具", command: "python3", args: ["/Users/me/mcp/fs_server.py"], env: { API_TOKEN: "••••1234" }, enabled: true, builtin: false, createdAt: "2026-07-12T02:30:00Z", updatedAt: "2026-07-12T02:30:00Z" }
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
    case "stop_all":
      return Promise.resolve(null);
    case "get_runtime_status":
      return Promise.resolve({ proxy: "amber", sandbox: "amber", upstream: "amber" });
    case "open_csp_json":
      return Promise.resolve("~/.csp/CSP.json");
    case "open_mcp_inventory_json":
      return Promise.resolve("~/.csp/mcp/inventory.json");
    case "open_network_allowlist_json":
      return Promise.resolve("~/.csp/network-allowlist.json");
    case "list_skills":
      return Promise.resolve(mockStore.skills.map((s) => ({ ...s })));
    case "discover_skills":
      return Promise.resolve([
        { name: "crypto-data", description: "获取加密货币实时数据", sourcePath: "/Users/me/.agents/skills/crypto-data", sourceLabel: "~/.agents/skills", alreadyImported: false },
        { name: "pdf", description: "读写 PDF", sourcePath: "/Users/me/.codex/skills/pdf", sourceLabel: "~/.codex/skills", alreadyImported: false },
        { name: "playwright", description: "浏览器自动化", sourcePath: "/Users/me/.codex/skills/playwright", sourceLabel: "~/.codex/skills", alreadyImported: true },
      ]);
    case "discover_workspace_skills":
      return Promise.resolve([
        {
          key: "workspace://org/ws1/file:crypto-data-v2_SKILL.md",
          name: "crypto-data-v2",
          description: "Enhanced crypto skill draft",
          workspaceId: "ws1",
          files: ["crypto-data-v2_SKILL.md → SKILL.md", "kernel.py"],
          warnings: [],
          alreadyImported: false,
        },
      ]);
    case "adopt_workspace_skills": {
      const keys = (args.input && args.input.keys) || [];
      const adopted = keys.map((key, i) => ({
        id: "sk_" + Math.random().toString(16).slice(2, 10) + i,
        name: "Adopted Skill",
        description: "From " + key,
        enabled: true,
        sizeBytes: 12000,
        importedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        requirements: ["python"],
      }));
      adopted.forEach((s) => mockStore.skills.push(s));
      return Promise.resolve({ adopted, failures: [], needsRestart: false });
    }
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
    case "create_skill": {
      const content = (args.input && args.input.content) || "";
      // Parse name/description from front-matter, mirroring the backend.
      const nameMatch = content.match(/^name:\s*(.*)$/m);
      const descMatch = content.match(/^description:\s*(.*)$/m);
      const name = (nameMatch ? nameMatch[1] : "").trim();
      const description = (descMatch ? descMatch[1] : "").trim();
      if (!name || name === "Untitled Skill") {
        return Promise.reject("Skill 名称不能为空");
      }
      if (mockStore.skills.some((s) => s.name === name)) {
        return Promise.reject("同名 Skill 已存在：" + name);
      }
      const id = "sk_" + Math.random().toString(16).slice(2, 10);
      const skill = {
        id,
        name,
        description,
        enabled: true,
        sizeBytes: content.length,
        importedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        requirements: [],
      };
      mockStore.skills.push(skill);
      return Promise.resolve({ skill, needsRestart: false });
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
    case "open_skill_file":
    case "open_skill_folder":
      return Promise.resolve("/preview/skills/" + ((args.input && args.input.skillId) || ""));
    case "list_mcp_servers":
      return Promise.resolve(mockStore.mcpServers.map((s) => ({ ...s })));
    case "discover_mcp_servers":
      return Promise.resolve([
        {
          name: "notion-mcp-v1",
          description: "",
          command: "/Users/me/.local/node/bin/notion-mcp",
          args: ["--transport", "stdio"],
          envKeys: ["NOTION_TOKEN"],
          sourceLabel: "Cursor",
          sourcePath: "/Users/me/.cursor/mcp.json",
          alreadyImported: false,
        },
        {
          name: "fs",
          description: "",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "/data"],
          envKeys: [],
          sourceLabel: "Codex",
          sourcePath: "/Users/me/.codex/config.toml",
          alreadyImported: false,
        },
      ]);
    case "import_discovered_mcp_server": {
      const found = {
        name: args.input.name,
        description: "",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "/data"],
        env: {},
      };
      const id = "mcp_" + Math.random().toString(16).slice(2, 18).padEnd(16, "0");
      const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
      const srv = { id, ...found, enabled: true, createdAt: now, updatedAt: now };
      mockStore.mcpServers.push(srv);
      return Promise.resolve({ ...srv });
    }
    case "inspect_mcp_server": {
      const inp = args.input || {};
      const errors = [];
      if (!inp.name) errors.push("Name is required");
      if (!inp.command) errors.push("Command is required");
      const known = ["node", "npm", "npx", "python", "python3", "pip", "pip3", "uv", "uvx", "deno", "bun", "bunx"];
      const commandOk = !!inp.command && (inp.command.startsWith("/") || known.includes(inp.command));
      const warnings = [];
      if (inp.command && !commandOk) warnings.push(`'${inp.command}' 不在受管运行时白名单，Science 可能拒绝。`);
      const exts = [".py", ".js", ".mjs", ".cjs", ".ts", ".rb", ".sh", ".jar", ".json"];
      (inp.args || []).forEach((a) => {
        const t = String(a).trim();
        if (t && !t.startsWith("/") && !t.startsWith("-") && (t.includes("/") || exts.some((e) => t.endsWith(e)))) {
          warnings.push(`'${t}' 看起来是相对路径；沙箱无工作目录且仅授权绝对路径，请用绝对路径。`);
        }
      });
      return Promise.resolve({ valid: errors.length === 0, commandOk, warnings, errors });
    }
    case "create_mcp_server": {
      const inp = args.input || {};
      const id = "mcp_" + Math.random().toString(16).slice(2, 18).padEnd(16, "0");
      const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
      const env = {};
      Object.entries(inp.env || {}).forEach(([k, v]) => (env[k] = mockMask(v)));
      const srv = { id, name: inp.name, description: inp.description || "", command: inp.command, args: inp.args || [], env, enabled: true, createdAt: now, updatedAt: now };
      mockStore.mcpServers.push(srv);
      return Promise.resolve(srv);
    }
    case "update_mcp_server": {
      const s = mockStore.mcpServers.find((x) => x.id === args.input.serverId);
      if (!s) return Promise.reject("MCP server not found");
      const inp = args.input.server || {};
      s.name = inp.name; s.description = inp.description || ""; s.command = inp.command; s.args = inp.args || [];
      // Mirror backend env-merge: empty value keeps old, absent key deletes.
      const env = {};
      Object.entries(inp.env || {}).forEach(([k, v]) => {
        if (v === "") { if (s.env && s.env[k] != null) env[k] = s.env[k]; }
        else env[k] = mockMask(v);
      });
      s.env = env; s.updatedAt = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
      return Promise.resolve(s);
    }
    case "set_mcp_server_enabled": {
      const s = mockStore.mcpServers.find((x) => x.id === args.input.serverId);
      if (s) s.enabled = !!args.input.enabled;
      return Promise.resolve(s);
    }
    case "remove_mcp_server":
      mockStore.mcpServers = mockStore.mcpServers.filter((x) => x.id !== args.input.serverId);
      return Promise.resolve(null);
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

function closeAllMenus() {
  resetAllMenuConfirms();
  [els.profileList, els.skillList, els.mcpList].forEach((listEl) => {
    if (!listEl) return;
    listEl.querySelectorAll(".pmenu").forEach((m) => {
      m.hidden = true;
      m.classList.remove("pmenu-up");
    });
    listEl.querySelectorAll(".pmenu-btn").forEach((b) => {
      b.setAttribute("aria-expanded", "false");
    });
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
  closeHeaderMenus();
  closeAllMenus();
  if (!wasOpen) {
    els.listhdMenu.hidden = false;
    els.listhdMoreBtn.setAttribute("aria-expanded", "true");
  }
}

// Generic ⋯ overflow menu (Skills / MCP headers): mirror the Profiles listhd menu.
function closeMenu(menuEl, btnEl) {
  if (menuEl) menuEl.hidden = true;
  if (btnEl) btnEl.setAttribute("aria-expanded", "false");
}

function toggleMenu(menuEl, btnEl) {
  if (!menuEl || !btnEl) return;
  const wasOpen = !menuEl.hidden;
  closeHeaderMenus();
  closeAllMenus();
  if (!wasOpen) {
    menuEl.hidden = false;
    btnEl.setAttribute("aria-expanded", "true");
  }
}

// Close every header overflow menu (Profiles / Skills / MCP) in one call.
function closeHeaderMenus() {
  closeListhdMenu();
  closeMenu(els.skillMenu, els.skillMoreBtn);
  closeMenu(els.mcpMenu, els.mcpMoreBtn);
}

function positionProfileMenu(menu, btn) {
  menu.classList.remove("pmenu-up");
  menu.hidden = false;
  // Resolve the actual scroll container the button lives in. Skill/MCP rows
  // scroll inside `.skill-list`; Profile rows scroll inside `.panel-body`.
  // Using a stale `els.panelBody` (the hidden Profiles pane) yielded a zero-height
  // rect on other tabs, forcing the menu to wrongly flip up and hide under the header.
  const scrollEl =
    btn.closest(".skill-list") ||
    btn.closest(".panel-body") ||
    els.panelBody ||
    els.profileList;
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
    // Skill / MCP manager actions: prevent concurrent mutations racing a running op.
    els.skillCreateBtn, els.skillCreateSaveBtn, els.skillCreateCancelBtn,
    els.skillMoreBtn, els.skillInspectBtn, els.skillImportConfirmBtn,
    els.skillDiscoverBtn, els.skillDiscoverImportBtn, els.skillDiscoverCancelBtn,
    els.skillAdoptBtn, els.skillAdoptConfirmBtn, els.skillAdoptCancelBtn,
    els.mcpMoreBtn, els.mcpJsonBtn, els.mcpNetworkAllowlistBtn, els.mcpDiscoverBtn, els.mcpDiscoverImportBtn, els.mcpDiscoverCancelBtn,
    els.mcpAddBtn, els.mcpSaveBtn, els.mcpCancelBtn,
  ].forEach((b) => b && (b.disabled = on));
  syncProfileBusyState();
  // On busy release, hand model-required save gating back to gates (avoid setBusy(false) overwriting gate).
  if (!on) { refreshWizGate(); refreshConnGate(); }
  updateRuntimeStatusUI();
}

/** Last probed lights from get_runtime_status (`green` | `amber`). */
let lastRuntimeLights = { proxy: "amber", sandbox: "amber" };
/** Transient UI phase while start/stop is in flight. */
let runtimePhase = null; // "starting" | "stopping" | null
let runtimeStatusTimer = null;

function runtimeStatusText() {
  const t = S();
  if (runtimePhase === "starting") return t.runStatusStarting;
  if (runtimePhase === "stopping") return t.runStatusStopping;
  const proxyOn = lastRuntimeLights.proxy === "green";
  const scienceOn = lastRuntimeLights.sandbox === "green";
  if (proxyOn && scienceOn) return t.runStatusBoth;
  if (proxyOn) return t.runStatusProxy;
  if (scienceOn) return t.runStatusScience;
  return t.runStatusOff;
}

function updateRuntimeStatusUI() {
  if (!els.runtimeStatusText) return;
  const text = runtimeStatusText();
  els.runtimeStatusText.textContent = text;
  const proxyOn = lastRuntimeLights.proxy === "green";
  const scienceOn = lastRuntimeLights.sandbox === "green";
  const busyPhase = runtimePhase === "starting" || runtimePhase === "stopping";
  els.runtimeStatusText.classList.toggle("is-busy", busyPhase);
  els.runtimeStatusText.classList.toggle("is-running", !busyPhase && (proxyOn || scienceOn));
}

async function refreshRuntimeStatus() {
  try {
    const st = await call("get_runtime_status");
    lastRuntimeLights = {
      proxy: st && st.proxy === "green" ? "green" : "amber",
      sandbox: st && st.sandbox === "green" ? "green" : "amber",
    };
  } catch (_) {
    // Keep last known lights on probe failure.
  }
  updateRuntimeStatusUI();
}

function startRuntimeStatusPolling() {
  if (runtimeStatusTimer) return;
  runtimeStatusTimer = setInterval(() => {
    if (!busy) refreshRuntimeStatus().catch(() => {});
  }, 4000);
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
  if (els.runtimeStatusSec) els.runtimeStatusSec.hidden = v !== "list";
  els.wizSec.hidden = v !== "wizard";
  els.connSec.hidden = v !== "conn";
  // Only drive the panel form chrome when the profiles pane is active.
  if (!els.panelBody || !els.panelBody.hidden) {
    els.panel.classList.toggle("view-form", v !== "list");
  }
  if (v === "list") {
    hideSkip();
    closeWizPresetMenu();
    // Drop stale errors (e.g. a one-off "fetch models 401") so they don't
    // linger on the profile list after create/edit succeeded with builtins.
    setMsg("");
  }
}
function cancelForm() { showView("list"); }

// Skill tab: list / create / discover / adopt (full-page panels, same chrome as config wizard).
function showSkillView(v) {
  const list = v === "list";
  if (els.skillListSec) els.skillListSec.hidden = !list;
  if (els.skillCreateSec) els.skillCreateSec.hidden = v !== "create";
  if (els.skillDiscoverSec) els.skillDiscoverSec.hidden = v !== "discover";
  if (els.skillAdoptSec) els.skillAdoptSec.hidden = v !== "adopt";
  if (els.skillPane) {
    els.skillPane.classList.toggle("pane-form", !list);
    if (!els.skillPane.hidden) els.panel.classList.toggle("view-form", !list);
  }
  if (list) closeMenu(els.skillMenu, els.skillMoreBtn);
}

// MCP tab: list / form (add|edit) / discover.
function showMcpView(v) {
  const list = v === "list";
  if (els.mcpListSec) els.mcpListSec.hidden = !list;
  if (els.mcpFormSec) els.mcpFormSec.hidden = v !== "form";
  if (els.mcpDiscoverSec) els.mcpDiscoverSec.hidden = v !== "discover";
  if (els.mcpPane) {
    els.mcpPane.classList.toggle("pane-form", !list);
    if (!els.mcpPane.hidden) els.panel.classList.toggle("view-form", !list);
  }
  if (list) closeMenu(els.mcpMenu, els.mcpMoreBtn);
}

function showSkip() { els.skipActivateBtn.hidden = false; }
function hideSkip() { els.skipActivateBtn.hidden = true; pendingSkipActivateId = null; }

// Dangerous ops "click again to confirm" (avoid window.confirm; unreliable in Tauri webview).
// Two-step delete for overflow-menu items: the first click relabels the button
// in place to "确认删除？" and keeps the menu open; the second click within the
// window runs `fn`. Auto-resets after 4s or when any menu closes so a stale
// confirm label never persists.
function menuConfirmDelete(btn, fn) {
  if (!btn) { fn(); return; }
  if (btn.dataset.confirming === "1") {
    resetMenuConfirm(btn);
    closeAllMenus();
    fn();
    return;
  }
  btn.dataset.origLabel = btn.textContent;
  btn.dataset.confirming = "1";
  btn.textContent = S().confirmDeleteShort || "确认删除？";
  btn.classList.add("confirming");
  btn._confirmTimer = setTimeout(() => resetMenuConfirm(btn), 4000);
}

function resetMenuConfirm(btn) {
  if (!btn || btn.dataset.confirming !== "1") return;
  if (btn._confirmTimer) { clearTimeout(btn._confirmTimer); btn._confirmTimer = null; }
  if (btn.dataset.origLabel != null) btn.textContent = btn.dataset.origLabel;
  btn.classList.remove("confirming");
  delete btn.dataset.confirming;
  delete btn.dataset.origLabel;
}

function resetAllMenuConfirms() {
  document.querySelectorAll('.pmenu-item[data-confirming="1"]').forEach(resetMenuConfirm);
}

// ── Load config + render list ──
async function loadConfig(opts = {}) {
  const keepView = !!opts.keepView;
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
    // Focus refresh must not yank the user out of create/edit forms.
    if (!keepView) showView("list");
    await refreshRuntimeStatus();
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
  // Prefer full localized label (e.g. "Z.AI（海外）") before short card names
  // that collide across regions ("Moonshot", "MiniMax", "GLM").
  const byLabel = wizPresets().find((item) => wizPresetLabel(item).toLowerCase() === q);
  if (byLabel) return byLabel;
  return wizPresets().find((item) => item.name.toLowerCase() === q) || null;
}

function applyWizPresetFields(preset) {
  els.wizName.value = preset.name;
  els.wizBase.value = preset.baseUrl;
  els.wizBase.readOnly = !!preset.lockUrl;
  wizLastAutoBase = preset.baseUrl || "";
  wizSelectedPresetId = preset.id;
  if (!preset.baseUrl) els.wizBase.focus();
}

function applyWizNameAutofill() {
  const name = els.wizName.value.trim();
  const preset = wizPresetByName(name);
  if (!preset) {
    wizSelectedPresetId = "";
    if (els.wizBase.readOnly) els.wizBase.readOnly = false;
    refreshWizGate();
    return;
  }
  wizSelectedPresetId = preset.id;
  const currentBase = els.wizBase.value.trim();
  const canAutoFill = !currentBase || currentBase === wizLastAutoBase;
  if (canAutoFill && preset.baseUrl) {
    els.wizBase.value = preset.baseUrl;
    wizLastAutoBase = preset.baseUrl;
    els.wizBase.readOnly = !!preset.lockUrl;
  }
  refreshWizGate();
}

function closeWizPresetMenu() {
  if (!els.wizPresetMenu || !els.wizPresetBtn) return;
  els.wizPresetMenu.hidden = true;
  els.wizPresetBtn.setAttribute("aria-expanded", "false");
}

function populateWizPresetMenu() {
  const menu = els.wizPresetMenu;
  if (!menu) return;
  const t = S();
  if (els.wizPresetBtn) els.wizPresetBtn.title = t.presetTitle;
  menu.innerHTML = "";
  for (const item of wizPresets()) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "pmenu-item";
    btn.setAttribute("role", "option");
    btn.dataset.presetId = item.id;
    btn.textContent = wizPresetLabel(item);
    if (item.id === wizSelectedPresetId) btn.setAttribute("aria-selected", "true");
    menu.appendChild(btn);
  }
}

function toggleWizPresetMenu() {
  if (!els.wizPresetMenu || !els.wizPresetBtn) return;
  const open = els.wizPresetMenu.hidden;
  closeAllMenus();
  closeHeaderMenus();
  if (open) {
    populateWizPresetMenu();
    els.wizPresetMenu.hidden = false;
    els.wizPresetBtn.setAttribute("aria-expanded", "true");
  } else {
    closeWizPresetMenu();
  }
}

function applyWizPresetFromMenu(presetId) {
  const preset = wizPresetById(presetId);
  closeWizPresetMenu();
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
  // OpenAI-compat roots: /v1, Coding Plan (/paas /coding/), DashScope, Volcengine Ark /api/v3
  if (
    norm.includes("/v1") ||
    norm.includes("/paas/") ||
    norm.includes("compatible-mode") ||
    norm.includes("/coding/") ||
    norm.includes("/api/v3")
  ) {
    return "custom-openai";
  }
  return "custom";
}

function openWizard() {
  hideSkip();
  wizLastAutoBase = "";
  els.wizName.value = "";
  wizSelectedPresetId = "";
  closeWizPresetMenu();
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
  // Prefer the selected preset's template when the URL is still the preset default
  // (covers hosts like volces.com/api/v3 that URL heuristics alone can miss).
  const selected = wizSelectedPresetId ? wizPresetById(wizSelectedPresetId) : null;
  const templateId =
    selected && selected.templateId && normBaseUrl(base) === normBaseUrl(selected.baseUrl)
      ? selected.templateId
      : inferTemplateId(base);
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
    setMsg("");
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
  setMsg("");
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
    setMsg("");
    await loadConfig();
  } catch (e) {
    setMsg(T("saveConnFail", { err: resolveBackendErr(e) }));
  } finally {
    setBusy(false);
  }
}

async function doDelete(id) {
  setBusy(true);
  try {
    await call("delete_profile", { id });
    setMsg("");
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
      setMsg("");
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
  runtimePhase = "starting";
  setBusy(true, { kind: "oneClick" });
  try {
    await call("one_click_login");
  } catch (e) {
    setMsg(T("oneClickFail", { err: resolveBackendErr(e) }));
  } finally {
    runtimePhase = null;
    setBusy(false);
    await refreshRuntimeStatus();
  }
}

async function stopAll() {
  runtimePhase = "stopping";
  setBusy(true, { kind: "stop" });
  try {
    await call("stop_all");
  } catch (e) {
    setMsg(T("stopFail", { err: resolveBackendErr(e) }));
  } finally {
    runtimePhase = null;
    setBusy(false);
    await refreshRuntimeStatus();
  }
}

function wire() {
  [
    "oneClickBtn", "stopBtn",
    "msg", "proxyPort", "sandboxPort", "advSec", "runtimeStatusSec", "runtimeStatusText", "i18nRuntimeStatus",
    "listSec", "profileList", "newBtn", "listhdMoreBtn", "listhdMenu", "editCspJsonBtn", "skipActivateBtn",
    "i18nMyConfigs", "i18nLabelProvider", "i18nLabelBase", "i18nLabelKey",
    "i18nConnName", "i18nConnBase", "i18nConnKey", "i18nPorts", "i18nProxyPort", "i18nSandboxPort",
    "wizSec", "wizName", "wizPresetBtn", "wizPresetMenu", "wizBase", "wizKey", "wizSaveBtn", "wizCancelBtn",
    "connSec", "connTitle", "connName", "connBase", "connBaseHint",
    "connModelInfo", "connModelHint", "connModelPick", "connKey", "connSaveBtn", "connCancelBtn",
    "tabProfiles", "tabSkills", "skillPane", "skillListSec",
    "skillCreateBtn", "skillCreateSec", "skillCreateName", "skillCreateDesc",
    "skillCreateBody", "skillCreateInspection", "skillCreateErrors",
    "skillCreateSaveBtn", "skillCreateCancelBtn",
    "skillMoreBtn", "skillMenu", "skillEmpty", "skillEmptyTitle", "skillEmptyHint",
    "skillApplyHint", "skillList", "skillMsg",
    "skillDiscoverBtn", "skillDiscoverSec", "skillDiscoverTitle", "skillDiscoverHint",
    "skillDiscoverList", "skillDiscoverEmpty", "skillDiscoverEmptyText",
    "skillDiscoverImportBtn", "skillDiscoverCancelBtn",
    "skillPathAdv", "skillPathAdvSummary", "skillPathLabel", "skillPathHint",
    "skillSourcePath", "skillInspectionPreview", "skillInspPreviewTitle",
    "inspName", "inspDesc", "inspStats", "inspReqs", "inspWarnings", "inspErrors",
    "skillInspectBtn", "skillImportConfirmBtn",
    "skillAdoptBtn", "skillAdoptSec", "skillAdoptTitle", "skillAdoptHint",
    "skillAdoptList", "skillAdoptEmpty", "skillAdoptEmptyText",
    "skillAdoptConfirmBtn", "skillAdoptCancelBtn",
    "tabMcp", "mcpPane", "mcpListSec", "mcpAddBtn", "mcpMoreBtn", "mcpMenu", "mcpEmpty", "mcpList", "mcpMsg",
    "mcpJsonBtn", "mcpNetworkAllowlistBtn", "mcpDiscoverBtn", "mcpDiscoverSec", "mcpDiscoverList",
    "mcpDiscoverEmpty", "mcpDiscoverImportBtn", "mcpDiscoverCancelBtn",
    "mcpFormSec", "mcpModalTitle", "mcpName", "mcpDesc", "mcpCommand", "mcpArgs", "mcpEnv",
    "mcpInspection", "mcpWarnings", "mcpErrors", "mcpSaveBtn", "mcpCancelBtn",
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
      if (act === "delete") { menuConfirmDelete(btn, () => doDelete(id)); return; }
      closeAllMenus();
      if (act === "editconn") openConn(id);
      return;
    }
    if (row && !e.target.closest(".pmenu-wrap")) {
      const id = row.getAttribute("data-id");
      if (id && id !== configState.active_id) activate(id, false);
    }
  });
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".pmenu-wrap")) closeAllMenus();
    if (!e.target.closest(".listhd-more")) closeHeaderMenus();
    if (!e.target.closest(".provider-combo")) closeWizPresetMenu();
  });

  els.newBtn.addEventListener("click", openWizard);
  els.listhdMoreBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    if (busy) return;
    toggleListhdMenu();
  });
  els.skillMoreBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    if (busy) return;
    toggleMenu(els.skillMenu, els.skillMoreBtn);
  });
  els.mcpMoreBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    if (busy) return;
    toggleMenu(els.mcpMenu, els.mcpMoreBtn);
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

  els.wizPresetBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleWizPresetMenu();
  });
  els.wizPresetMenu.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-preset-id]");
    if (!btn) return;
    applyWizPresetFromMenu(btn.dataset.presetId);
  });
  els.wizName.addEventListener("input", applyWizNameAutofill);
  els.wizName.addEventListener("focus", closeWizPresetMenu);
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
  els.tabMcp.addEventListener("click", () => switchTab("mcp"));

  // MCP panel actions
  els.mcpJsonBtn.addEventListener("click", async () => {
    closeMenu(els.mcpMenu, els.mcpMoreBtn);
    if (busy) return;
    try {
      await call("open_mcp_inventory_json");
    } catch (e) {
      setMcpMsg(resolveBackendErr(e));
    }
  });
  if (els.mcpNetworkAllowlistBtn) {
    els.mcpNetworkAllowlistBtn.addEventListener("click", async () => {
      closeMenu(els.mcpMenu, els.mcpMoreBtn);
      if (busy) return;
      try {
        await call("open_network_allowlist_json");
      } catch (e) {
        setMcpMsg(resolveBackendErr(e));
      }
    });
  }
  els.mcpDiscoverBtn.addEventListener("click", openMcpDiscover);
  els.mcpDiscoverCancelBtn.addEventListener("click", closeMcpDiscover);
  els.mcpDiscoverImportBtn.addEventListener("click", importDiscoveredMcpServers);
  els.mcpDiscoverList.addEventListener("change", (e) => {
    if (e.target && e.target.type === "checkbox") refreshMcpDiscoverGate();
  });
  els.mcpAddBtn.addEventListener("click", () => openMcpModal());
  els.mcpSaveBtn.addEventListener("click", saveMcpServer);
  els.mcpCancelBtn.addEventListener("click", closeMcpModal);
  // Editing any field invalidates a prior warning acknowledgement.
  [els.mcpName, els.mcpDesc, els.mcpCommand, els.mcpArgs, els.mcpEnv].forEach((el) =>
    el.addEventListener("input", () => { mcpWarnAck = false; })
  );
  els.mcpList.addEventListener("click", (e) => {
    if (busy) return;
    if (handleDescToggleClick(e)) return;
    const row = e.target.closest(".skill-row[data-id]");
    if (!row) return;
    const id = row.getAttribute("data-id");
    const name = row.getAttribute("data-name");
    const btn = e.target.closest("[data-act]");
    if (btn) {
      const act = btn.getAttribute("data-act");
      if (act === "menu") {
        const wrap = btn.closest(".pmenu-wrap");
        const menu = wrap && wrap.querySelector(".pmenu");
        if (!menu) return;
        const wasOpen = !menu.hidden;
        closeHeaderMenus();
        closeAllMenus();
        if (!wasOpen) {
          positionProfileMenu(menu, btn);
          btn.setAttribute("aria-expanded", "true");
        }
        return;
      }
      if (act === "delete") { menuConfirmDelete(btn, () => doRemoveMcpServer(id)); return; }
      closeAllMenus();
      if (act === "edit") openMcpModal(id);
    } else if (e.target.type === "checkbox") {
      toggleMcpServer(id, e.target.checked);
    }
  });

  // Skills panel actions
  els.skillCreateBtn.addEventListener("click", openSkillCreate);
  els.skillCreateSaveBtn.addEventListener("click", saveNewSkill);
  els.skillCreateCancelBtn.addEventListener("click", closeSkillCreate);
  // Keep the SKILL.md front-matter in sync with the name/description fields
  // until the body is edited by hand (then the body wins — see openSkillCreate).
  els.skillCreateName.addEventListener("input", syncSkillCreateBody);
  els.skillCreateDesc.addEventListener("input", syncSkillCreateBody);
  els.skillCreateBody.addEventListener("input", () => { skillBodyDirty = true; });
  els.skillInspectBtn.addEventListener("click", inspectSkillSource);
  els.skillImportConfirmBtn.addEventListener("click", importSkillConfirm);
  els.skillDiscoverBtn.addEventListener("click", openSkillDiscover);
  els.skillDiscoverCancelBtn.addEventListener("click", closeSkillDiscover);
  els.skillDiscoverImportBtn.addEventListener("click", importDiscoveredSkills);
  els.skillAdoptBtn.addEventListener("click", openSkillAdopt);
  els.skillAdoptCancelBtn.addEventListener("click", closeSkillAdopt);
  els.skillAdoptConfirmBtn.addEventListener("click", adoptWorkspaceSkills);
  els.skillDiscoverList.addEventListener("change", (e) => {
    if (e.target && e.target.type === "checkbox") refreshDiscoverGate();
  });
  els.skillAdoptList.addEventListener("change", (e) => {
    if (e.target && e.target.type === "checkbox") refreshAdoptGate();
  });
  els.skillSourcePath.addEventListener("input", () => {
    els.skillImportConfirmBtn.disabled = true;
    els.skillInspectionPreview.hidden = true;
  });

  // Skill list interactions
  els.skillList.addEventListener("click", (e) => {
    if (busy) return;
    if (handleDescToggleClick(e)) return;
    const btn = e.target.closest("[data-act]");
    const row = e.target.closest(".skill-row[data-id]");
    if (!row) return;
    const id = row.getAttribute("data-id");
    const name = row.getAttribute("data-name");
    if (btn) {
      const act = btn.getAttribute("data-act");
      if (act === "menu") {
        const wrap = btn.closest(".pmenu-wrap");
        const menu = wrap && wrap.querySelector(".pmenu");
        if (!menu) return;
        const wasOpen = !menu.hidden;
        closeHeaderMenus();
        closeAllMenus();
        if (!wasOpen) {
          positionProfileMenu(menu, btn);
          btn.setAttribute("aria-expanded", "true");
        }
        return;
      }
      if (act === "delete") { menuConfirmDelete(btn, () => doRemoveSkill(id)); return; }
      closeAllMenus();
      if (act === "edit") openSkillFile(id, name);
      else if (act === "openfolder") openSkillFolder(id, name);
    } else if (e.target.type === "checkbox") {
      toggleSkill(id, e.target.checked);
    }
  });
}

// ── Tabs Navigation ──
function switchTab(tab) {
  if (busy) return;
  hideSkip();
  closeAllMenus();
  closeHeaderMenus();
  setMsg("");
  setSkillMsg("");
  setMcpMsg("");

  const tabs = { profiles: els.tabProfiles, skills: els.tabSkills, mcp: els.tabMcp };
  for (const [name, btn] of Object.entries(tabs)) {
    const on = name === tab;
    btn.classList.toggle("active", on);
    btn.setAttribute("aria-selected", on ? "true" : "false");
  }

  const isProfiles = tab === "profiles";
  els.panelBody.hidden = !isProfiles;
  els.skillPane.hidden = tab !== "skills";
  els.mcpPane.hidden = tab !== "mcp";
  els.advSec.hidden = !isProfiles;
  if (els.runtimeStatusSec) els.runtimeStatusSec.hidden = !isProfiles;

  // Reset every tab's sub-view so leaving a form never leaves view-form stuck.
  showSkillView("list");
  showMcpView("list");
  if (isProfiles) {
    showView("list");
  } else {
    // Clear profile form state without re-showing ports/status on Skills/MCP.
    els.listSec.hidden = false;
    els.wizSec.hidden = true;
    els.connSec.hidden = true;
    closeWizPresetMenu();
    hideSkip();
    els.panel.classList.remove("view-form");
  }

  if (tab === "skills") loadSkills();
  else if (tab === "mcp") loadMcp();
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

function setSkillMsg(text, kind = "err") {
  const t = text ? String(text) : "";
  const cls = kind === "err" ? "err" : "ok";
  els.skillMsg.textContent = t;
  els.skillMsg.className = "msg" + (t ? " " + cls : "");
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
    const reqTags = (s.requirements || []).map(r => `<span class="skill-req-tag" title="${escapeHtml(r)}">${escapeHtml(r)}</span>`).join("");
    const dateText = formatImportedAt(s.importedAt);
    return `
      <div class="skill-row${enabledClass}" data-id="${escapeHtml(s.id)}" data-name="${escapeHtml(s.name)}">
        <div class="skill-row-top">
          <div class="skill-title-group">
            <input type="checkbox"${checked} />
            <span class="skill-name" title="${escapeHtml(s.name)}">${escapeHtml(s.name)}</span>
            ${s.builtin ? `<span class="badge on" title="${escapeHtml(S().skillBuiltinHint)}">${escapeHtml(S().mcpBuiltinBadge)}</span>` : ""}
          </div>
          <div class="pmenu-wrap">
            <button type="button" class="abtn pmenu-btn" data-act="menu" aria-haspopup="true" aria-expanded="false" title="${escapeHtml(S().menuMore)}">⋯</button>
            <div class="pmenu" hidden role="menu">
              <button type="button" class="pmenu-item" data-act="edit" role="menuitem">${escapeHtml(S().edit)}</button>
              <button type="button" class="pmenu-item" data-act="openfolder" role="menuitem">${escapeHtml(S().openFolder)}</button>
              <button type="button" class="pmenu-item danger" data-act="delete" role="menuitem">${escapeHtml(S().delete)}</button>
            </div>
          </div>
        </div>
        ${skillDescHtml(s.description)}
        <div class="skill-meta">
          <span>大小: ${escapeHtml(formatBytes(s.sizeBytes))}</span>
          ${dateText ? `· <span>导入: ${escapeHtml(dateText)}</span>` : ""}
          ${reqTags ? `· <div class="skill-reqs-list">${reqTags}</div>` : ""}
        </div>
      </div>
    `;
  }).join("");
  refreshDescToggles(els.skillList);
}

function skillDescHtml(description) {
  if (!description) return "";
  return `
    <div class="skill-desc-block">
      <div class="skill-desc">${escapeHtml(description)}</div>
      <button type="button" class="skill-desc-toggle" data-act="toggle-desc" hidden aria-expanded="false">${escapeHtml(S().descExpand)}</button>
    </div>
  `;
}

function refreshDescToggles(listEl) {
  if (!listEl) return;
  listEl.querySelectorAll(".skill-desc-block").forEach((block) => {
    const desc = block.querySelector(".skill-desc");
    const toggle = block.querySelector(".skill-desc-toggle");
    if (!desc || !toggle) return;
    desc.classList.remove("is-expanded", "is-truncatable");
    toggle.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    toggle.textContent = S().descExpand;
    desc.removeAttribute("title");
    requestAnimationFrame(() => {
      const needs = desc.scrollHeight > desc.clientHeight + 1;
      toggle.hidden = !needs;
      desc.classList.toggle("is-truncatable", needs);
      if (needs) desc.title = desc.textContent || "";
      else desc.removeAttribute("title");
    });
  });
}

function toggleSkillDesc(block) {
  const desc = block.querySelector(".skill-desc");
  const toggle = block.querySelector(".skill-desc-toggle");
  if (!desc) return;
  const expanded = desc.classList.toggle("is-expanded");
  desc.classList.toggle("is-truncatable", !expanded);
  if (toggle) {
    toggle.hidden = false;
    toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
    toggle.textContent = expanded ? S().descCollapse : S().descExpand;
  }
  if (expanded) {
    desc.removeAttribute("title");
  } else {
    requestAnimationFrame(() => {
      const needs = desc.scrollHeight > desc.clientHeight + 1;
      if (toggle) toggle.hidden = !needs;
      desc.classList.toggle("is-truncatable", needs);
      if (needs) desc.title = desc.textContent || "";
      else desc.removeAttribute("title");
    });
  }
}

function handleDescToggleClick(e) {
  const toggleBtn = e.target.closest("[data-act='toggle-desc']");
  const descEl = e.target.closest(".skill-desc.is-truncatable, .skill-desc.is-expanded");
  if (!toggleBtn && !descEl) return false;
  const block = (toggleBtn || descEl).closest(".skill-desc-block");
  if (!block) return false;
  e.preventDefault();
  toggleSkillDesc(block);
  return true;
}

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
    setSkillMsg("");
    await loadSkills();
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

async function openSkillFile(id) {
  setBusy(true);
  try {
    await call("open_skill_file", { input: { skillId: id } });
    setSkillMsg("");
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

async function openSkillFolder(id) {
  setBusy(true);
  try {
    await call("open_skill_folder", { input: { skillId: id } });
    setSkillMsg("");
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

async function doRemoveSkill(id) {
  setBusy(true);
  try {
    await call("remove_skill", { input: { skillId: id } });
    setSkillMsg("");
    await loadSkills();
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

// ── Skill Create (new from scratch) Modal ──
// Tracks whether the user hand-edited the SKILL.md body. While false, the body
// is regenerated from the name/description fields so front-matter stays in sync;
// once true, the body textarea is authoritative (single source of truth on save).
let skillBodyDirty = false;

function skillCreateTemplate(name, desc) {
  const n = name || "";
  const d = desc || "";
  return `---
name: ${n}
description: ${d}
---

# ${n}

<在此填写这个 Skill 的使用说明与触发条件…>
`;
}

function syncSkillCreateBody() {
  if (skillBodyDirty) return;
  els.skillCreateBody.value = skillCreateTemplate(
    els.skillCreateName.value.trim(),
    els.skillCreateDesc.value.trim()
  );
}

function openSkillCreate() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
  els.skillCreateName.value = "";
  els.skillCreateDesc.value = "";
  skillBodyDirty = false;
  els.skillCreateBody.value = skillCreateTemplate("", "");
  els.skillCreateInspection.hidden = true;
  els.skillCreateErrors.hidden = true;
  showSkillView("create");
  els.skillCreateName.focus();
}

function closeSkillCreate() {
  showSkillView("list");
}

async function saveNewSkill() {
  // The body textarea is authoritative: name/description are parsed from its
  // front-matter by the backend, so the fields and front-matter never diverge.
  const content = els.skillCreateBody.value;
  els.skillCreateInspection.hidden = true;
  els.skillCreateErrors.hidden = true;
  setBusy(true);
  try {
    const result = await call("create_skill", { input: { content } });
    closeSkillCreate();
    await loadSkills();
    if (result && result.needsRestart) {
      setSkillMsg("已新建 Skill；沙箱已停止，正在重新启动…", "info");
      try {
        await call("one_click_login");
        setSkillMsg("已新建并重启完成", "info");
      } catch (e) {
        setSkillMsg(`已新建，但重新启动失败：${resolveBackendErr(e)}`);
      }
    } else {
      setSkillMsg("已新建 Skill", "info");
    }
  } catch (e) {
    els.skillCreateInspection.hidden = false;
    els.skillCreateErrors.hidden = false;
    els.skillCreateErrors.textContent = resolveBackendErr(e);
  } finally {
    setBusy(false);
  }
}

// ── Skill path import (advanced, nested under scan page) ──
function resetSkillPathImport() {
  if (els.skillPathAdv) els.skillPathAdv.open = false;
  if (els.skillSourcePath) els.skillSourcePath.value = "";
  if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.disabled = true;
  if (els.skillInspectionPreview) els.skillInspectionPreview.hidden = true;
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
    closeSkillDiscover();
    await loadSkills();
  } catch (e) {
    els.inspErrors.hidden = false;
    els.inspErrors.textContent = resolveBackendErr(e);
    els.skillInspectionPreview.hidden = false;
  } finally {
    setBusy(false);
  }
}

// ── Skill Discovery (scan → pick → import; same pattern as MCP) ──
async function openSkillDiscover() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
  resetSkillPathImport();
  els.skillDiscoverList.innerHTML = `<p class="hint">${escapeHtml(S().skillScanning)}</p>`;
  els.skillDiscoverEmpty.hidden = true;
  els.skillDiscoverImportBtn.disabled = true;
  showSkillView("discover");
  try {
    const found = (await call("discover_skills")) || [];
    renderDiscover(found);
  } catch (e) {
    els.skillDiscoverList.innerHTML = "";
    els.skillDiscoverEmpty.hidden = false;
    setSkillMsg(resolveBackendErr(e));
  }
}

function closeSkillDiscover() {
  resetSkillPathImport();
  showSkillView("list");
}

function renderDiscover(found) {
  if (!found.length) {
    els.skillDiscoverList.innerHTML = "";
    els.skillDiscoverEmpty.hidden = false;
    return;
  }
  els.skillDiscoverEmpty.hidden = true;
  const importedBadge = S().skillAlreadyImported;
  els.skillDiscoverList.innerHTML = found.map((d) => {
    const disabled = d.alreadyImported ? " disabled" : "";
    const badge = d.alreadyImported ? `<span class="skill-req-tag">${escapeHtml(importedBadge)}</span>` : "";
    return `
      <label class="skill-discover-row${d.alreadyImported ? " disabled" : ""}">
        <input type="checkbox" value="${escapeHtml(d.sourcePath)}"${disabled} />
        <span class="skill-discover-main">
          <span class="skill-name">${escapeHtml(d.name)} ${badge}</span>
          ${d.description ? `<span class="skill-desc">${escapeHtml(d.description)}</span>` : ""}
          <span class="skill-meta"><span>${escapeHtml(d.sourceLabel)}</span></span>
        </span>
      </label>
    `;
  }).join("");
  refreshDiscoverGate();
}

function refreshDiscoverGate() {
  const checked = els.skillDiscoverList.querySelectorAll("input[type=checkbox]:checked");
  els.skillDiscoverImportBtn.disabled = busy || checked.length === 0;
}

async function importDiscoveredSkills() {
  const paths = Array.from(
    els.skillDiscoverList.querySelectorAll("input[type=checkbox]:checked")
  ).map((el) => el.value);
  if (!paths.length) return;
  setBusy(true);
  const failures = [];
  try {
    for (const p of paths) {
      try {
        await call("import_skill", { input: { sourcePath: p } });
      } catch (e) {
        failures.push(`${p}: ${resolveBackendErr(e)}`);
      }
    }
    closeSkillDiscover();
    await loadSkills();
    if (failures.length) setSkillMsg(`部分导入失败: ${failures.join("; ")}`);
  } finally {
    setBusy(false);
  }
}

// ── Science Workspace Skill Adopt ──
async function openSkillAdopt() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
  els.skillAdoptList.innerHTML = `<p class="hint">${escapeHtml(S().skillScanning)}</p>`;
  els.skillAdoptEmpty.hidden = true;
  els.skillAdoptConfirmBtn.disabled = true;
  showSkillView("adopt");
  try {
    const found = (await call("discover_workspace_skills")) || [];
    renderSkillAdopt(found);
  } catch (e) {
    els.skillAdoptList.innerHTML = "";
    els.skillAdoptEmpty.hidden = false;
    setSkillMsg(resolveBackendErr(e));
  }
}

function closeSkillAdopt() {
  showSkillView("list");
}

function renderSkillAdopt(found) {
  if (!found.length) {
    els.skillAdoptList.innerHTML = "";
    els.skillAdoptEmpty.hidden = false;
    return;
  }
  els.skillAdoptEmpty.hidden = true;
  const adoptedBadge = S().skillAdoptedBadge;
  els.skillAdoptList.innerHTML = found.map((d) => {
    const files = (d.files || []).slice(0, 6).join(", ");
    const more = (d.files || []).length > 6 ? ` +${d.files.length - 6}` : "";
    const warn = (d.warnings || []).length
      ? `<span class="skill-meta"><span>${escapeHtml(d.warnings[0])}</span></span>`
      : "";
    const badge = d.alreadyImported ? `<span class="skill-req-tag">${escapeHtml(adoptedBadge)}</span>` : "";
    return `
      <label class="skill-discover-row">
        <input type="checkbox" value="${escapeHtml(d.key)}" />
        <span class="skill-discover-main">
          <span class="skill-name">${escapeHtml(d.name)} ${badge}</span>
          ${d.description ? `<span class="skill-desc">${escapeHtml(d.description)}</span>` : ""}
          <span class="skill-meta"><span>workspace: ${escapeHtml(d.workspaceId || "")}</span></span>
          ${files ? `<span class="skill-meta"><span>${escapeHtml(files)}${more}</span></span>` : ""}
          ${warn}
        </span>
      </label>
    `;
  }).join("");
  refreshAdoptGate();
}

function refreshAdoptGate() {
  const checked = els.skillAdoptList.querySelectorAll("input[type=checkbox]:checked");
  els.skillAdoptConfirmBtn.disabled = busy || checked.length === 0;
}

async function adoptWorkspaceSkills() {
  const keys = Array.from(
    els.skillAdoptList.querySelectorAll("input[type=checkbox]:checked")
  ).map((el) => el.value);
  if (!keys.length) return;
  setBusy(true);
  try {
    const result = await call("adopt_workspace_skills", { input: { keys } });
    closeSkillAdopt();
    await loadSkills();
    const failures = (result.failures || []).join("; ");
    if (result.needsRestart) {
      setSkillMsg("已采纳 Skill；沙箱已停止，正在重新启动…", "info");
      try {
        await call("one_click_login");
        setSkillMsg("已采纳并重启完成", "info");
      } catch (e) {
        setSkillMsg(`已采纳，但重新启动失败：${resolveBackendErr(e)}`);
      }
    } else if (failures) {
      setSkillMsg(`部分采纳失败: ${failures}`);
    }
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

// ── Local MCP Manager ──
let mcpCache = [];
let mcpEditId = null;
// Set once warnings have been shown for the current form state, so a second
// "保存" click confirms past non-blocking warnings (errors always block).
let mcpWarnAck = false;

async function openMcpDiscover() {
  closeMenu(els.mcpMenu, els.mcpMoreBtn);
  if (busy) return;
  els.mcpDiscoverList.innerHTML = "<p class=\"hint\">扫描中…</p>";
  els.mcpDiscoverEmpty.hidden = true;
  els.mcpDiscoverImportBtn.disabled = true;
  showMcpView("discover");
  try {
    const found = (await call("discover_mcp_servers")) || [];
    renderMcpDiscover(found);
  } catch (e) {
    els.mcpDiscoverList.innerHTML = "";
    els.mcpDiscoverEmpty.hidden = false;
    setMcpMsg(resolveBackendErr(e));
  }
}

function closeMcpDiscover() {
  showMcpView("list");
}

function renderMcpDiscover(found) {
  if (!found.length) {
    els.mcpDiscoverList.innerHTML = "";
    els.mcpDiscoverEmpty.hidden = false;
    return;
  }
  els.mcpDiscoverEmpty.hidden = true;
  els.mcpDiscoverList.innerHTML = found.map((d) => {
    const disabled = d.alreadyImported ? " disabled" : "";
    const badge = d.alreadyImported ? `<span class="skill-req-tag">已导入</span>` : "";
    const env = (d.envKeys || []).length
      ? `<span class="skill-meta"><span>env: ${escapeHtml((d.envKeys || []).join(", "))}</span></span>`
      : "";
    const args = (d.args || []).length ? ` ${escapeHtml((d.args || []).join(" "))}` : "";
    return `
      <label class="skill-discover-row${d.alreadyImported ? " disabled" : ""}">
        <input type="checkbox" data-source-path="${escapeHtml(d.sourcePath)}" value="${escapeHtml(d.name)}"${disabled} />
        <span class="skill-discover-main">
          <span class="skill-name">${escapeHtml(d.name)} ${badge}</span>
          ${d.description ? `<span class="skill-desc">${escapeHtml(d.description)}</span>` : ""}
          <span class="mcp-cmd">${escapeHtml(d.command)}${args}</span>
          ${env}
          <span class="skill-meta"><span>${escapeHtml(d.sourceLabel)}</span></span>
        </span>
      </label>
    `;
  }).join("");
  refreshMcpDiscoverGate();
}

function refreshMcpDiscoverGate() {
  const checked = els.mcpDiscoverList.querySelectorAll("input[type=checkbox]:checked");
  els.mcpDiscoverImportBtn.disabled = busy || checked.length === 0;
}

async function importDiscoveredMcpServers() {
  const selected = Array.from(
    els.mcpDiscoverList.querySelectorAll("input[type=checkbox]:checked")
  ).map((el) => ({ sourcePath: el.dataset.sourcePath, name: el.value }));
  if (!selected.length) return;
  setBusy(true);
  const failures = [];
  try {
    for (const item of selected) {
      try {
        await call("import_discovered_mcp_server", { input: item });
      } catch (e) {
        failures.push(`${item.name}: ${resolveBackendErr(e)}`);
      }
    }
    closeMcpDiscover();
    await loadMcp();
    if (failures.length) setMcpMsg(`部分导入失败: ${failures.join("; ")}`);
  } finally {
    setBusy(false);
  }
}

async function loadMcp() {
  try {
    const list = await call("list_mcp_servers");
    mcpCache = list || [];
    renderMcp(mcpCache);
  } catch (e) {
    setMcpMsg(resolveBackendErr(e));
  }
}

function setMcpMsg(text) {
  const t = text ? String(text) : "";
  els.mcpMsg.textContent = t;
  els.mcpMsg.className = "msg" + (t ? " err" : "");
  els.mcpMsg.parentElement.hidden = !t;
}

function renderMcp(list) {
  if (!list.length) {
    els.mcpEmpty.hidden = false;
    els.mcpList.innerHTML = "";
    return;
  }
  els.mcpEmpty.hidden = true;
  els.mcpList.innerHTML = list.map((s) => {
    const enabledClass = s.enabled ? "" : " disabled";
    const checked = s.enabled ? " checked" : "";
    const cmdLine = escapeHtml([s.command, ...(s.args || [])].join(" "));
    const envKeys = Object.keys(s.env || {});
    const envTags = envKeys.map((k) => `<span class="skill-req-tag" title="${escapeHtml(k)}">${escapeHtml(k)}</span>`).join("");
    return `
      <div class="skill-row${enabledClass}" data-id="${escapeHtml(s.id)}" data-name="${escapeHtml(s.name)}">
        <div class="skill-row-top">
          <div class="skill-title-group">
            <input type="checkbox"${checked} />
            <span class="skill-name" title="${escapeHtml(s.name)}">${escapeHtml(s.name)}</span>
            ${s.builtin ? `<span class="badge on" title="${escapeHtml(S().mcpBuiltinHint)}">${escapeHtml(S().mcpBuiltinBadge)}</span>` : ""}
          </div>
          <div class="pmenu-wrap">
            <button type="button" class="abtn pmenu-btn" data-act="menu" aria-haspopup="true" aria-expanded="false" title="${escapeHtml(S().menuMore)}">⋯</button>
            <div class="pmenu" hidden role="menu">
              <button type="button" class="pmenu-item" data-act="edit" role="menuitem">${escapeHtml(S().edit)}</button>
              <button type="button" class="pmenu-item danger" data-act="delete" role="menuitem">${escapeHtml(S().delete)}</button>
            </div>
          </div>
        </div>
        ${skillDescHtml(s.description)}
        <div class="skill-meta">
          <span class="mcp-cmd" title="${cmdLine}">${cmdLine}</span>
          ${envTags ? `· <div class="skill-reqs-list">${envTags}</div>` : ""}
        </div>
      </div>
    `;
  }).join("");
  refreshDescToggles(els.mcpList);
}

async function toggleMcpServer(id, enabled) {
  setBusy(true);
  try {
    await call("set_mcp_server_enabled", { input: { serverId: id, enabled } });
    setMcpMsg("");
    await loadMcp();
  } catch (e) {
    setMcpMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

async function doRemoveMcpServer(id) {
  setBusy(true);
  try {
    await call("remove_mcp_server", { input: { serverId: id } });
    setMcpMsg("");
    await loadMcp();
  } catch (e) {
    setMcpMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

function openMcpModal(id) {
  if (busy) return;
  closeMenu(els.mcpMenu, els.mcpMoreBtn);
  mcpEditId = id || null;
  const s = id ? mcpCache.find((x) => x.id === id) : null;
  els.mcpModalTitle.textContent = s ? "编辑本地 MCP" : "新增本地 MCP";
  els.mcpName.value = s ? s.name : "";
  els.mcpDesc.value = s ? (s.description || "") : "";
  els.mcpCommand.value = s ? s.command : "";
  els.mcpArgs.value = s ? (s.args || []).join("\n") : "";
  // Env values are returned masked; on edit we show keys with blank values so the
  // user re-enters secrets intentionally (never round-trip a masked value).
  els.mcpEnv.value = s ? Object.keys(s.env || {}).map((k) => `${k}=`).join("\n") : "";
  els.mcpInspection.hidden = true;
  els.mcpWarnings.hidden = true;
  els.mcpErrors.hidden = true;
  mcpWarnAck = false;
  showMcpView("form");
  els.mcpName.focus();
}

function closeMcpModal() {
  showMcpView("list");
  mcpEditId = null;
  mcpWarnAck = false;
}

function parseArgsLines(text) {
  return text.split("\n").map((l) => l.trim()).filter((l) => l.length > 0);
}

function parseEnvLines(text) {
  const env = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const k = line.slice(0, eq).trim();
    const v = line.slice(eq + 1).trim();
    if (k) env[k] = v;
  }
  return env;
}

async function saveMcpServer() {
  const input = {
    name: els.mcpName.value.trim(),
    description: els.mcpDesc.value.trim(),
    command: els.mcpCommand.value.trim(),
    args: parseArgsLines(els.mcpArgs.value),
    env: parseEnvLines(els.mcpEnv.value),
  };
  els.mcpWarnings.hidden = true;
  els.mcpErrors.hidden = true;
  els.mcpInspection.hidden = true;
  try {
    const insp = await call("inspect_mcp_server", { input });
    if (insp && !insp.valid) {
      // Errors always block the save; reset any prior warning acknowledgement.
      mcpWarnAck = false;
      els.mcpErrors.hidden = false;
      els.mcpErrors.textContent = `错误: ${(insp.errors || []).join("; ")}`;
      els.mcpInspection.hidden = false;
      return;
    }
    const warnings = (insp && insp.warnings) || [];
    if (warnings.length && !mcpWarnAck) {
      // Surface warnings and require a second click to confirm past them.
      mcpWarnAck = true;
      els.mcpWarnings.hidden = false;
      els.mcpWarnings.textContent = `警告: ${warnings.join("; ")}（如确认无误，请再次点击「保存」）`;
      els.mcpInspection.hidden = false;
      return;
    }
  } catch (e) {
    mcpWarnAck = false;
    els.mcpErrors.hidden = false;
    els.mcpErrors.textContent = resolveBackendErr(e);
    els.mcpInspection.hidden = false;
    return;
  }

  setBusy(true);
  try {
    if (mcpEditId) {
      await call("update_mcp_server", { input: { serverId: mcpEditId, server: input } });
    } else {
      await call("create_mcp_server", { input });
    }
    closeMcpModal();
    await loadMcp();
  } catch (e) {
    els.mcpErrors.hidden = false;
    els.mcpErrors.textContent = resolveBackendErr(e);
    els.mcpInspection.hidden = false;
  } finally {
    setBusy(false);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  wire();
  await loadConfig();
  startRuntimeStatusPolling();
  window.addEventListener("focus", () => {
    if (!busy) {
      loadConfig({ keepView: true }).catch(() => {});
      refreshRuntimeStatus().catch(() => {});
      if (!els.skillPane.hidden) loadSkills().catch(() => {});
      if (!els.mcpPane.hidden) loadMcp().catch(() => {});
    }
  });
});
