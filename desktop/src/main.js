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
    modelPickSearch: "搜索模型…",
    modelPickSearchEmpty: "没有匹配的模型",
    ports: "端口管理",
    runtimeStatus: "运行状态",
    runStatusOff: "未运行",
    runStatusProxy: "代理运行中·Science 未启动",
    runStatusScience: "Science 运行中·代理未启动",
    runStatusBoth: "代理+Science 运行中",
    runStatusStarting: "启动中",
    runStatusStopping: "停止中",
    runStatusTipOff: "代理与 Science 均未运行。",
    runStatusTipProxy: "本地代理在跑，但 Science 沙箱未启动。",
    runStatusTipScience: "Science 沙箱在跑，但代理未启动；需（重新）启动代理后才能作为代理使用。",
    runStatusTipBoth: "代理与 Science 沙箱均已就绪，可作为代理使用。",
    runStatusTipStarting: "正在启动代理与 Science…",
    runStatusTipStopping: "正在停止代理与 Science…",
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
    createSkill: "新建 Skill",
    addMcp: "+ 新建",
    scanImport: "扫描导入",
    importFolder: "手动导入",
    adoptFromScience: "同步 Science 技能库",
    skillsManageTitle: "Skills 管理",
    skillApplyHint: "启用/停用的改动会在下次点击「启动 Claude Science」时生效；若沙箱正在运行，会自动重启以应用。",
    skillEmptyTitle: "还没有 Skill。",
    skillEmptyHint: "点「导入」添加本地或网络 Skill，或在「⋯」里新建 / 扫描 / 同步 Science 技能库。",
    skillDiscoverTitle: "扫描本地 Skill",
    skillDiscoverHintHtml: "从其他 Agent 软件导入。",
    skillDiscoverEmpty: "没有扫描到可导入的 Skill。",
    skillDiscoverImport: "导入所选",
    skillImportTitle: "导入 Skill",
    skillImportIntroHtml: "粘贴路径 / URL，或点「浏览」选本地文件夹或 ZIP。",
    skillPathLabel: "路径或 URL",
    skillPathPlaceholder: "/path、.zip 或 https://…",
    skillBrowse: "浏览",
    skillPickTitle: "选择 Skill 文件夹或 ZIP 包",
    skillPathHintHtml: "需含 <code>SKILL.md</code>；支持公开 GitHub 目录链接。",
    skillInspPreviewTitle: "预览",
    skillImportPath: "导入",
    skillAlreadyImported: "已导入",
    skillScanning: "扫描中…",
    skillAdoptTitle: "同步 Science 技能库",
    skillAdoptHintHtml: "优先回收 <code>orgs/…/skills/</code> 中 Science 已改过的托管技能到 <code>~/.csp/skills/</code>；也可导入库中尚未入库的技能，或工作区未入库草稿。",
    skillAdoptEmpty: "没有需要同步的技能。",
    skillAdoptConfirm: "同步所选",
    skillAdoptedBadge: "可回收",
    skillAdoptPreview: "预览",
    skillAdoptPreviewTitle: "预览",
    skillAdoptOpen: "在 Finder 打开",
    skillAdoptPreviewClose: "关闭",
    skillAdoptPreviewMeta: "{file} · {chars} 字符 · {bytes}",
    skillAdoptPreviewTruncated: "（内容过长，已截断显示）",
    skillAdoptReadoptHint: "Science 侧已有改动；同步会写回 CSP 库。",
    skillAdoptPreviewFail: "预览失败：{err}",
    skillSyncKindHarvest: "回收更新",
    skillSyncKindImport: "新入库",
    skillSyncKindWorkspace: "工作区草稿",
    skillSyncBytes: "库 {store} → Science {science}",
    skillSynced: "已同步 Skill",
    mcpDiscoverPreview: "预览",
    mcpPreviewTitle: "MCP 配置预览",
    mcpPreviewMeta: "{name} · {source}",
    mcpPreviewFail: "预览失败：{err}",
    mcpPreviewOpen: "打开配置文件",
    skillCreateTitle: "新建 Skill",
    skillCreated: "已新建 Skill",
    skillCreatedRestarting: "已新建 Skill；沙箱已停止，正在重新启动…",
    skillCreatedRestarted: "已新建并重启完成",
    skillCreatedRestartFail: "已新建，但重新启动失败：{err}",
    skillImported: "已导入 Skill",
    skillImportedRestarting: "已导入 Skill；沙箱已停止，正在重新启动…",
    skillImportedRestarted: "已导入并重启完成",
    skillImportedRestartFail: "已导入，但重新启动失败：{err}",
    skillImportPartialFail: "部分导入失败: {err}",
    skillToggledRestarting: "已更新启用状态；沙箱已停止，正在重新启动…",
    skillToggledRestarted: "已应用并重启完成",
    skillToggledRestartFail: "已更新，但重新启动失败：{err}",
    skillAdoptedRestarting: "已同步 Skill；沙箱已停止，正在重新启动…",
    skillAdoptedRestarted: "已同步并重启完成",
    skillAdoptedRestartFail: "已同步，但重新启动失败：{err}",
    skillAdoptPartialFail: "部分同步失败: {err}",
    metaSize: "大小",
    metaImported: "导入",
    inspUnnamedSkill: "未命名 Skill",
    inspNoDesc: "无描述",
    inspFileCountSize: "文件数: {count} · 大小: {size}",
    inspRequirements: "依赖环境: {reqs}",
    inspNone: "无",
    inspWarningsPrefix: "警告: {msg}",
    inspErrorsPrefix: "错误: {msg}",
    mcpManageTitle: "MCP 管理",
    mcpApplyHint: "新增/编辑/启用的改动会在下次点击「启动 Claude Science」时生效；若沙箱正在运行，会自动重启以应用。",
    mcpEmptyTitle: "还没有 MCP。",
    mcpEmptyHintHtml: "点「新建」，配置本地 <code>stdio</code>（command/args/env）或远程 HTTP/SSE（url/headers）。stdio 写入 <code>local-mcp.json</code>；远程写入 org 库 <code>custom_mcp_servers</code>。",
    mcpFormTitleNew: "新增 MCP",
    mcpFormTitleEdit: "编辑 MCP",
    mcpTransportLabel: "连接类型",
    mcpTransportHintHtml: "本地→<code>local-mcp.json</code>；远程→Science <code>custom_mcp_servers</code>。",
    mcpTransportStdio: "本地命令（stdio）",
    mcpTransportHttp: "远程 HTTP / Streamable HTTP",
    mcpTransportSse: "远程 SSE",
    mcpNameHintHtml: "本地：字母/数字/<code>-_.</code>；远程：小写字母/数字/<code>-</code>。",
    mcpUrlLabel: "URL",
    mcpUrlHintHtml: "不要把 token 写进 URL，请用下方 Headers。",
    mcpHeadersLabel: "Headers（可选，每行 Name=Value）",
    mcpHeadersHintHtml: "密钥本地 0600 保存，部署为 <code>headers_helper</code>；编辑语义同 env。",
    mcpCommandLabel: "命令",
    mcpCommandHintHtml: "命令（<code>python3</code>/<code>node</code>/<code>npx</code>…）或绝对路径。",
    mcpArgsLabel: "参数（每行一个）",
    mcpArgsHintHtml: "绝对路径会自动加入沙箱读权限。",
    mcpEnvLabel: "环境变量（每行 KEY=VALUE）",
    mcpEnvHintHtml: "编辑时 <code>KEY=</code> 留空保留原值，删行删除，新值覆盖。",
    mcpDiscoverTitle: "扫描 MCP",
    mcpDiscoverHintHtml: "从其他 Agent 软件导入。",
    mcpDiscoverEmpty: "没有扫描到可导入的 MCP。",
    mcpBadgeStdio: "stdio",
    mcpBadgeHttp: "HTTP",
    mcpBadgeSse: "SSE",
    mcpDiscoverImport: "导入所选",
    mcpScanning: "扫描中…",
    mcpNetworkAllowlist: "网络授权配置",
    mcpEditJson: "编辑 JSON",
    mcpImportPartialFail: "部分导入失败: {err}",
    mcpSavedRestarting: "已保存 MCP；沙箱已停止，正在重新启动…",
    mcpSavedRestarted: "已保存并重启完成",
    mcpSavedRestartFail: "已保存，但重新启动失败：{err}",
    mcpImportedRestarting: "已导入 MCP；沙箱已停止，正在重新启动…",
    mcpImportedRestarted: "已导入并重启完成",
    mcpImportedRestartFail: "已导入，但重新启动失败：{err}",
    mcpToggledRestarting: "已更新启用状态；沙箱已停止，正在重新启动…",
    mcpToggledRestarted: "已应用并重启完成",
    mcpToggledRestartFail: "已更新，但重新启动失败：{err}",
    mcpWarnConfirm: "警告: {msg}（如确认无误，请再次点击「保存」）",
    mcpDiscoverBtn: "扫描导入",
    modelHintNative: "由 Science 选择器 + 内置映射自动选择（opus 深度 / haiku 快速）。",
    modelHintFixed: "勾选要在 Science 中启用的模型；列表第一个用于后台任务兜底。",
    metaManyModels: "{n} 个模型已启用",
    platterCardTitle: "多提供商 · 自选模型",
    platterEmptyMeta: "未选用 · 点击配置",
    platterMeta: "{n} 个模型 · 跨 {providers} 个提供商",
    platterDefaultBadge: "默认",
    platterEdit: "配置",
    platterHint: "从已保存的提供商中勾选模型（最多 {max} 个）。第一个为默认模型。",
    platterSelectedLabel: "已选模型（顺序即优先级）",
    platterImportBtn: "从当前生效连接导入",
    platterSaveBtn: "保存",
    platterActivateBtn: "设为当前生效",
    platterCapHint: "已选 {n}/{max}",
    platterBrowseAll: "浏览全部模型",
    platterCatalogLoading: "正在获取模型目录…",
    platterCatalogUnsupported: "该端点不支持模型目录（/v1/models）。",
    platterCatalogNetworkErr: "获取模型目录失败（网络/上游繁忙）。",
    platterCatalogEmpty: "目录中没有更多可添加的模型。",
    platterManualPlaceholder: "手动输入模型 ID",
    platterManualAdd: "添加",
    errScienceModelCap: "Science 最多同时启用 {max} 个模型。",
    errPlatterEmpty: "请至少选择一个模型。",
    errPlatterAdapterUnsupported: "「{name}」无法加入拼盘（请先填写 API Key）。",
    platterSaveOk: "拼盘已保存。",
    platterSaveReloaded: "拼盘已保存，代理已按新配置重启。",
    profileDiscoverBtn: "扫描导入",
    profileDiscoverTitle: "扫描本地 LLM",
    profileDiscoverHintHtml: "扫描 Agent/编码软件里的本地自定义 LLM（Zed、Continue、OpenCode、OpenClaw / QClaw、Factory、Cline、Aider、Codex、Qwen Code、iFlow、Crush，以及 Cursor / Claude Code / Trae（含 TRAE SOLO）的自定义 endpoint）。仅账号登录、无自定义 endpoint 的模型不会出现。",
    profileDiscoverEmpty: "没有扫描到可导入的提供商。",
    profileDiscoverImport: "导入所选",
    profileDiscoverScanning: "正在扫描…",
    profileDiscoverPreview: "预览",
    profileDiscoverNeedsKey: "需补 Key",
    profileDiscoverKeyEnv: "Key 来自环境变量",
    profileDiscoverKeyConfig: "Key 已自动读取",
    profileDiscoverKeyKeychain: "Key 来自钥匙串",
    profileDiscoverModelCount: "{n} 个模型",
    profileDiscoverImportOk: "已导入 {ok} 个；跳过 {skip} 个（相同 URL + Key 的配置已存在）。",
    profileDiscoverImportOkOnly: "已导入 {ok} 个。",
    profileDiscoverImportSkipOnly: "跳过 {skip} 个（相同 URL + Key 的配置已存在）。",
    profileDiscoverImportFail: "{n} 个导入失败（{err}）。",
    profileDiscoverNeedsKeyHint: "部分导入项缺少 API Key，请在编辑里补全。",
    errEditorLlmNotFound: "未找到提供商「{name}」。",
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
    modelPickSearch: "Search models…",
    modelPickSearchEmpty: "No matching models",
    ports: "Ports",
    runtimeStatus: "Runtime",
    runStatusOff: "Not running",
    runStatusProxy: "Proxy on · Science off",
    runStatusScience: "Science on · proxy off",
    runStatusBoth: "Proxy + Science running",
    runStatusStarting: "Starting…",
    runStatusStopping: "Stopping…",
    runStatusTipOff: "Neither proxy nor Science is running.",
    runStatusTipProxy: "Local proxy is up, but the Science sandbox is not.",
    runStatusTipScience: "Science is running but the proxy is down. (Re)start the proxy before using CSP as a proxy.",
    runStatusTipBoth: "Proxy and Science sandbox are ready to use as a proxy.",
    runStatusTipStarting: "Starting proxy and Science…",
    runStatusTipStopping: "Stopping proxy and Science…",
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
    createSkill: "New Skill",
    addMcp: "+ New",
    scanImport: "Scan & import",
    importFolder: "Manual import",
    adoptFromScience: "Sync Science skills",
    skillsManageTitle: "Skills",
    skillApplyHint: "Enable/disable takes effect the next time you Start Claude Science; a running sandbox restarts automatically.",
    skillEmptyTitle: "No Skills yet.",
    skillEmptyHint: "Tap Import for a local or network Skill, or use ⋯ to create / scan / sync Science skill library.",
    skillDiscoverTitle: "Scan local Skills",
    skillDiscoverHintHtml: "Import from other agent apps.",
    skillDiscoverEmpty: "No importable Skills found.",
    skillDiscoverImport: "Import selected",
    skillImportTitle: "Import Skill",
    skillImportIntroHtml: "Paste a path / URL, or use Browse for a local folder or ZIP.",
    skillPathLabel: "Path or URL",
    skillPathPlaceholder: "/path, .zip, or https://…",
    skillBrowse: "Browse",
    skillPickTitle: "Select Skill folder or zip",
    skillPathHintHtml: "Must contain <code>SKILL.md</code>; public GitHub tree URLs work.",
    skillInspPreviewTitle: "Preview",
    skillImportPath: "Import",
    skillAlreadyImported: "Imported",
    skillScanning: "Scanning…",
    skillAdoptTitle: "Sync Science skill library",
    skillAdoptHintHtml: "Primarily harvest Science edits under <code>orgs/…/skills/</code> back into <code>~/.csp/skills/</code>. Can also import library skills not yet in CSP, or unpublished workspace drafts.",
    skillAdoptEmpty: "Nothing to sync.",
    skillAdoptConfirm: "Sync selected",
    skillAdoptedBadge: "Harvest",
    skillAdoptPreview: "Preview",
    skillAdoptPreviewTitle: "Preview",
    skillAdoptOpen: "Reveal in Finder",
    skillAdoptPreviewClose: "Close",
    skillAdoptPreviewMeta: "{file} · {chars} chars · {bytes}",
    skillAdoptPreviewTruncated: "(truncated for display)",
    skillAdoptReadoptHint: "Science copy differs; sync writes it back to the CSP store.",
    skillAdoptPreviewFail: "Preview failed: {err}",
    skillSyncKindHarvest: "Harvest update",
    skillSyncKindImport: "New import",
    skillSyncKindWorkspace: "Workspace draft",
    skillSyncBytes: "store {store} → Science {science}",
    skillSynced: "Skills synced",
    mcpDiscoverPreview: "Preview",
    mcpPreviewTitle: "MCP config preview",
    mcpPreviewMeta: "{name} · {source}",
    mcpPreviewFail: "Preview failed: {err}",
    mcpPreviewOpen: "Open config file",
    skillCreateTitle: "New Skill",
    skillCreated: "Skill created",
    skillCreatedRestarting: "Skill created; sandbox stopped, restarting…",
    skillCreatedRestarted: "Created and restarted",
    skillCreatedRestartFail: "Created, but restart failed: {err}",
    skillImported: "Skill imported",
    skillImportedRestarting: "Skill imported; sandbox stopped, restarting…",
    skillImportedRestarted: "Imported and restarted",
    skillImportedRestartFail: "Imported, but restart failed: {err}",
    skillImportPartialFail: "Some imports failed: {err}",
    skillToggledRestarting: "Enable state updated; sandbox stopped, restarting…",
    skillToggledRestarted: "Applied and restarted",
    skillToggledRestartFail: "Updated, but restart failed: {err}",
    skillAdoptedRestarting: "Skills synced; sandbox stopped, restarting…",
    skillAdoptedRestarted: "Synced and restarted",
    skillAdoptedRestartFail: "Synced, but restart failed: {err}",
    skillAdoptPartialFail: "Some syncs failed: {err}",
    metaSize: "Size",
    metaImported: "Imported",
    inspUnnamedSkill: "Unnamed Skill",
    inspNoDesc: "No description",
    inspFileCountSize: "Files: {count} · Size: {size}",
    inspRequirements: "Requires: {reqs}",
    inspNone: "none",
    inspWarningsPrefix: "Warnings: {msg}",
    inspErrorsPrefix: "Errors: {msg}",
    mcpManageTitle: "MCP",
    mcpApplyHint: "Create/edit/enable changes apply the next time you Start Claude Science; a running sandbox restarts automatically.",
    mcpEmptyTitle: "No MCP yet.",
    mcpEmptyHintHtml: "Tap New to add a local <code>stdio</code> (command/args/env) or remote HTTP/SSE (url/headers) connector. Stdio writes <code>local-mcp.json</code>; remote writes org DB <code>custom_mcp_servers</code>.",
    mcpFormTitleNew: "New MCP",
    mcpFormTitleEdit: "Edit MCP",
    mcpTransportLabel: "Connection type",
    mcpTransportHintHtml: "Local→<code>local-mcp.json</code>; remote→Science <code>custom_mcp_servers</code>.",
    mcpTransportStdio: "Local command (stdio)",
    mcpTransportHttp: "Remote HTTP / Streamable HTTP",
    mcpTransportSse: "Remote SSE",
    mcpNameHintHtml: "Local: letters/digits/<code>-_.</code>; remote: lowercase letters/digits/<code>-</code>.",
    mcpUrlLabel: "URL",
    mcpUrlHintHtml: "Do not put tokens in the URL; use Headers below.",
    mcpHeadersLabel: "Headers (optional, one Name=Value per line)",
    mcpHeadersHintHtml: "Secrets stay local (0600), deploy as <code>headers_helper</code>; edit semantics match env.",
    mcpCommandLabel: "Command",
    mcpCommandHintHtml: "Command (<code>python3</code>/<code>node</code>/<code>npx</code>…) or an absolute path.",
    mcpArgsLabel: "Args (one per line)",
    mcpArgsHintHtml: "Absolute paths get sandbox read access.",
    mcpEnvLabel: "Environment (one KEY=VALUE per line)",
    mcpEnvHintHtml: "On edit: blank <code>KEY=</code> keeps value, delete line to remove, new value overwrites.",
    mcpDiscoverTitle: "Scan MCP",
    mcpDiscoverHintHtml: "Import from other agent apps.",
    mcpDiscoverEmpty: "No importable MCP found.",
    mcpBadgeStdio: "stdio",
    mcpBadgeHttp: "HTTP",
    mcpBadgeSse: "SSE",
    mcpDiscoverImport: "Import selected",
    mcpScanning: "Scanning…",
    mcpNetworkAllowlist: "Network allowlist",
    mcpEditJson: "Edit JSON",
    mcpImportPartialFail: "Some imports failed: {err}",
    mcpSavedRestarting: "MCP saved; sandbox stopped, restarting…",
    mcpSavedRestarted: "Saved and restarted",
    mcpSavedRestartFail: "Saved, but restart failed: {err}",
    mcpImportedRestarting: "MCP imported; sandbox stopped, restarting…",
    mcpImportedRestarted: "Imported and restarted",
    mcpImportedRestartFail: "Imported, but restart failed: {err}",
    mcpToggledRestarting: "Enable state updated; sandbox stopped, restarting…",
    mcpToggledRestarted: "Applied and restarted",
    mcpToggledRestartFail: "Updated, but restart failed: {err}",
    mcpWarnConfirm: "Warnings: {msg} (click Save again to confirm)",
    mcpDiscoverBtn: "Scan & import",
    modelHintNative: "Auto-mapped via Science picker + built-in routing (opus for depth, haiku for speed).",
    modelHintFixed: "Check models to enable in Science; the first is the fallback for background tasks.",
    metaManyModels: "{n} models enabled",
    platterCardTitle: "Multi-provider · custom models",
    platterEmptyMeta: "Not configured · tap to set up",
    platterMeta: "{n} models · {providers} providers",
    platterDefaultBadge: "Default",
    platterEdit: "Configure",
    platterHint: "Pick models from saved providers (max {max}). First selected is the default.",
    platterSelectedLabel: "Selected models (order = priority)",
    platterImportBtn: "Import from active connection",
    platterSaveBtn: "Save",
    platterActivateBtn: "Set active",
    platterCapHint: "Selected {n}/{max}",
    platterBrowseAll: "Browse all models",
    platterCatalogLoading: "Fetching model catalog…",
    platterCatalogUnsupported: "This endpoint does not support /v1/models.",
    platterCatalogNetworkErr: "Failed to fetch model catalog (network/upstream busy).",
    platterCatalogEmpty: "No more models to add from the catalog.",
    platterManualPlaceholder: "Enter model ID manually",
    platterManualAdd: "Add",
    errScienceModelCap: "Science supports at most {max} models at once.",
    errPlatterEmpty: "Select at least one model.",
    errPlatterAdapterUnsupported: "「{name}」 cannot join the platter (API key required).",
    platterSaveOk: "Platter saved.",
    platterSaveReloaded: "Platter saved; proxy restarted with the new config.",
    profileDiscoverBtn: "Scan & import",
    profileDiscoverTitle: "Scan local LLMs",
    profileDiscoverHintHtml: "Scan local custom LLMs from agent/coding apps (Zed, Continue, OpenCode, OpenClaw / QClaw, Factory, Cline, Aider, Codex, Qwen Code, iFlow, Crush, plus custom endpoints in Cursor / Claude Code / Trae incl. TRAE SOLO). Only models with account login and no custom endpoint won't appear.",
    profileDiscoverEmpty: "No importable providers found.",
    profileDiscoverImport: "Import selected",
    profileDiscoverScanning: "Scanning…",
    profileDiscoverPreview: "Preview",
    profileDiscoverNeedsKey: "Needs key",
    profileDiscoverKeyEnv: "Key from env",
    profileDiscoverKeyConfig: "Key auto-read",
    profileDiscoverKeyKeychain: "Key from Keychain",
    profileDiscoverModelCount: "{n} models",
    profileDiscoverImportOk: "Imported {ok}; skipped {skip} (same URL + key already exists).",
    profileDiscoverImportOkOnly: "Imported {ok}.",
    profileDiscoverImportSkipOnly: "Skipped {skip} (same URL + key already exists).",
    profileDiscoverImportFail: "{n} import(s) failed ({err}).",
    profileDiscoverNeedsKeyHint: "Some imports need an API key — edit the profile to add one.",
    errEditorLlmNotFound: "Provider 「{name}」 not found.",
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
  if (els.profileDiscoverBtn) els.profileDiscoverBtn.textContent = t.profileDiscoverBtn;
  if (els.profileDiscoverTitle) els.profileDiscoverTitle.textContent = t.profileDiscoverTitle;
  if (els.profileDiscoverHint) els.profileDiscoverHint.innerHTML = t.profileDiscoverHintHtml;
  if (els.profileDiscoverEmptyText) els.profileDiscoverEmptyText.textContent = t.profileDiscoverEmpty;
  if (els.profileDiscoverImportBtn) els.profileDiscoverImportBtn.textContent = t.profileDiscoverImport;
  if (els.profileDiscoverCancelBtn) els.profileDiscoverCancelBtn.textContent = t.cancel;
  if (els.skipActivateBtn) els.skipActivateBtn.textContent = t.skipActivate;
  if (els.listhdMoreBtn) els.listhdMoreBtn.title = t.menuMore;
  if (els.skillCreateBtn) els.skillCreateBtn.textContent = t.createSkill;
  if (els.mcpAddBtn) els.mcpAddBtn.textContent = t.addMcp;
  if (els.skillDiscoverBtn) els.skillDiscoverBtn.textContent = t.scanImport;
  if (els.skillImportBtn) els.skillImportBtn.textContent = t.importFolder;
  if (els.skillAdoptBtn) els.skillAdoptBtn.textContent = t.adoptFromScience;
  if (els.mcpDiscoverBtn) els.mcpDiscoverBtn.textContent = t.mcpDiscoverBtn;
  if (els.mcpNetworkAllowlistBtn) els.mcpNetworkAllowlistBtn.textContent = t.mcpNetworkAllowlist;
  if (els.mcpJsonBtn) els.mcpJsonBtn.textContent = t.mcpEditJson;
  if (els.skillMoreBtn) els.skillMoreBtn.title = t.menuMore;
  if (els.mcpMoreBtn) els.mcpMoreBtn.title = t.menuMore;
  const skillsTitle = document.querySelector("#skillListSec > .skill-hd > .sec-title");
  if (skillsTitle) skillsTitle.textContent = t.skillsManageTitle;
  if (els.skillApplyHint) els.skillApplyHint.textContent = t.skillApplyHint;
  if (els.skillEmptyTitle) els.skillEmptyTitle.textContent = t.skillEmptyTitle;
  if (els.skillEmptyHint) els.skillEmptyHint.textContent = t.skillEmptyHint;
  if (els.skillCreateTitle) els.skillCreateTitle.textContent = t.skillCreateTitle;
  if (els.skillCreateSaveBtn) els.skillCreateSaveBtn.textContent = t.save;
  if (els.skillCreateCancelBtn) els.skillCreateCancelBtn.textContent = t.cancel;
  if (els.skillDiscoverTitle) els.skillDiscoverTitle.textContent = t.skillDiscoverTitle;
  if (els.skillDiscoverHint) els.skillDiscoverHint.innerHTML = t.skillDiscoverHintHtml;
  if (els.skillDiscoverEmptyText) els.skillDiscoverEmptyText.textContent = t.skillDiscoverEmpty;
  if (els.skillDiscoverImportBtn) els.skillDiscoverImportBtn.textContent = t.skillDiscoverImport;
  if (els.skillDiscoverCancelBtn) els.skillDiscoverCancelBtn.textContent = t.cancel;
  if (els.skillImportTitle) els.skillImportTitle.textContent = t.skillImportTitle;
  if (els.skillImportIntro) els.skillImportIntro.innerHTML = t.skillImportIntroHtml;
  if (els.skillPathLabel) els.skillPathLabel.textContent = t.skillPathLabel;
  if (els.skillSourcePath) els.skillSourcePath.placeholder = t.skillPathPlaceholder;
  if (els.skillBrowseBtn) els.skillBrowseBtn.textContent = t.skillBrowse;
  if (els.skillPathHint) els.skillPathHint.innerHTML = t.skillPathHintHtml;
  if (els.skillInspPreviewTitle) els.skillInspPreviewTitle.textContent = t.skillInspPreviewTitle;
  if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.textContent = t.skillImportPath;
  if (els.skillImportCancelBtn) els.skillImportCancelBtn.textContent = t.cancel;
  if (els.skillAdoptTitle) els.skillAdoptTitle.textContent = t.skillAdoptTitle;
  if (els.skillAdoptHint) els.skillAdoptHint.innerHTML = t.skillAdoptHintHtml;
  if (els.skillAdoptEmptyText) els.skillAdoptEmptyText.textContent = t.skillAdoptEmpty;
  if (els.skillAdoptConfirmBtn) els.skillAdoptConfirmBtn.textContent = t.skillAdoptConfirm;
  if (els.skillAdoptCancelBtn) els.skillAdoptCancelBtn.textContent = t.cancel;
  if (els.previewOverlayCloseBtn) {
    els.previewOverlayCloseBtn.setAttribute("aria-label", t.skillAdoptPreviewClose);
    els.previewOverlayCloseBtn.title = t.skillAdoptPreviewClose;
  }
  if (els.previewOverlayOpenBtn) els.previewOverlayOpenBtn.textContent = t.skillAdoptOpen;
  const mcpTitle = document.querySelector("#mcpListSec > .skill-hd > .sec-title");
  if (mcpTitle) mcpTitle.textContent = t.mcpManageTitle;
  if (els.mcpApplyHint) els.mcpApplyHint.textContent = t.mcpApplyHint;
  if (els.mcpEmptyTitle) els.mcpEmptyTitle.textContent = t.mcpEmptyTitle;
  if (els.mcpEmptyHint) els.mcpEmptyHint.innerHTML = t.mcpEmptyHintHtml;
  if (els.mcpFormTitle) els.mcpFormTitle.textContent = t.mcpFormTitleNew;
  if (els.mcpTransportLabel) els.mcpTransportLabel.textContent = t.mcpTransportLabel;
  if (els.mcpTransportHint) els.mcpTransportHint.innerHTML = t.mcpTransportHintHtml;
  if (els.mcpTransport) {
    const opts = els.mcpTransport.options;
    if (opts[0]) opts[0].textContent = t.mcpTransportStdio;
    if (opts[1]) opts[1].textContent = t.mcpTransportHttp;
    if (opts[2]) opts[2].textContent = t.mcpTransportSse;
  }
  if (els.mcpNameHint) els.mcpNameHint.innerHTML = t.mcpNameHintHtml;
  if (els.mcpCommandLabel) els.mcpCommandLabel.textContent = t.mcpCommandLabel;
  if (els.mcpCommandHint) els.mcpCommandHint.innerHTML = t.mcpCommandHintHtml;
  if (els.mcpArgsLabel) els.mcpArgsLabel.textContent = t.mcpArgsLabel;
  if (els.mcpArgsHint) els.mcpArgsHint.innerHTML = t.mcpArgsHintHtml;
  if (els.mcpEnvLabel) els.mcpEnvLabel.textContent = t.mcpEnvLabel;
  if (els.mcpEnvHint) els.mcpEnvHint.innerHTML = t.mcpEnvHintHtml;
  if (els.mcpUrlLabel) els.mcpUrlLabel.textContent = t.mcpUrlLabel;
  if (els.mcpUrlHint) els.mcpUrlHint.innerHTML = t.mcpUrlHintHtml;
  if (els.mcpHeadersLabel) els.mcpHeadersLabel.textContent = t.mcpHeadersLabel;
  if (els.mcpHeadersHint) els.mcpHeadersHint.innerHTML = t.mcpHeadersHintHtml;
  if (els.mcpSaveBtn) els.mcpSaveBtn.textContent = t.save;
  if (els.mcpCancelBtn) els.mcpCancelBtn.textContent = t.cancel;
  if (els.mcpDiscoverTitle) els.mcpDiscoverTitle.textContent = t.mcpDiscoverTitle;
  if (els.mcpDiscoverHint) els.mcpDiscoverHint.innerHTML = t.mcpDiscoverHintHtml;
  if (els.mcpDiscoverEmptyText) els.mcpDiscoverEmptyText.textContent = t.mcpDiscoverEmpty;
  if (els.mcpDiscoverImportBtn) els.mcpDiscoverImportBtn.textContent = t.mcpDiscoverImport;
  if (els.mcpDiscoverCancelBtn) els.mcpDiscoverCancelBtn.textContent = t.cancel;
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
  schema_version: 5,
  active_id: "",
  active_mode: "profile",
  model_platter: { entries: [] },
  proxy_port: 18991,
  sandbox_port: 8990,
  profiles: [
    { id: "p-demo1", name: "GLM", template_id: "glm", api_format: "anthropic", base_url: "https://open.bigmodel.cn/api/anthropic", model: "glm-5.2", active_models: ["glm-5.2"], key: "••••••1234", icon: "glm", icon_color: "#2E6BE6", notes: "" },
  ],
  skills: [
    { id: "sk_1", name: "AlphaFold Database Fetch & Analyze", description: "Retrieve and analyze AlphaFold predicted structures for a protein.", enabled: true, sizeBytes: 12450, importedAt: "2026-07-12T02:30:00Z", requirements: ["python"] }
  ],
  mcpServers: [
    { id: "mcp_0000000000000000", name: "web-search", description: "Local web + literature search (no API key required). GENERAL csp_web_search → duckduckgo_ia/lite (wikipedia NOT on GENERAL); LITERATURE search_literature → wikipedia/Crossref/arXiv/PubMed. Optional BRAVE_SEARCH_API_KEY / SERPER_API_KEY / TAVILY_API_KEY improve quality only. Never call native Anthropic web_search.", transport: "stdio", command: "python3", args: ["/Users/me/.csp/sandbox/home/.claude-science/mcp/csp-web-search-server.py"], env: { BRAVE_SEARCH_API_KEY: "", SERPER_API_KEY: "", TAVILY_API_KEY: "" }, url: "", headers: {}, enabled: true, builtin: true, createdAt: "2026-07-12T02:30:00Z", updatedAt: "2026-07-12T02:30:00Z" },
    { id: "mcp_0000000000000001", name: "local-fs", description: "本地文件系统工具", transport: "stdio", command: "python3", args: ["/Users/me/mcp/fs_server.py"], env: { API_TOKEN: "••••1234" }, url: "", headers: {}, enabled: true, builtin: false, createdAt: "2026-07-12T02:30:00Z", updatedAt: "2026-07-12T02:30:00Z" }
  ],
};
function mockMask(k) { return k ? "••••" + String(k).slice(-4) : ""; }
function mockInvoke(cmd, args) {
  args = args || {};
  switch (cmd) {
    case "get_config":
      return Promise.resolve({
        schema_version: mockStore.schema_version, active_id: mockStore.active_id,
        active_mode: mockStore.active_mode,
        model_platter: mockStore.model_platter,
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
      if (args.activeModels != null) p.active_models = args.activeModels.slice();
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
    case "save_model_platter":
      mockStore.model_platter = { entries: (args.entries || []).map((e) => ({ ...e })) };
      return Promise.resolve({ proxy_reloaded: false });
    case "set_active_platter":
      if (!mockStore.model_platter || !mockStore.model_platter.entries.length) {
        return Promise.reject('{"i18n":"errPlatterEmpty","vars":{}}');
      }
      mockStore.active_id = PLATTER_ACTIVE_ID;
      mockStore.active_mode = "platter";
      return Promise.resolve({ committed: true, active_id: PLATTER_ACTIVE_ID });
    case "discover_editor_llm_providers":
      return Promise.resolve([
        {
          id: "Zed|Demo-Moon",
          name: "Demo-Moon",
          sourceLabel: "Zed",
          sourcePath: "/tmp/zed-settings.json",
          apiUrl: "https://api.moonshot.cn/v1",
          models: ["kimi-k2", "kimi-k2-turbo"],
          alreadyImported: false,
          hasKey: false,
          needsKey: true,
        },
        {
          id: "Continue|Moonshot Chat",
          name: "Moonshot Chat",
          sourceLabel: "Continue",
          sourcePath: "/tmp/.continue/config.yaml",
          apiUrl: "https://api.moonshot.cn/v1",
          models: ["kimi-k2"],
          alreadyImported: false,
          hasKey: true,
          needsKey: false,
        },
      ]);
    case "preview_discovered_editor_llm":
      return Promise.resolve({
        name: args.name,
        sourceLabel: "Zed",
        sourcePath: args.sourcePath,
        config: { api_url: "https://api.moonshot.cn/v1", available_models: [{ name: "kimi-k2" }] },
      });
    case "import_discovered_editor_llm": {
      const id = "p-" + Math.random().toString(16).slice(2, 10);
      mockStore.profiles.push({
        id,
        name: args.name || "Imported",
        template_id: "custom-openai",
        api_format: "openai_chat",
        base_url: "https://api.moonshot.cn/v1",
        model: "kimi-k2",
        active_models: ["kimi-k2"],
        key: "",
        icon: "openai",
        icon_color: "#10A37F",
        notes: "",
      });
      return Promise.resolve({ skipped: false, id, needsKey: true, hasKey: false });
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
    case "discover_science_skill_sync":
      return Promise.resolve([
        {
          key: "science://harvest/sk_1",
          kind: "harvest",
          name: "crypto-data-pro",
          description: "Improved draft",
          skillId: "sk_1",
          files: ["SKILL.md", "kernel.py"],
          warnings: ["Science library is newer/larger than CSP store"],
          storeBytes: 2310,
          scienceBytes: 16790,
          alreadyImported: true,
        },
      ]);
    case "preview_science_skill": {
      const key = (args.input && args.input.key) || "";
      const file = (args.input && args.input.file) || "SKILL.md";
      return Promise.resolve({
        key,
        name: "crypto-data-pro",
        description: "Improved",
        workspaceId: "",
        alreadyImported: true,
        openPath: "/preview/science/crypto-data-pro",
        files: [
          { name: "SKILL.md", sizeBytes: 16790 },
          { name: "kernel.py", sizeBytes: 39594 },
        ],
        activeFile: file,
        content: "# Improved preview mock\n",
        truncated: false,
        charCount: 24,
      });
    }
    case "open_science_skill":
      return Promise.resolve("/preview/science/" + ((args.input && args.input.key) || ""));
    case "sync_science_skills": {
      const keys = (args.input && args.input.keys) || [];
      const synced = keys.map((key, i) => ({
        id: "sk_" + Math.random().toString(16).slice(2, 10) + i,
        name: "Synced Skill",
        description: "From " + key,
        enabled: true,
        sizeBytes: 16000,
        importedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        requirements: ["python"],
      }));
      synced.forEach((s) => mockStore.skills.push(s));
      return Promise.resolve({ synced, failures: [], needsRestart: false });
    }
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
    case "preview_workspace_skill": {
      const key = (args.input && args.input.key) || "";
      const file = (args.input && args.input.file) || "SKILL.md";
      return Promise.resolve({
        key,
        name: "crypto-data-v2",
        description: "Enhanced crypto skill draft",
        workspaceId: "ws1",
        alreadyImported: false,
        openPath: "/preview/workspace/crypto-data-v2",
        files: [
          { name: "SKILL.md", sizeBytes: 1878 },
          { name: "kernel.py", sizeBytes: 39514 },
        ],
        activeFile: file,
        content: "---\nname: crypto-data-v2\ndescription: preview mock\n---\n\n# Preview\nMock SKILL.md body for browser preview.",
        truncated: false,
        charCount: 96,
      });
    }
    case "open_workspace_skill":
      return Promise.resolve("/preview/workspace/" + ((args.input && args.input.key) || ""));
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
    case "pick_skill_source":
      return Promise.resolve("/Users/me/.agents/skills/demo-skill");
    case "inspect_skill_source": {
      const source = (args.input && args.input.source) || (args.input && args.input.sourcePath) || "";
      const valid = source.length > 0;
      if (!valid) {
        return Promise.reject("Invalid source supplied");
      }
      return Promise.resolve({
        valid,
        name: "Local Skill",
        description: "Inspected skill from: " + source,
        fileCount: 3,
        totalSizeBytes: 89000,
        requirements: ["python", "mcp"],
        warnings: [],
        errors: [],
        importPath: source.startsWith("http") ? "/tmp/mock-extracted-skill" : source,
        logicalSource: source,
      });
    }
    case "import_skill": {
      const source = (args.input && args.input.source) || (args.input && args.input.sourcePath) || "";
      const id = "sk_" + Math.random().toString(16).slice(2, 10);
      const newSkill = {
        id,
        name: "Imported Skill",
        description: "Skill imported from: " + source,
        storePath: "/mock/store/" + id,
        sourcePath: source,
        enabled: true,
        sizeBytes: 89000,
        importedAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
        requirements: ["python", "mcp"],
      };
      mockStore.skills.push(newSkill);
      return Promise.resolve({ skill: newSkill, needsRestart: false });
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
      return Promise.resolve({ skill: s, needsRestart: false });
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
    case "preview_discovered_mcp": {
      const input = args.input || {};
      const name = input.name || "notion-mcp-v1";
      return Promise.resolve({
        name,
        sourcePath: input.sourcePath || "/Users/me/.cursor/mcp.json",
        content: JSON.stringify(
          {
            command: "/Users/me/.local/node/bin/notion-mcp",
            args: ["--transport", "stdio"],
            env: { NOTION_TOKEN: "••••" },
          },
          null,
          2
        ),
        truncated: false,
        charCount: 120,
      });
    }
    case "open_discovered_mcp_source":
      return Promise.resolve((args.input && args.input.sourcePath) || "/preview/mcp.json");
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
      return Promise.resolve({ server: { ...srv }, needsRestart: false });
    }
    case "inspect_mcp_server": {
      const inp = args.input || {};
      const errors = [];
      const transport = inp.transport || "stdio";
      const remote = transport === "sse" || transport === "streamable_http";
      if (!inp.name) errors.push("Name is required");
      if (remote) {
        if (!inp.url) errors.push("URL is required for remote MCP");
      } else if (!inp.command) {
        errors.push("Command is required");
      }
      const known = ["node", "npm", "npx", "python", "python3", "pip", "pip3", "uv", "uvx", "deno", "bun", "bunx"];
      const commandOk = remote || (!!inp.command && (inp.command.startsWith("/") || known.includes(inp.command)));
      const warnings = [];
      if (!remote && inp.command && !commandOk) warnings.push(`'${inp.command}' 不在受管运行时白名单，Science 可能拒绝。`);
      if (remote && inp.headers && Object.keys(inp.headers).length) {
        warnings.push("Header values are stored in CSP's local 0600 inventory and deployed as a Science headers_helper.");
      }
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
      const headers = {};
      Object.entries(inp.headers || {}).forEach(([k, v]) => (headers[k] = mockMask(v)));
      const srv = {
        id,
        name: inp.name,
        description: inp.description || "",
        transport: inp.transport || "stdio",
        command: inp.command || "",
        args: inp.args || [],
        env,
        url: inp.url || "",
        headers,
        enabled: true,
        createdAt: now,
        updatedAt: now,
      };
      mockStore.mcpServers.push(srv);
      return Promise.resolve({ server: srv, needsRestart: false });
    }
    case "update_mcp_server": {
      const s = mockStore.mcpServers.find((x) => x.id === args.input.serverId);
      if (!s) return Promise.reject("MCP server not found");
      const inp = args.input.server || {};
      s.name = inp.name;
      s.description = inp.description || "";
      s.transport = inp.transport || "stdio";
      s.command = inp.command || "";
      s.args = inp.args || [];
      s.url = inp.url || "";
      // Mirror backend env-merge: empty value keeps old, absent key deletes.
      const env = {};
      Object.entries(inp.env || {}).forEach(([k, v]) => {
        if (v === "") { if (s.env && s.env[k] != null) env[k] = s.env[k]; }
        else env[k] = mockMask(v);
      });
      s.env = env;
      const headers = {};
      Object.entries(inp.headers || {}).forEach(([k, v]) => {
        if (v === "") { if (s.headers && s.headers[k] != null) headers[k] = s.headers[k]; }
        else headers[k] = mockMask(v);
      });
      s.headers = headers;
      s.updatedAt = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
      return Promise.resolve({ server: s, needsRestart: false });
    }
    case "set_mcp_server_enabled": {
      const s = mockStore.mcpServers.find((x) => x.id === args.input.serverId);
      if (s) s.enabled = !!args.input.enabled;
      return Promise.resolve({ server: s, needsRestart: false });
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
let configState = { profiles: [], templates: [], active_id: "", active_mode: "profile", model_platter: { entries: [] }, proxy_port: 18991, sandbox_port: 8990 };
let platterDraft = [];
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

const MODEL_PICK_SEARCH_MIN = 9;

function renderModelPick(container, builtin, selected, onChange, opts) {
  if (!container) return;
  const max = (opts && opts.max) || 0;
  const sel = (selected || []).filter(Boolean);
  // Enabled models pinned on top in their configured order, then the rest of the pool.
  const candidates = [];
  for (const id of sel) if (!candidates.includes(id)) candidates.push(id);
  for (const id of builtin || []) if (!candidates.includes(id)) candidates.push(id);
  if (!candidates.length) {
    container.hidden = true;
    container.innerHTML = "";
    return;
  }
  container.hidden = false;
  const selSet = new Set(sel.length ? sel : candidates);
  // Preserve an in-progress filter across re-renders (e.g. after model fetch).
  const prevSearch = container.querySelector(".model-pick-search input");
  const prevFilter = prevSearch ? prevSearch.value : "";
  const searchHtml = candidates.length >= MODEL_PICK_SEARCH_MIN
    ? '<div class="model-pick-search"><input type="search" placeholder="' +
      escapeHtml(S().modelPickSearch) + '" value="' + escapeHtml(prevFilter) + '">' +
      '<span class="model-pick-search-empty" hidden>' + escapeHtml(S().modelPickSearchEmpty) + "</span></div>"
    : "";
  container.innerHTML = searchHtml + candidates.map((id) => {
    const checked = selSet.has(id) ? " checked" : "";
    return '<label class="model-pick-item"><input type="checkbox" data-model="' +
      escapeHtml(id) + '"' + checked + '><span class="model-pick-label">' + escapeHtml(id) + "</span></label>";
  }).join("");
  container.querySelectorAll('input[type="checkbox"]').forEach((cb) => {
    cb.addEventListener("change", () => {
      if (max > 0 && cb.checked) {
        const n = container.querySelectorAll('input[type="checkbox"]:checked').length;
        if (n > max) {
          cb.checked = false;
          setMsg(T("errScienceModelCap", { max }));
          return;
        }
      }
      if (onChange) onChange();
    });
  });
  const search = container.querySelector(".model-pick-search input");
  if (search) {
    const applyFilter = () => {
      const q = search.value.trim().toLowerCase();
      let visible = 0;
      container.querySelectorAll(".model-pick-item").forEach((item) => {
        const cb = item.querySelector("input[data-model]");
        const id = (cb && cb.getAttribute("data-model")) || "";
        const show = !q || id.toLowerCase().includes(q);
        item.hidden = !show;
        if (show) visible += 1;
      });
      const empty = container.querySelector(".model-pick-search-empty");
      if (empty) empty.hidden = visible > 0;
    };
    search.addEventListener("input", applyFilter);
    if (prevFilter) applyFilter();
  }
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
    renderModelPick(ui.pick, builtin, selected, onPickChange, { max: MAX_SCIENCE_MODELS });
  }
  return cap;
}

function setMsg(text, kind = "err") {
  const t = text ? String(text) : "";
  els.msg.textContent = t;
  els.msg.className = "msg" + (t ? ` ${kind}` : "");
  els.msg.hidden = !t;
  // Keep the section visible while the "activate anyway" escape hatch is shown.
  els.msg.parentElement.hidden = !t && els.skipActivateBtn.hidden;
  // Only yank the viewport for errors — info/ok toasts must not jump the form
  // (e.g. platter catalog picks already show their result in the selected list).
  if (t && kind === "err" && els.panel && els.panel.classList.contains("view-form")) {
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
  // Resolve the actual scroll container the button lives in — the element with
  // `overflow-y: auto` that clips the menu. Skill/MCP rows scroll inside
  // `.skill-list`; Profile rows scroll inside `#profileList` (NOT `.panel-body`,
  // whose bottom extends past the list into the feedback area, so measuring
  // against it under-reports clipping and bottom cards fail to flip the menu up).
  // Using a stale `els.panelBody` (the hidden Profiles pane) yielded a zero-height
  // rect on other tabs, forcing the menu to wrongly flip up and hide under the header.
  const scrollEl =
    btn.closest(".skill-list") ||
    btn.closest("#profileList") ||
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
    els.profileDiscoverBtn, els.profileDiscoverImportBtn, els.profileDiscoverCancelBtn,
    // Disable port inputs while busy: changing ports mid-operation races in-flight work (P1-c frontend).
    els.proxyPort, els.sandboxPort,
    // Skill / MCP manager actions: prevent concurrent mutations racing a running op.
    els.skillCreateBtn, els.skillCreateSaveBtn, els.skillCreateCancelBtn,
    els.skillMoreBtn, els.skillBrowseBtn, els.skillImportConfirmBtn, els.skillImportCancelBtn,
    els.skillImportBtn, els.skillDiscoverBtn, els.skillDiscoverImportBtn, els.skillDiscoverCancelBtn,
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

function runtimeStatusKind() {
  if (runtimePhase === "starting") return "starting";
  if (runtimePhase === "stopping") return "stopping";
  const proxyOn = lastRuntimeLights.proxy === "green";
  const scienceOn = lastRuntimeLights.sandbox === "green";
  if (proxyOn && scienceOn) return "both";
  if (proxyOn) return "proxy";
  if (scienceOn) return "science";
  return "off";
}

function runtimeStatusText(kind) {
  const t = S();
  switch (kind || runtimeStatusKind()) {
    case "starting": return t.runStatusStarting;
    case "stopping": return t.runStatusStopping;
    case "both": return t.runStatusBoth;
    case "proxy": return t.runStatusProxy;
    case "science": return t.runStatusScience;
    default: return t.runStatusOff;
  }
}

function runtimeStatusTip(kind) {
  const t = S();
  switch (kind || runtimeStatusKind()) {
    case "starting": return t.runStatusTipStarting;
    case "stopping": return t.runStatusTipStopping;
    case "both": return t.runStatusTipBoth;
    case "proxy": return t.runStatusTipProxy;
    case "science": return t.runStatusTipScience;
    default: return t.runStatusTipOff;
  }
}

function updateRuntimeStatusUI() {
  if (!els.runtimeStatusText) return;
  const kind = runtimeStatusKind();
  const text = runtimeStatusText(kind);
  const tip = runtimeStatusTip(kind);
  els.runtimeStatusText.textContent = text;
  els.runtimeStatusText.title = tip;
  els.runtimeStatusText.setAttribute("aria-label", `${text}. ${tip}`);
  const busyPhase = kind === "starting" || kind === "stopping";
  const ready = kind === "both";
  const warn = kind === "proxy" || kind === "science";
  els.runtimeStatusText.classList.toggle("is-busy", busyPhase);
  els.runtimeStatusText.classList.toggle("is-ready", ready);
  els.runtimeStatusText.classList.toggle("is-warn", warn);
  els.runtimeStatusText.classList.remove("is-running");
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

/** Collapse whitespace and cap length for one-line discover card details. */
function truncateOneLine(s, max) {
  const t = String(s == null ? "" : s).replace(/\s+/g, " ").trim();
  if (!max || t.length <= max) return t;
  return `${t.slice(0, Math.max(0, max - 1))}…`;
}

function tplById(id) {
  return (configState.templates || []).find((t) => t.id === id) || null;
}

// Shared footer (runtime status + ports): visible on any tab's list view, hidden in wizard/forms.
function updateSharedRuntimeFooter() {
  const show = !els.panel.classList.contains("view-form");
  if (els.runtimeStatusSec) els.runtimeStatusSec.hidden = !show;
  if (els.advSec) els.advSec.hidden = !show;
}

// ── View switching: list / new wizard / connection edit. One form visible at a time (list hidden to reduce height). ──
function showView(v) {
  els.listSec.hidden = v !== "list";
  els.wizSec.hidden = v !== "wizard";
  els.connSec.hidden = v !== "conn";
  if (els.platterSec) els.platterSec.hidden = v !== "platter";
  if (els.profileDiscoverSec) els.profileDiscoverSec.hidden = v !== "discover";
  // Only drive the panel form chrome when the profiles pane is active.
  if (!els.panelBody || !els.panelBody.hidden) {
    els.panel.classList.toggle("view-form", v !== "list");
  }
  updateSharedRuntimeFooter();
  // Drop stale feedback on every view change (e.g. a switch-verify error
  // raised on the list must not linger on the discover/wizard/platter views).
  hideSkip();
  setMsg("");
  if (v === "list") closeWizPresetMenu();
}
function cancelForm() { showView("list"); }

async function openProfileDiscover() {
  closeListhdMenu();
  if (busy) return;
  closePreviewOverlay();
  if (els.profileDiscoverList) {
    els.profileDiscoverList.innerHTML = `<p class="hint">${escapeHtml(S().profileDiscoverScanning)}</p>`;
  }
  if (els.profileDiscoverEmpty) els.profileDiscoverEmpty.hidden = true;
  if (els.profileDiscoverImportBtn) els.profileDiscoverImportBtn.disabled = true;
  showView("discover");
  try {
    const found = (await call("discover_editor_llm_providers")) || [];
    renderProfileDiscover(found);
  } catch (e) {
    if (els.profileDiscoverList) els.profileDiscoverList.innerHTML = "";
    if (els.profileDiscoverEmpty) els.profileDiscoverEmpty.hidden = false;
    setMsg(resolveBackendErr(e));
  }
}

function closeProfileDiscover() {
  closePreviewOverlay();
  showView("list");
}

function renderProfileDiscover(found) {
  if (!els.profileDiscoverList) return;
  if (!found.length) {
    els.profileDiscoverList.innerHTML = "";
    if (els.profileDiscoverEmpty) els.profileDiscoverEmpty.hidden = false;
    refreshProfileDiscoverGate();
    return;
  }
  if (els.profileDiscoverEmpty) els.profileDiscoverEmpty.hidden = true;
  const importedBadge = S().skillAlreadyImported;
  const needsKeyBadge = S().profileDiscoverNeedsKey;
  const previewLabel = S().profileDiscoverPreview;
  els.profileDiscoverList.innerHTML = found.map((d) => {
    const disabled = d.alreadyImported ? " disabled" : "";
    let badge = "";
    if (d.alreadyImported) {
      badge = `<span class="skill-req-tag">${escapeHtml(importedBadge)}</span>`;
    } else if (d.needsKey) {
      badge = `<span class="skill-req-tag">${escapeHtml(needsKeyBadge)}</span>`;
    } else if (d.keySource === "env") {
      badge = `<span class="skill-req-tag">${escapeHtml(S().profileDiscoverKeyEnv)}</span>`;
    } else if (d.keySource === "config") {
      badge = `<span class="skill-req-tag">${escapeHtml(S().profileDiscoverKeyConfig)}</span>`;
    } else if (d.keySource === "keychain") {
      badge = `<span class="skill-req-tag">${escapeHtml(S().profileDiscoverKeyKeychain)}</span>`;
    }
    const models = T("profileDiscoverModelCount", { n: (d.models || []).length });
    const detail = escapeHtml(truncateOneLine(d.apiUrl || "", 96));
    return `
      <label class="skill-discover-row${d.alreadyImported ? " disabled" : ""}">
        <input type="checkbox" data-source-path="${escapeHtml(d.sourcePath)}" value="${escapeHtml(d.name)}"${disabled} />
        <span class="skill-discover-main">
          <span class="skill-name skill-name-flex"><span class="skill-name-text">${escapeHtml(d.name)}</span>${badge}</span>
          ${detail ? `<span class="mcp-cmd">${detail}</span>` : ""}
          <span class="skill-meta"><span>${escapeHtml(d.sourceLabel)}</span><span>${escapeHtml(models)}</span></span>
        </span>
        <span class="skill-discover-actions">
          <button type="button" class="btn small"
            data-llm-preview-source="${escapeHtml(d.sourcePath)}"
            data-llm-preview-name="${escapeHtml(d.name)}"
            data-llm-preview-label="${escapeHtml(d.sourceLabel)}">${escapeHtml(previewLabel)}</button>
        </span>
      </label>
    `;
  }).join("");
  refreshProfileDiscoverGate();
}

function refreshProfileDiscoverGate() {
  if (!els.profileDiscoverImportBtn || !els.profileDiscoverList) return;
  const n = els.profileDiscoverList.querySelectorAll("input[type=checkbox]:checked:not(:disabled)").length;
  els.profileDiscoverImportBtn.disabled = n === 0 || busy;
}

async function loadProfileDiscoverPreview(sourcePath, name, sourceLabel) {
  if (!sourcePath || !name || busy) return;
  showPreviewOverlayLoading(S().profileDiscoverPreview, "compact");
  previewOverlayState = { kind: "editor-llm", sourcePath, name };
  if (els.previewOverlayOpenBtn) els.previewOverlayOpenBtn.hidden = true;
  try {
    const data = await call("preview_discovered_editor_llm", { sourcePath, name });
    if (els.previewOverlayTitle) {
      els.previewOverlayTitle.textContent = `${S().profileDiscoverPreview} · ${data.name || name}`;
    }
    if (els.previewOverlayMeta) {
      els.previewOverlayMeta.textContent = `${data.name || name} · ${sourceLabel || data.sourceLabel || ""}`;
    }
    const body = JSON.stringify(data.config || data, null, 2);
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = body;
  } catch (e) {
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = resolveBackendErr(e);
  }
}

async function importDiscoveredEditorLlms() {
  if (!els.profileDiscoverList || busy) return;
  const boxes = [...els.profileDiscoverList.querySelectorAll("input[type=checkbox]:checked:not(:disabled)")];
  if (!boxes.length) return;
  setBusy(true, { kind: "importEditorLlm" });
  let ok = 0;
  let skip = 0;
  let needsKey = false;
  const failed = [];
  try {
    // Per-item errors must not abort the batch (an early failure would silently
    // drop the remaining selections).
    for (const box of boxes) {
      try {
        const res = await call("import_discovered_editor_llm", {
          sourcePath: box.getAttribute("data-source-path"),
          name: box.value,
        });
        if (res && res.skipped) skip += 1;
        else {
          ok += 1;
          if (res && res.needsKey) needsKey = true;
        }
      } catch (e) {
        failed.push(`${box.value}: ${resolveBackendErr(e)}`);
      }
    }
    await loadConfig({ keepView: true });
    let msg = "";
    if (ok && skip) msg = T("profileDiscoverImportOk", { ok, skip });
    else if (ok) msg = T("profileDiscoverImportOkOnly", { ok });
    else if (skip) msg = T("profileDiscoverImportSkipOnly", { skip });
    if (needsKey) msg = (msg ? msg + " " : "") + S().profileDiscoverNeedsKeyHint;
    if (failed.length) {
      msg = (msg ? msg + " " : "") + T("profileDiscoverImportFail", { n: failed.length, err: failed[0] });
    }
    // Order matters: showView clears stale feedback, so set the message after.
    showView("list");
    // Green check for a clean import; red only when something actually failed.
    setMsg(msg, failed.length ? "err" : "ok");
  } finally {
    setBusy(false);
  }
}

// Skill tab: list / create / import / discover / adopt (full-page panels, same chrome as config wizard).
function showSkillView(v) {
  const list = v === "list";
  if (els.skillListSec) els.skillListSec.hidden = !list;
  if (els.skillCreateSec) els.skillCreateSec.hidden = v !== "create";
  if (els.skillImportSec) els.skillImportSec.hidden = v !== "import";
  if (els.skillDiscoverSec) els.skillDiscoverSec.hidden = v !== "discover";
  if (els.skillAdoptSec) els.skillAdoptSec.hidden = v !== "adopt";
  if (els.skillPane) {
    els.skillPane.classList.toggle("pane-form", !list);
    if (!els.skillPane.hidden) els.panel.classList.toggle("view-form", !list);
  }
  updateSharedRuntimeFooter();
  // Drop stale feedback on every sub-view change (same rationale as showView).
  setSkillMsg("");
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
  updateSharedRuntimeFooter();
  setMcpMsg("");
  if (list) closeMenu(els.mcpMenu, els.mcpMoreBtn);
}

function showSkip() {
  els.skipActivateBtn.hidden = false;
  els.msg.parentElement.hidden = false;
}
function hideSkip() {
  els.skipActivateBtn.hidden = true;
  pendingSkipActivateId = null;
  if (!els.msg.textContent) els.msg.parentElement.hidden = true;
}

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
    configState.active_mode = cfg.active_mode || "profile";
    configState.model_platter = cfg.model_platter || { entries: [] };
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

function isPlatterActive() {
  return configState.active_id === PLATTER_ACTIVE_ID;
}

function platterEntries() {
  return (configState.model_platter && configState.model_platter.entries) || [];
}

function platterModelSelected(profileId, model) {
  return platterDraft.some((e) => e.profile_id === profileId && e.model === model);
}

function profilePlatterSupported(p) {
  return !!(p && (typeof p.has_key === "boolean" ? p.has_key : !!p.key));
}

/** Models offered in the platter picker for a profile (upstream real ids). */
function platterCatalogModels(p) {
  const t = tplById(p.template_id);
  const adapter = (t && t.adapter) || "";
  if (adapter === "deepseek") {
    return ["deepseek-v4-pro", "deepseek-v4-flash"];
  }
  const fromProfile = profileModels(p);
  if (fromProfile.length) return fromProfile;
  return ((t && t.builtin_models) || []).slice();
}

function renderPlatterCardHtml() {
  const t = S();
  const active = isPlatterActive();
  const entries = platterEntries();
  const n = entries.length;
  const meta = !n
    ? escapeHtml(t.platterEmptyMeta)
    : escapeHtml(T("platterMeta", { n, providers: new Set(entries.map((e) => e.profile_id)).size }));
  return (
    '<div class="prow platter-row' + (active ? " pactive" : "") + '" data-id="' + PLATTER_ACTIVE_ID + '">' +
      '<div class="prow-top">' +
        '<span class="pico platter-ico" aria-hidden="true">⊞</span>' +
        '<span class="pname">' + escapeHtml(t.platterCardTitle) + "</span>" +
        (active ? '<span class="badge on">' + escapeHtml(S().activeBadge) + "</span>" : "") +
      "</div>" +
      '<div class="pmeta">' + meta + "</div>" +
      '<div class="prow-acts">' +
        '<div class="pmenu-wrap">' +
          '<button type="button" class="abtn pmenu-btn" data-act="menu" aria-haspopup="true" aria-expanded="false" title="' + escapeHtml(S().menuMore) + '">⋯</button>' +
          '<div class="pmenu" hidden role="menu">' +
            '<button type="button" class="pmenu-item" data-act="editplatter" role="menuitem">' + escapeHtml(t.platterEdit) + "</button>" +
          "</div>" +
        "</div>" +
      "</div>" +
    "</div>"
  );
}

function renderPlatterSelected() {
  if (!els.platterSelectedList) return;
  const t = S();
  if (!platterDraft.length) {
    els.platterSelectedList.innerHTML = '<p class="hint">' + escapeHtml(t.platterEmptyMeta) + "</p>";
  } else {
    els.platterSelectedList.innerHTML = platterDraft.map((e, idx) => {
      const p = (configState.profiles || []).find((x) => x.id === e.profile_id);
      const pname = p ? p.name : e.profile_id;
      const def = idx === 0 ? (' <span class="badge">' + escapeHtml(t.platterDefaultBadge) + "</span>") : "";
      return (
        '<div class="platter-sel-item" data-idx="' + idx + '">' +
          '<span class="platter-sel-label">' + escapeHtml(pname + " · " + e.model) + def + "</span>" +
          '<span class="platter-sel-acts">' +
            '<button type="button" class="btn tiny" data-platter-up="' + idx + '" ' + (idx === 0 ? "disabled" : "") + ">↑</button>" +
            '<button type="button" class="btn tiny" data-platter-down="' + idx + '" ' + (idx === platterDraft.length - 1 ? "disabled" : "") + ">↓</button>" +
            '<button type="button" class="btn tiny" data-platter-rm="' + idx + '">×</button>' +
          "</span>" +
        "</div>"
      );
    }).join("");
  }
  if (els.platterCapHint) {
    els.platterCapHint.textContent = T("platterCapHint", { n: platterDraft.length, max: MAX_SCIENCE_MODELS });
  }
}

// Per-profile "browse all models" catalog state for the platter picker.
// pid → { status: "idle"|"loading"|"ok"|"err", ids: [], errText: "" }
let platterCatalogState = {};

function platterCatalogFor(pid) {
  return platterCatalogState[pid] || { status: "idle", ids: [], errText: "" };
}

function platterPickItemHtml(pid, model, opts) {
  const checked = platterModelSelected(pid, model) ? " checked" : "";
  const catalog = opts && opts.catalog;
  return '<label class="model-pick-item' + (catalog ? " model-pick-catalog" : "") +
    '"><input type="checkbox" data-platter-profile="' + escapeHtml(pid) +
    '" data-platter-model="' + escapeHtml(model) + '"' + (catalog ? ' data-platter-from-catalog="1"' : "") +
    checked + '><span class="model-pick-label">' + escapeHtml(model) + "</span></label>";
}

function renderPlatterProviders() {
  if (!els.platterProviderList) return;
  const box = els.platterProviderList;
  const t = S();
  const scrollTop = box.scrollTop;
  const ps = (configState.profiles || []).filter((p) => profilePlatterSupported(p) && (p.has_key !== false));
  if (!ps.length) {
    box.innerHTML = '<p class="hint">' + escapeHtml(S().emptyHint) + "</p>";
    return;
  }
  // Preserve an in-progress filter across re-renders (catalog load, model enable).
  const prevSearch = box.querySelector(".model-pick-search input");
  const prevFilter = prevSearch ? prevSearch.value : "";
  const blocks = ps.map((p) => {
    const enabled = platterCatalogModels(p);
    const cat = platterCatalogFor(p.id);
    // Catalog extras: models present upstream but not yet enabled on the profile.
    // Draft picks outside the enabled list (catalog/manual adds that could not be
    // synced) must render too, or they would be invisible and unsearchable here.
    const extras = [];
    for (const e of platterDraft) {
      if (e.profile_id === p.id && !enabled.includes(e.model)) extras.push(e.model);
    }
    if (cat.status === "ok") {
      for (const m of cat.ids) {
        if (!enabled.includes(m) && !extras.includes(m)) extras.push(m);
      }
    }
    if (!enabled.length && !extras.length && cat.status === "idle") return "";
    const boxes = enabled.map((m) => platterPickItemHtml(p.id, m)).join("") +
      extras.map((m) => platterPickItemHtml(p.id, m, { catalog: true })).join("");
    let foot = "";
    if (cat.status === "idle") {
      foot = '<button type="button" class="btn tiny platter-browse-btn" data-platter-browse="' + escapeHtml(p.id) + '">' +
        escapeHtml(t.platterBrowseAll) + "</button>";
    } else if (cat.status === "loading") {
      foot = '<span class="hint platter-catalog-hint">' + escapeHtml(t.platterCatalogLoading) + "</span>";
    } else if (cat.status === "ok" && !extras.length) {
      foot = '<span class="hint platter-catalog-hint">' + escapeHtml(t.platterCatalogEmpty) + "</span>";
    } else if (cat.status === "err") {
      foot = '<span class="hint platter-catalog-err">' + escapeHtml(cat.errText) + "</span>" +
        '<div class="platter-manual"><input type="text" data-platter-manual="' + escapeHtml(p.id) +
        '" placeholder="' + escapeHtml(t.platterManualPlaceholder) + '" autocomplete="off" spellcheck="false">' +
        '<button type="button" class="btn tiny" data-platter-manual-add="' + escapeHtml(p.id) + '">' +
        escapeHtml(t.platterManualAdd) + "</button></div>";
    }
    return '<div class="platter-provider-block" data-platter-block="' + escapeHtml(p.id) + '">' +
      '<div class="platter-provider-name">' + escapeHtml(p.name) + "</div>" +
      '<div class="model-pick">' + boxes + "</div>" + foot + "</div>";
  }).join("");
  // Always show search here: besides filtering, typing pulls the providers'
  // full catalogs, so it must be reachable even with only a few enabled models.
  const searchHtml =
    '<div class="model-pick-search"><input type="search" placeholder="' +
    escapeHtml(t.modelPickSearch) + '" value="' + escapeHtml(prevFilter) + '">' +
    '<span class="model-pick-search-empty" hidden>' + escapeHtml(t.modelPickSearchEmpty) + "</span></div>";
  box.innerHTML = searchHtml + blocks;
  box.querySelectorAll("input[type=checkbox]").forEach((cb) => {
    cb.addEventListener("change", () => {
      const pid = cb.getAttribute("data-platter-profile");
      const model = cb.getAttribute("data-platter-model");
      if (!cb.checked) {
        platterDraft = platterDraft.filter((e) => !(e.profile_id === pid && e.model === model));
        renderPlatterSelected();
        renderPlatterProviders();
        return;
      }
      if (platterDraft.length >= MAX_SCIENCE_MODELS) {
        cb.checked = false;
        setMsg(T("errScienceModelCap", { max: MAX_SCIENCE_MODELS }));
        return;
      }
      // Catalog picks and enabled models behave the same: selection only touches
      // the platter draft, never the profile's own enabled list (providers stay
      // independent).
      platterDraft.push({ profile_id: pid, model });
      renderPlatterSelected();
      renderPlatterProviders();
    });
  });
  box.querySelectorAll("[data-platter-browse]").forEach((btn) => {
    btn.addEventListener("click", () => browsePlatterCatalog(btn.getAttribute("data-platter-browse")));
  });
  box.querySelectorAll("[data-platter-manual-add]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const pid = btn.getAttribute("data-platter-manual-add");
      const input = box.querySelector('input[data-platter-manual="' + pid.replace(/"/g, '\\"') + '"]');
      const model = input ? input.value.trim() : "";
      if (model) enablePlatterCatalogModel(pid, model);
    });
  });
  box.querySelectorAll("input[data-platter-manual]").forEach((input) => {
    input.addEventListener("keydown", (ev) => {
      if (ev.key !== "Enter") return;
      ev.preventDefault();
      const model = input.value.trim();
      if (model) enablePlatterCatalogModel(input.getAttribute("data-platter-manual"), model);
    });
  });
  const search = box.querySelector(".model-pick-search input");
  if (search) {
    const applyFilter = () => {
      const q = search.value.trim().toLowerCase();
      // Search must reach models that are not enabled yet: the first query
      // lazily pulls every provider's full catalog (cached per editor session),
      // so picking doesn't require browsing each provider by hand.
      if (q) {
        const idle = ps.filter((p) => platterCatalogFor(p.id).status === "idle");
        if (idle.length) {
          idle.forEach((p) => browsePlatterCatalog(p.id));
          return; // each load re-renders; the preserved query re-applies the filter
        }
      }
      let visible = 0;
      box.querySelectorAll(".platter-provider-block").forEach((block) => {
        let blockVisible = 0;
        block.querySelectorAll(".model-pick-item").forEach((item) => {
          const cbEl = item.querySelector("input[data-platter-model]");
          const id = (cbEl && cbEl.getAttribute("data-platter-model")) || "";
          const show = !q || id.toLowerCase().includes(q);
          item.hidden = !show;
          if (show) blockVisible += 1;
        });
        // While filtering, collapse groups with no match (footer entries stay in matched groups).
        block.hidden = !!q && blockVisible === 0;
        visible += blockVisible;
      });
      const empty = box.querySelector(".model-pick-search-empty");
      if (empty) {
        const loading = q && ps.some((p) => platterCatalogFor(p.id).status === "loading");
        empty.textContent = loading ? t.platterCatalogLoading : t.modelPickSearchEmpty;
        empty.hidden = visible > 0;
      }
    };
    search.addEventListener("input", applyFilter);
    // Catalog loads re-render mid-typing; give focus (and the caret) back so
    // the user keeps typing without clicking into the box again.
    if (prevSearch && document.activeElement === prevSearch) {
      search.focus();
      const end = search.value.length;
      try { search.setSelectionRange(end, end); } catch (_) { /* type=search quirk */ }
    }
    if (prevFilter) applyFilter();
  }
  // Rebuilding via innerHTML resets scroll; restore so a checkbox toggle
  // doesn't yank the list away from where the user was looking.
  box.scrollTop = scrollTop;
}

/** Load the upstream /v1/models catalog for one profile (stored key reused backend-side). */
async function browsePlatterCatalog(pid) {
  const p = (configState.profiles || []).find((x) => x.id === pid);
  if (!p) return;
  platterCatalogState[pid] = { status: "loading", ids: [], errText: "" };
  renderPlatterProviders();
  let next;
  try {
    const t = tplById(p.template_id);
    const r = await call("fetch_models", {
      req: {
        template_id: p.template_id,
        api_format: p.api_format || (t ? t.api_format : ""),
        base_url: p.base_url || "",
        key: "",
        profile_id: p.id,
      },
    });
    const ids = ((r && r.models) || []).map((m) => m.id).filter(Boolean);
    if (r && r.source === "live" && ids.length) {
      next = { status: "ok", ids, errText: "" };
    } else {
      const key = r && r.error_kind === "network" ? "platterCatalogNetworkErr" : "platterCatalogUnsupported";
      next = { status: "err", ids: [], errText: S()[key] };
    }
  } catch (e) {
    next = { status: "err", ids: [], errText: resolveBackendErr(e) };
  }
  platterCatalogState[pid] = next;
  renderPlatterProviders();
}

/** Catalog/manual pick: select into the platter draft only. The profile's own
 *  enabled list is never touched — providers stay independent. */
function enablePlatterCatalogModel(pid, model) {
  const p = (configState.profiles || []).find((x) => x.id === pid);
  if (!p || !model) return;
  if (!platterModelSelected(pid, model)) {
    if (platterDraft.length >= MAX_SCIENCE_MODELS) {
      setMsg(T("errScienceModelCap", { max: MAX_SCIENCE_MODELS }));
      renderPlatterProviders();
      return;
    }
    platterDraft.push({ profile_id: pid, model });
  }
  renderPlatterSelected();
  renderPlatterProviders();
}

function openPlatterEditor() {
  platterDraft = platterEntries().map((e) => ({ profile_id: e.profile_id, model: e.model }));
  platterCatalogState = {}; // fresh session: collapse previously browsed catalogs
  setMsg("");
  if (els.platterTitle) els.platterTitle.textContent = S().platterCardTitle;
  if (els.platterHint) els.platterHint.textContent = T("platterHint", { max: MAX_SCIENCE_MODELS });
  if (els.platterSelectedLabel) els.platterSelectedLabel.textContent = S().platterSelectedLabel;
  if (els.platterImportBtn) els.platterImportBtn.textContent = S().platterImportBtn;
  if (els.platterSaveBtn) els.platterSaveBtn.textContent = S().platterSaveBtn;
  if (els.platterActivateBtn) els.platterActivateBtn.textContent = S().platterActivateBtn;
  if (els.platterCancelBtn) els.platterCancelBtn.textContent = S().cancel;
  const canImport = configState.active_id && configState.active_id !== PLATTER_ACTIVE_ID;
  if (els.platterImportRow) els.platterImportRow.hidden = !canImport;
  renderPlatterProviders();
  renderPlatterSelected();
  showView("platter");
}

function importPlatterFromActive() {
  const id = configState.active_id;
  if (!id || id === PLATTER_ACTIVE_ID) return;
  const p = (configState.profiles || []).find((x) => x.id === id);
  if (!p) return;
  platterDraft = platterCatalogModels(p).slice(0, MAX_SCIENCE_MODELS).map((m) => ({ profile_id: id, model: m }));
  renderPlatterProviders();
  renderPlatterSelected();
}

async function savePlatter() {
  if (!platterDraft.length) {
    setMsg(S().errPlatterEmpty);
    return;
  }
  setBusy(true, { kind: "savePlatter" });
  try {
    const res = await call("save_model_platter", { entries: platterDraft });
    setMsg(res && res.proxy_reloaded ? S().platterSaveReloaded : S().platterSaveOk, "ok");
    await loadConfig({ keepView: true });
  } catch (e) {
    setMsg(resolveBackendErr(e));
  } finally {
    setBusy(false);
  }
}

async function activatePlatter(skipVerify) {
  hideSkip();
  if (!platterEntries().length && !platterDraft.length) {
    openPlatterEditor();
    return;
  }
  setBusy(true, { kind: "activatePlatter" });
  try {
    if (platterDraft.length) {
      await call("save_model_platter", { entries: platterDraft });
    }
    const r = await call("set_active_platter", { skipVerify: !!skipVerify });
    if (r && r.committed) {
      setMsg("");
      await loadConfig();
    } else {
      await loadConfig();
      setMsg(resolveHint(r, "switchRejected"));
      if (r && r.can_skip) { pendingSkipActivateId = PLATTER_ACTIVE_ID; showSkip(); }
    }
  } catch (e) {
    await loadConfig();
    setMsg(T("switchFail", { err: resolveBackendErr(e) }));
  } finally {
    setBusy(false);
  }
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
  }).join("") + renderPlatterCardHtml();
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

const PLATTER_ACTIVE_ID = "__platter__";
const MAX_SCIENCE_MODELS = 8;

function modelsToEnableOnCreate(discoveredIds, builtin) {
  const ids = (discoveredIds || []).filter(Boolean);
  const builtins = (builtin || []).filter(Boolean);
  const candidates = ids.length ? ids : builtins;
  return candidates.length ? [candidates[0]] : [];
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
    "listSec", "profileList", "newBtn", "listhdMoreBtn", "listhdMenu", "editCspJsonBtn", "profileDiscoverBtn", "skipActivateBtn",
    "i18nMyConfigs", "i18nLabelProvider", "i18nLabelBase", "i18nLabelKey",
    "i18nConnName", "i18nConnBase", "i18nConnKey", "i18nPorts", "i18nProxyPort", "i18nSandboxPort",
    "wizSec", "wizName", "wizPresetBtn", "wizPresetMenu", "wizBase", "wizKey", "wizSaveBtn", "wizCancelBtn",
    "connSec", "connTitle", "connName", "connBase", "connBaseHint",
    "connModelLabel", "connModelInfo", "connModelHint", "connModelPick", "connKey", "connSaveBtn", "connCancelBtn",
    "platterSec", "platterTitle", "platterHint", "platterImportRow", "platterImportBtn",
    "platterProviderList", "platterSelectedLabel", "platterSelectedList", "platterCapHint",
    "platterSaveBtn", "platterActivateBtn", "platterCancelBtn",
    "profileDiscoverSec", "profileDiscoverTitle", "profileDiscoverHint", "profileDiscoverList",
    "profileDiscoverEmpty", "profileDiscoverEmptyText", "profileDiscoverImportBtn", "profileDiscoverCancelBtn",
    "tabProfiles", "tabSkills", "skillPane", "skillListSec",
    "skillCreateBtn", "skillCreateSec", "skillCreateTitle", "skillCreateName", "skillCreateDesc",
    "skillCreateBody", "skillCreateInspection", "skillCreateErrors",
    "skillCreateSaveBtn", "skillCreateCancelBtn",
    "skillMoreBtn", "skillMenu", "skillEmpty", "skillEmptyTitle", "skillEmptyHint",
    "skillApplyHint", "skillList", "skillMsg",
    "skillDiscoverBtn", "skillDiscoverSec", "skillDiscoverTitle", "skillDiscoverHint",
    "skillDiscoverList", "skillDiscoverEmpty", "skillDiscoverEmptyText",
    "skillDiscoverImportBtn", "skillDiscoverCancelBtn",
    "skillImportBtn", "skillImportSec", "skillImportTitle", "skillImportIntro",
    "skillPathLabel", "skillPathHint",
    "skillSourcePath", "skillBrowseBtn",
    "skillInspectionPreview", "skillInspPreviewTitle",
    "inspName", "inspDesc", "inspStats", "inspReqs", "inspWarnings", "inspErrors",
    "skillImportConfirmBtn", "skillImportCancelBtn",
    "skillAdoptBtn", "skillAdoptSec", "skillAdoptTitle", "skillAdoptHint",
    "skillAdoptList", "skillAdoptEmpty", "skillAdoptEmptyText",
    "skillAdoptConfirmBtn", "skillAdoptCancelBtn",
    "previewOverlay", "previewOverlayTitle", "previewOverlayMeta", "previewOverlayBody",
    "previewOverlayFileSelect", "previewOverlayOpenBtn", "previewOverlayCloseBtn",
    "tabMcp", "mcpPane", "mcpListSec", "mcpAddBtn", "mcpMoreBtn", "mcpMenu", "mcpEmpty", "mcpEmptyTitle", "mcpEmptyHint",
    "mcpApplyHint", "mcpList", "mcpMsg",
    "mcpJsonBtn", "mcpNetworkAllowlistBtn", "mcpDiscoverBtn", "mcpDiscoverSec", "mcpDiscoverTitle", "mcpDiscoverHint",
    "mcpDiscoverList", "mcpDiscoverEmpty", "mcpDiscoverEmptyText", "mcpDiscoverImportBtn", "mcpDiscoverCancelBtn",
    "mcpFormSec", "mcpFormTitle", "mcpName", "mcpNameHint", "mcpDesc",
    "mcpTransport", "mcpTransportLabel", "mcpTransportHint",
    "mcpStdioFields", "mcpRemoteFields",
    "mcpCommand", "mcpCommandLabel", "mcpCommandHint",
    "mcpArgs", "mcpArgsLabel", "mcpArgsHint",
    "mcpEnv", "mcpEnvLabel", "mcpEnvHint",
    "mcpUrl", "mcpUrlLabel", "mcpUrlHint",
    "mcpHeaders", "mcpHeadersLabel", "mcpHeadersHint",
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
      if (act === "editplatter") { closeAllMenus(); openPlatterEditor(); return; }
      closeAllMenus();
      if (act === "editconn") openConn(id);
      return;
    }
    if (row && !e.target.closest(".pmenu-wrap")) {
      const id = row.getAttribute("data-id");
      if (!id || id === configState.active_id) return;
      if (id === PLATTER_ACTIVE_ID) {
        if (!platterEntries().length) openPlatterEditor();
        else activatePlatter(false);
        return;
      }
      activate(id, false);
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
  if (els.profileDiscoverBtn) {
    els.profileDiscoverBtn.addEventListener("click", () => {
      if (busy) return;
      openProfileDiscover();
    });
  }
  if (els.profileDiscoverCancelBtn) {
    els.profileDiscoverCancelBtn.addEventListener("click", closeProfileDiscover);
  }
  if (els.profileDiscoverImportBtn) {
    els.profileDiscoverImportBtn.addEventListener("click", importDiscoveredEditorLlms);
  }
  if (els.profileDiscoverList) {
    els.profileDiscoverList.addEventListener("change", () => refreshProfileDiscoverGate());
    els.profileDiscoverList.addEventListener("click", (e) => {
      const btn = e.target.closest("[data-llm-preview-name]");
      if (!btn) return;
      // Row is a <label>; without this the button click would also toggle the checkbox.
      e.preventDefault();
      e.stopPropagation();
      loadProfileDiscoverPreview(
        btn.getAttribute("data-llm-preview-source"),
        btn.getAttribute("data-llm-preview-name"),
        btn.getAttribute("data-llm-preview-label")
      );
    });
  }
  els.skipActivateBtn.addEventListener("click", () => {
    const id = pendingSkipActivateId;
    if (!id) return;
    if (id === PLATTER_ACTIVE_ID) activatePlatter(true);
    else activate(id, true);
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
  if (els.platterSaveBtn) els.platterSaveBtn.addEventListener("click", savePlatter);
  if (els.platterActivateBtn) els.platterActivateBtn.addEventListener("click", () => activatePlatter(false));
  if (els.platterCancelBtn) els.platterCancelBtn.addEventListener("click", cancelForm);
  if (els.platterImportBtn) els.platterImportBtn.addEventListener("click", importPlatterFromActive);
  if (els.platterSelectedList) {
    els.platterSelectedList.addEventListener("click", (e) => {
      const up = e.target.closest("[data-platter-up]");
      const down = e.target.closest("[data-platter-down]");
      const rm = e.target.closest("[data-platter-rm]");
      if (up) {
        const i = parseInt(up.getAttribute("data-platter-up"), 10);
        if (i > 0) {
          const t = platterDraft[i - 1]; platterDraft[i - 1] = platterDraft[i]; platterDraft[i] = t;
          renderPlatterSelected(); renderPlatterProviders();
        }
      } else if (down) {
        const i = parseInt(down.getAttribute("data-platter-down"), 10);
        if (i >= 0 && i < platterDraft.length - 1) {
          const t = platterDraft[i + 1]; platterDraft[i + 1] = platterDraft[i]; platterDraft[i] = t;
          renderPlatterSelected(); renderPlatterProviders();
        }
      } else if (rm) {
        const i = parseInt(rm.getAttribute("data-platter-rm"), 10);
        if (i >= 0) {
          platterDraft.splice(i, 1);
          renderPlatterSelected(); renderPlatterProviders();
        }
      }
    });
  }

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
  els.mcpDiscoverList.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-mcp-preview-source]");
    if (!btn) return;
    e.preventDefault();
    e.stopPropagation();
    loadMcpDiscoverPreview(
      btn.getAttribute("data-mcp-preview-source"),
      btn.getAttribute("data-mcp-preview-name"),
      btn.getAttribute("data-mcp-preview-label")
    );
  });
  els.mcpAddBtn.addEventListener("click", () => openMcpForm());
  els.mcpSaveBtn.addEventListener("click", saveMcpServer);
  els.mcpCancelBtn.addEventListener("click", closeMcpForm);
  // Editing any field invalidates a prior warning acknowledgement.
  [els.mcpName, els.mcpDesc, els.mcpCommand, els.mcpArgs, els.mcpEnv, els.mcpUrl, els.mcpHeaders].forEach((el) =>
    el && el.addEventListener("input", () => { mcpWarnAck = false; })
  );
  if (els.mcpTransport) {
    els.mcpTransport.addEventListener("change", () => {
      mcpWarnAck = false;
      syncMcpTransportFields();
    });
  }
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
      if (act === "edit") openMcpForm(id);
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
  els.skillBrowseBtn.addEventListener("click", () => {
    pickSkillSource();
  });
  els.skillImportConfirmBtn.addEventListener("click", importSkillConfirm);
  els.skillImportCancelBtn.addEventListener("click", closeSkillImport);
  els.skillImportBtn.addEventListener("click", openSkillImport);
  els.skillDiscoverBtn.addEventListener("click", openSkillDiscover);
  els.skillDiscoverCancelBtn.addEventListener("click", closeSkillDiscover);
  els.skillDiscoverImportBtn.addEventListener("click", importDiscoveredSkills);
  els.skillAdoptBtn.addEventListener("click", openSkillAdopt);
  els.skillAdoptCancelBtn.addEventListener("click", closeSkillAdopt);
  els.skillAdoptConfirmBtn.addEventListener("click", adoptWorkspaceSkills);
  if (els.previewOverlayCloseBtn) {
    els.previewOverlayCloseBtn.addEventListener("click", closePreviewOverlay);
  }
  if (els.previewOverlayOpenBtn) {
    els.previewOverlayOpenBtn.addEventListener("click", openPreviewOverlayPath);
  }
  if (els.previewOverlayFileSelect) {
    els.previewOverlayFileSelect.addEventListener("change", () => {
      if (!previewOverlayState || previewOverlayState.kind !== "skill-adopt") return;
      loadAdoptPreview(previewOverlayState.key, els.previewOverlayFileSelect.value);
    });
  }
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && els.previewOverlay && !els.previewOverlay.hidden) {
      closePreviewOverlay();
    }
  });
  els.skillDiscoverList.addEventListener("change", (e) => {
    if (e.target && e.target.type === "checkbox") refreshDiscoverGate();
  });
  // Expand/collapse long descriptions without toggling the row checkbox.
  els.skillDiscoverList.addEventListener("click", (e) => {
    if (handleDescToggleClick(e)) e.stopPropagation();
  });
  els.skillAdoptList.addEventListener("change", (e) => {
    if (e.target && e.target.type === "checkbox") refreshAdoptGate();
  });
  els.skillAdoptList.addEventListener("click", (e) => {
    if (handleDescToggleClick(e)) {
      e.stopPropagation();
      return;
    }
    const btn = e.target.closest("[data-adopt-preview]");
    if (!btn) return;
    e.preventDefault();
    e.stopPropagation();
    loadAdoptPreview(btn.getAttribute("data-adopt-preview"));
  });
  els.skillSourcePath.addEventListener("input", () => {
    skillResolvedImport = { source: "", importPath: "" };
    els.skillImportConfirmBtn.disabled = !els.skillSourcePath.value.trim();
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
      if (act === "edit") openSkillFile(id);
      if (act === "openfolder") openSkillFolder(id);
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

  // Reset every tab's sub-view so leaving a form never leaves view-form stuck.
  showSkillView("list");
  showMcpView("list");
  if (isProfiles) {
    showView("list");
  } else {
    // Clear profile form state; shared footer stays on list chrome via showSkillView/showMcpView.
    els.listSec.hidden = false;
    els.wizSec.hidden = true;
    els.connSec.hidden = true;
    closeWizPresetMenu();
    hideSkip();
    els.panel.classList.remove("view-form");
    updateSharedRuntimeFooter();
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
  const cls = kind === "ok" || kind === "info" || kind === "warn" ? kind : "err";
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
          <span>${escapeHtml(S().metaSize)}: ${escapeHtml(formatBytes(s.sizeBytes))}</span>
          ${dateText ? `<span>${escapeHtml(S().metaImported)}: ${escapeHtml(dateText)}</span>` : ""}
        </div>
        ${reqTags ? `<div class="skill-reqs-list">${reqTags}</div>` : ""}
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
    const result = await call("set_skill_enabled", { input: { skillId: id, enabled } });
    await loadSkills();
    if (result && result.needsRestart) {
      setSkillMsg(T("skillToggledRestarting"), "info");
      try {
        await call("one_click_login");
        setSkillMsg(T("skillToggledRestarted"), "ok");
      } catch (e) {
        setSkillMsg(T("skillToggledRestartFail", { err: resolveBackendErr(e) }));
      }
    } else {
      setSkillMsg("");
    }
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
      setSkillMsg(T("skillCreatedRestarting"), "info");
      try {
        await call("one_click_login");
        setSkillMsg(T("skillCreatedRestarted"), "ok");
      } catch (e) {
        setSkillMsg(T("skillCreatedRestartFail", { err: resolveBackendErr(e) }));
      }
    } else {
      setSkillMsg(T("skillCreated"), "ok");
    }
  } catch (e) {
    els.skillCreateInspection.hidden = false;
    els.skillCreateErrors.hidden = false;
    els.skillCreateErrors.textContent = resolveBackendErr(e);
  } finally {
    setBusy(false);
  }
}

// ── Skill path import (full-page form from ⋯ → Import) ──
let skillResolvedImport = { source: "", importPath: "" };

function resetSkillPathImport() {
  if (els.skillSourcePath) els.skillSourcePath.value = "";
  skillResolvedImport = { source: "", importPath: "" };
  if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.disabled = true;
  if (els.skillInspectionPreview) els.skillInspectionPreview.hidden = true;
}

function openSkillImport() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
  resetSkillPathImport();
  showSkillView("import");
}

function closeSkillImport() {
  resetSkillPathImport();
  showSkillView("list");
}

async function pickSkillSource() {
  if (busy) return;
  try {
    const path = await call("pick_skill_source", {
      input: { title: T("skillPickTitle") },
    });
    if (!path) return;
    els.skillSourcePath.value = path;
    await inspectSkillSource();
  } catch (e) {
    setSkillMsg(resolveBackendErr(e));
  }
}

async function inspectSkillSource(opts) {
  const skipBusy = opts && opts.skipBusy;
  const source = els.skillSourcePath.value.trim();
  if (!source) return false;
  if (!skipBusy) setBusy(true);
  let canImport = false;
  try {
    const data = await call("inspect_skill_source", { input: { source } });
    skillResolvedImport = {
      source: data.logicalSource || source,
      importPath: data.importPath || "",
    };
    els.inspName.textContent = data.name || S().inspUnnamedSkill;
    els.inspDesc.textContent = data.description || S().inspNoDesc;
    els.inspStats.textContent = T("inspFileCountSize", {
      count: data.fileCount,
      size: formatBytes(data.totalSizeBytes),
    });
    els.inspReqs.textContent = T("inspRequirements", {
      reqs: (data.requirements || []).join(", ") || S().inspNone,
    });

    if (data.warnings && data.warnings.length) {
      els.inspWarnings.hidden = false;
      els.inspWarnings.textContent = T("inspWarningsPrefix", { msg: data.warnings.join("; ") });
    } else {
      els.inspWarnings.hidden = true;
    }

    if (data.errors && data.errors.length) {
      els.inspErrors.hidden = false;
      els.inspErrors.textContent = T("inspErrorsPrefix", { msg: data.errors.join("; ") });
    } else {
      els.inspErrors.hidden = true;
    }

    els.skillInspectionPreview.hidden = false;
    canImport = !!data.valid;
  } catch (e) {
    els.inspName.textContent = "";
    els.inspDesc.textContent = "";
    els.inspStats.textContent = "";
    els.inspReqs.textContent = "";
    els.inspWarnings.hidden = true;
    els.inspErrors.hidden = false;
    els.inspErrors.textContent = resolveBackendErr(e);
    els.skillInspectionPreview.hidden = false;
    canImport = false;
  } finally {
    if (!skipBusy) setBusy(false);
    if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.disabled = !canImport;
  }
  return canImport;
}

async function importSkillConfirm() {
  const source = els.skillSourcePath.value.trim();
  if (!source) return;
  setBusy(true);
  try {
    const already =
      skillResolvedImport.source === source && !!skillResolvedImport.importPath;
    if (!already) {
      const ok = await inspectSkillSource({ skipBusy: true });
      if (!ok) return;
    }
    const result = await call("import_skill", {
      input: {
        source: skillResolvedImport.source || source,
        importPath: skillResolvedImport.importPath || undefined,
      },
    });
    closeSkillImport();
    await loadSkills();
    if (result && result.needsRestart) {
      setSkillMsg(T("skillImportedRestarting"), "info");
      try {
        await call("one_click_login");
        setSkillMsg(T("skillImportedRestarted"), "ok");
      } catch (e) {
        setSkillMsg(T("skillImportedRestartFail", { err: resolveBackendErr(e) }));
      }
    } else {
      setSkillMsg(T("skillImported"), "ok");
    }
  } catch (e) {
    els.inspErrors.hidden = false;
    els.inspErrors.textContent = resolveBackendErr(e);
    els.skillInspectionPreview.hidden = false;
  } finally {
    const ready =
      skillResolvedImport.source === els.skillSourcePath.value.trim() &&
      !!skillResolvedImport.importPath;
    setBusy(false);
    if (els.skillImportConfirmBtn) els.skillImportConfirmBtn.disabled = !ready;
  }
}

// ── Skill Discovery (scan → pick → import; same pattern as MCP) ──
async function openSkillDiscover() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
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
          ${skillDescHtml(d.description)}
          <span class="skill-meta"><span>${escapeHtml(d.sourceLabel)}</span></span>
        </span>
      </label>
    `;
  }).join("");
  refreshDescToggles(els.skillDiscoverList);
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
  let needsRestart = false;
  try {
    for (const p of paths) {
      try {
        const result = await call("import_skill", { input: { source: p } });
        if (result && result.needsRestart) needsRestart = true;
      } catch (e) {
        failures.push(`${p}: ${resolveBackendErr(e)}`);
      }
    }
    closeSkillDiscover();
    await loadSkills();
    if (needsRestart) {
      setSkillMsg(T("skillImportedRestarting"), "info");
      try {
        await call("one_click_login");
        setSkillMsg(
          failures.length
            ? T("skillImportPartialFail", { err: failures.join("; ") })
            : T("skillImportedRestarted"),
          failures.length ? "err" : "ok"
        );
      } catch (e) {
        setSkillMsg(T("skillImportedRestartFail", { err: resolveBackendErr(e) }));
      }
    } else if (failures.length) {
      setSkillMsg(T("skillImportPartialFail", { err: failures.join("; ") }));
    } else {
      setSkillMsg(T("skillImported"), "ok");
    }
  } finally {
    setBusy(false);
  }
}

// ── Science Workspace Skill Adopt ──
let previewOverlayState = null;

function closePreviewOverlay() {
  previewOverlayState = null;
  if (els.previewOverlay) {
    els.previewOverlay.hidden = true;
    els.previewOverlay.classList.remove("preview-overlay--compact");
  }
  if (els.previewOverlayBody) els.previewOverlayBody.textContent = "";
  if (els.previewOverlayMeta) els.previewOverlayMeta.textContent = "";
  if (els.previewOverlayFileSelect) {
    els.previewOverlayFileSelect.innerHTML = "";
    els.previewOverlayFileSelect.hidden = true;
  }
  if (els.previewOverlayOpenBtn) els.previewOverlayOpenBtn.hidden = true;
}

function setPreviewOverlayLayout(layout) {
  if (!els.previewOverlay) return;
  els.previewOverlay.classList.toggle("preview-overlay--compact", layout === "compact");
}

function showPreviewOverlayLoading(title, layout = "full") {
  setPreviewOverlayLayout(layout);
  if (els.previewOverlayTitle) {
    els.previewOverlayTitle.textContent = title || S().skillAdoptPreviewTitle;
  }
  if (els.previewOverlayMeta) els.previewOverlayMeta.textContent = "";
  if (els.previewOverlayBody) els.previewOverlayBody.textContent = S().skillScanning;
  if (els.previewOverlayFileSelect) els.previewOverlayFileSelect.hidden = true;
  if (els.previewOverlayOpenBtn) els.previewOverlayOpenBtn.hidden = true;
  if (els.previewOverlay) els.previewOverlay.hidden = false;
}

async function openPreviewOverlayPath() {
  if (!previewOverlayState) return;
  try {
    if (previewOverlayState.kind === "skill-adopt") {
      await call("open_science_skill", { input: { key: previewOverlayState.key } });
    } else if (previewOverlayState.kind === "mcp") {
      await call("open_discovered_mcp_source", {
        input: { sourcePath: previewOverlayState.sourcePath },
      });
    }
  } catch (e) {
    const err = resolveBackendErr(e);
    const msg =
      previewOverlayState?.kind === "mcp"
        ? T("mcpPreviewFail", { err })
        : T("skillAdoptPreviewFail", { err });
    if (els.previewOverlayMeta) els.previewOverlayMeta.textContent = msg;
    if (previewOverlayState?.kind === "mcp") setMcpMsg(msg);
    else setSkillMsg(msg);
  }
}

async function openSkillAdopt() {
  closeMenu(els.skillMenu, els.skillMoreBtn);
  if (busy) return;
  closePreviewOverlay();
  els.skillAdoptList.innerHTML = `<p class="hint">${escapeHtml(S().skillScanning)}</p>`;
  els.skillAdoptEmpty.hidden = true;
  els.skillAdoptConfirmBtn.disabled = true;
  showSkillView("adopt");
  try {
    const found = (await call("discover_science_skill_sync")) || [];
    renderSkillAdopt(found);
  } catch (e) {
    els.skillAdoptList.innerHTML = "";
    els.skillAdoptEmpty.hidden = false;
    setSkillMsg(resolveBackendErr(e));
  }
}

function closeSkillAdopt() {
  closePreviewOverlay();
  showSkillView("list");
}

function skillSyncKindLabel(kind) {
  if (kind === "harvest") return S().skillSyncKindHarvest;
  if (kind === "import") return S().skillSyncKindImport;
  return S().skillSyncKindWorkspace;
}

function renderSkillAdopt(found) {
  if (!found.length) {
    els.skillAdoptList.innerHTML = "";
    els.skillAdoptEmpty.hidden = false;
    return;
  }
  els.skillAdoptEmpty.hidden = true;
  const previewLabel = S().skillAdoptPreview;
  els.skillAdoptList.innerHTML = found.map((d) => {
    const files = (d.files || []).slice(0, 6).join(", ");
    const more = (d.files || []).length > 6 ? ` +${d.files.length - 6}` : "";
    const warn = (d.warnings || []).length
      ? `<span class="skill-meta"><span>${escapeHtml(d.warnings[0])}</span></span>`
      : "";
    const kindBadge = `<span class="skill-req-tag">${escapeHtml(skillSyncKindLabel(d.kind))}</span>`;
    const checked = d.kind === "harvest" ? " checked" : "";
    let bytes = "";
    if (d.storeBytes != null || d.scienceBytes != null) {
      bytes = `<span class="skill-meta"><span>${escapeHtml(T("skillSyncBytes", {
        store: formatByteSize(d.storeBytes || 0),
        science: formatByteSize(d.scienceBytes || 0),
      }))}</span></span>`;
    }
    return `
      <div class="skill-discover-row">
        <input type="checkbox" value="${escapeHtml(d.key)}"${checked} />
        <span class="skill-discover-main">
          <span class="skill-name">${escapeHtml(d.name)} ${kindBadge}</span>
          ${skillDescHtml(d.description)}
          ${bytes}
          ${files ? `<span class="skill-meta"><span>${escapeHtml(files)}${more}</span></span>` : ""}
          ${warn}
        </span>
        <span class="skill-discover-actions">
          <button type="button" class="btn small" data-adopt-preview="${escapeHtml(d.key)}">${escapeHtml(previewLabel)}</button>
        </span>
      </div>
    `;
  }).join("");
  refreshDescToggles(els.skillAdoptList);
  refreshAdoptGate();
}

function refreshAdoptGate() {
  const checked = els.skillAdoptList.querySelectorAll("input[type=checkbox]:checked");
  els.skillAdoptConfirmBtn.disabled = busy || checked.length === 0;
}

function formatByteSize(n) {
  const v = Number(n) || 0;
  if (v < 1024) return `${v} B`;
  if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KB`;
  return `${(v / (1024 * 1024)).toFixed(1)} MB`;
}

async function loadAdoptPreview(key, file) {
  if (!key || busy) return;
  showPreviewOverlayLoading(S().skillAdoptPreviewTitle, "full");
  previewOverlayState = { kind: "skill-adopt", key };
  try {
    const data = await call("preview_science_skill", {
      input: { key, file: file || null },
    });
    previewOverlayState = { kind: "skill-adopt", key: data.key };
    const files = data.files || [];
    if (files.length > 1 && els.previewOverlayFileSelect) {
      els.previewOverlayFileSelect.hidden = false;
      els.previewOverlayFileSelect.innerHTML = files.map((f) => {
        const selected = f.name === data.activeFile ? " selected" : "";
        return `<option value="${escapeHtml(f.name)}"${selected}>${escapeHtml(f.name)} (${formatByteSize(f.sizeBytes)})</option>`;
      }).join("");
    } else if (els.previewOverlayFileSelect) {
      els.previewOverlayFileSelect.hidden = true;
      els.previewOverlayFileSelect.innerHTML = "";
    }
    if (els.previewOverlayOpenBtn) {
      els.previewOverlayOpenBtn.hidden = false;
      els.previewOverlayOpenBtn.textContent = S().skillAdoptOpen;
    }
    if (els.previewOverlayTitle) {
      els.previewOverlayTitle.textContent = `${S().skillAdoptPreviewTitle} · ${data.name || ""}`;
    }
    let meta = T("skillAdoptPreviewMeta", {
      file: data.activeFile || "",
      chars: String(data.charCount ?? 0),
      bytes: formatByteSize((files.find((f) => f.name === data.activeFile)?.sizeBytes) || 0),
    });
    if (data.truncated) meta += " " + S().skillAdoptPreviewTruncated;
    if (data.alreadyImported) meta += " · " + S().skillAdoptReadoptHint;
    if (els.previewOverlayMeta) els.previewOverlayMeta.textContent = meta;
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = data.content || "";
    if (els.previewOverlay) els.previewOverlay.hidden = false;
  } catch (e) {
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = "";
    if (els.previewOverlayMeta) {
      els.previewOverlayMeta.textContent = T("skillAdoptPreviewFail", {
        err: resolveBackendErr(e),
      });
    }
  }
}

async function adoptWorkspaceSkills() {
  const keys = Array.from(
    els.skillAdoptList.querySelectorAll("input[type=checkbox]:checked")
  ).map((el) => el.value);
  if (!keys.length) return;
  setBusy(true);
  try {
    const result = await call("sync_science_skills", { input: { keys } });
    closeSkillAdopt();
    await loadSkills();
    const failures = (result.failures || []).join("; ");
    if (result.needsRestart) {
      setSkillMsg(T("skillAdoptedRestarting"), "info");
      try {
        await call("one_click_login");
        setSkillMsg(T("skillAdoptedRestarted"), "ok");
      } catch (e) {
        setSkillMsg(T("skillAdoptedRestartFail", { err: resolveBackendErr(e) }));
      }
    } else if (failures) {
      setSkillMsg(T("skillAdoptPartialFail", { err: failures }));
    } else {
      setSkillMsg(S().skillSynced || S().skillImported, "ok");
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
  closePreviewOverlay();
  els.mcpDiscoverList.innerHTML = `<p class="hint">${escapeHtml(S().mcpScanning)}</p>`;
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
  closePreviewOverlay();
  showMcpView("list");
}

function renderMcpDiscover(found) {
  if (!found.length) {
    els.mcpDiscoverList.innerHTML = "";
    els.mcpDiscoverEmpty.hidden = false;
    return;
  }
  els.mcpDiscoverEmpty.hidden = true;
  const importedBadge = S().skillAlreadyImported;
  const previewLabel = S().mcpDiscoverPreview;
  els.mcpDiscoverList.innerHTML = found.map((d) => {
    const disabled = d.alreadyImported ? " disabled" : "";
    const badge = d.alreadyImported ? `<span class="skill-req-tag">${escapeHtml(importedBadge)}</span>` : "";
    const transport = d.transport || "stdio";
    const isRemote = transport === "sse" || transport === "streamable_http";
    const transportBadge = isRemote
      ? (transport === "sse" ? S().mcpBadgeSse : S().mcpBadgeHttp)
      : S().mcpBadgeStdio;
    const envKeys = isRemote ? (d.headerKeys || []) : (d.envKeys || []);
    const env = envKeys.length
      ? `<span class="skill-meta"><span>${isRemote ? "headers" : "env"}: ${envKeys.length}</span></span>`
      : "";
    const rawDetail = isRemote
      ? (d.url || "")
      : `${d.command || ""}${(d.args || []).length ? ` ${(d.args || []).join(" ")}` : ""}`;
    const detail = escapeHtml(truncateOneLine(rawDetail, 96));
    return `
      <div class="skill-discover-row${d.alreadyImported ? " disabled" : ""}">
        <input type="checkbox" data-source-path="${escapeHtml(d.sourcePath)}" value="${escapeHtml(d.name)}"${disabled} />
        <span class="skill-discover-main">
          <span class="skill-name">${escapeHtml(d.name)} ${badge} <span class="badge">${escapeHtml(transportBadge)}</span></span>
          ${detail ? `<span class="mcp-cmd">${detail}</span>` : ""}
          ${env}
          <span class="skill-meta"><span>${escapeHtml(d.sourceLabel)}</span></span>
        </span>
        <span class="skill-discover-actions">
          <button type="button" class="btn small"
            data-mcp-preview-source="${escapeHtml(d.sourcePath)}"
            data-mcp-preview-name="${escapeHtml(d.name)}"
            data-mcp-preview-label="${escapeHtml(d.sourceLabel)}">${escapeHtml(previewLabel)}</button>
        </span>
      </div>
    `;
  }).join("");
  refreshMcpDiscoverGate();
}

async function loadMcpDiscoverPreview(sourcePath, name, sourceLabel) {
  if (!sourcePath || !name || busy) return;
  showPreviewOverlayLoading(S().mcpPreviewTitle, "compact");
  previewOverlayState = { kind: "mcp", sourcePath, name };
  if (els.previewOverlayOpenBtn) {
    els.previewOverlayOpenBtn.hidden = false;
    els.previewOverlayOpenBtn.textContent = S().mcpPreviewOpen;
  }
  try {
    const data = await call("preview_discovered_mcp", {
      input: { sourcePath, name },
    });
    previewOverlayState = { kind: "mcp", sourcePath: data.sourcePath, name: data.name };
    if (els.previewOverlayTitle) {
      els.previewOverlayTitle.textContent = `${S().mcpPreviewTitle} · ${data.name || ""}`;
    }
    let meta = T("mcpPreviewMeta", {
      name: data.name || "",
      source: sourceLabel || "",
    });
    if (data.truncated) meta += " " + S().skillAdoptPreviewTruncated;
    if (els.previewOverlayMeta) els.previewOverlayMeta.textContent = meta;
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = data.content || "";
    if (els.previewOverlay) els.previewOverlay.hidden = false;
  } catch (e) {
    if (els.previewOverlayBody) els.previewOverlayBody.textContent = "";
    if (els.previewOverlayMeta) {
      els.previewOverlayMeta.textContent = T("mcpPreviewFail", {
        err: resolveBackendErr(e),
      });
    }
  }
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
  let needsRestart = false;
  try {
    for (const item of selected) {
      try {
        const result = await call("import_discovered_mcp_server", { input: item });
        if (result && result.needsRestart) needsRestart = true;
      } catch (e) {
        failures.push(`${item.name}: ${resolveBackendErr(e)}`);
      }
    }
    closeMcpDiscover();
    await loadMcp();
    if (needsRestart) {
      setMcpMsg(T("mcpImportedRestarting"), "info");
      try {
        await call("one_click_login");
        setMcpMsg(
          failures.length
            ? T("mcpImportPartialFail", { err: failures.join("; ") })
            : T("mcpImportedRestarted"),
          failures.length ? "err" : "ok"
        );
      } catch (e) {
        setMcpMsg(T("mcpImportedRestartFail", { err: resolveBackendErr(e) }));
      }
    } else if (failures.length) {
      setMcpMsg(T("mcpImportPartialFail", { err: failures.join("; ") }));
    }
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

function setMcpMsg(text, kind = "err") {
  const t = text ? String(text) : "";
  const cls = kind === "ok" || kind === "info" || kind === "warn" ? kind : "err";
  els.mcpMsg.textContent = t;
  els.mcpMsg.className = "msg" + (t ? " " + cls : "");
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
    const transport = (s.transport || "stdio");
    const isRemote = transport === "sse" || transport === "streamable_http";
    const badge = isRemote
      ? (transport === "sse" ? S().mcpBadgeSse : S().mcpBadgeHttp)
      : S().mcpBadgeStdio;
    const cmdLine = isRemote
      ? escapeHtml(s.url || "")
      : escapeHtml([s.command, ...(s.args || [])].join(" "));
    const secretKeys = Object.keys(isRemote ? (s.headers || {}) : (s.env || {}));
    const envTags = secretKeys.map((k) => `<span class="skill-req-tag" title="${escapeHtml(k)}">${escapeHtml(k)}</span>`).join("");
    return `
      <div class="skill-row${enabledClass}" data-id="${escapeHtml(s.id)}" data-name="${escapeHtml(s.name)}">
        <div class="skill-row-top">
          <div class="skill-title-group">
            <input type="checkbox"${checked} />
            <span class="skill-name" title="${escapeHtml(s.name)}">${escapeHtml(s.name)}</span>
            <span class="badge">${escapeHtml(badge)}</span>
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
        </div>
        ${envTags ? `<div class="skill-reqs-list">${envTags}</div>` : ""}
      </div>
    `;
  }).join("");
  refreshDescToggles(els.mcpList);
}

async function toggleMcpServer(id, enabled) {
  setBusy(true);
  try {
    const result = await call("set_mcp_server_enabled", { input: { serverId: id, enabled } });
    await loadMcp();
    if (result && result.needsRestart) {
      setMcpMsg(T("mcpToggledRestarting"), "info");
      try {
        await call("one_click_login");
        setMcpMsg(T("mcpToggledRestarted"), "ok");
      } catch (e) {
        setMcpMsg(T("mcpToggledRestartFail", { err: resolveBackendErr(e) }));
      }
    } else {
      setMcpMsg("");
    }
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

/** Open MCP create/edit form (full-page form-sec, not a modal). */
function openMcpForm(id) {
  if (busy) return;
  closeMenu(els.mcpMenu, els.mcpMoreBtn);
  mcpEditId = id || null;
  const s = id ? mcpCache.find((x) => x.id === id) : null;
  els.mcpFormTitle.textContent = s ? S().mcpFormTitleEdit : S().mcpFormTitleNew;
  els.mcpName.value = s ? s.name : "";
  els.mcpDesc.value = s ? (s.description || "") : "";
  const transport = (s && s.transport) || "stdio";
  if (els.mcpTransport) els.mcpTransport.value = transport;
  els.mcpCommand.value = s ? (s.command || "") : "";
  els.mcpArgs.value = s ? (s.args || []).join("\n") : "";
  // Env/header values are returned masked; on edit we show keys with blank values
  // so the user re-enters secrets intentionally (never round-trip a masked value).
  els.mcpEnv.value = s ? Object.keys(s.env || {}).map((k) => `${k}=`).join("\n") : "";
  if (els.mcpUrl) els.mcpUrl.value = s ? (s.url || "") : "";
  if (els.mcpHeaders) {
    els.mcpHeaders.value = s
      ? Object.keys(s.headers || {}).map((k) => `${k}=`).join("\n")
      : "";
  }
  els.mcpInspection.hidden = true;
  els.mcpWarnings.hidden = true;
  els.mcpErrors.hidden = true;
  mcpWarnAck = false;
  syncMcpTransportFields();
  showMcpView("form");
  els.mcpName.focus();
}

function syncMcpTransportFields() {
  const transport = (els.mcpTransport && els.mcpTransport.value) || "stdio";
  const remote = transport === "sse" || transport === "streamable_http";
  if (els.mcpStdioFields) els.mcpStdioFields.hidden = remote;
  if (els.mcpRemoteFields) els.mcpRemoteFields.hidden = !remote;
}

function closeMcpForm() {
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

function collectMcpInput() {
  const transport = (els.mcpTransport && els.mcpTransport.value) || "stdio";
  const remote = transport === "sse" || transport === "streamable_http";
  return {
    name: els.mcpName.value.trim(),
    description: els.mcpDesc.value.trim(),
    transport,
    command: remote ? "" : els.mcpCommand.value.trim(),
    args: remote ? [] : parseArgsLines(els.mcpArgs.value),
    env: remote ? {} : parseEnvLines(els.mcpEnv.value),
    url: remote ? (els.mcpUrl ? els.mcpUrl.value.trim() : "") : "",
    headers: remote ? parseEnvLines(els.mcpHeaders ? els.mcpHeaders.value : "") : {},
  };
}

async function saveMcpServer() {
  const input = collectMcpInput();
  els.mcpWarnings.hidden = true;
  els.mcpErrors.hidden = true;
  els.mcpInspection.hidden = true;
  try {
    const insp = await call("inspect_mcp_server", { input });
    if (insp && !insp.valid) {
      // Errors always block the save; reset any prior warning acknowledgement.
      mcpWarnAck = false;
      els.mcpErrors.hidden = false;
      els.mcpErrors.textContent = T("inspErrorsPrefix", { msg: (insp.errors || []).join("; ") });
      els.mcpInspection.hidden = false;
      return;
    }
    const warnings = (insp && insp.warnings) || [];
    if (warnings.length && !mcpWarnAck) {
      // Surface warnings and require a second click to confirm past them.
      mcpWarnAck = true;
      els.mcpWarnings.hidden = false;
      els.mcpWarnings.textContent = T("mcpWarnConfirm", { msg: warnings.join("; ") });
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
    let result;
    if (mcpEditId) {
      result = await call("update_mcp_server", { input: { serverId: mcpEditId, server: input } });
    } else {
      result = await call("create_mcp_server", { input });
    }
    closeMcpForm();
    await loadMcp();
    if (result && result.needsRestart) {
      setMcpMsg(T("mcpSavedRestarting"), "info");
      try {
        await call("one_click_login");
        setMcpMsg(T("mcpSavedRestarted"), "ok");
      } catch (e) {
        setMcpMsg(T("mcpSavedRestartFail", { err: resolveBackendErr(e) }));
      }
    }
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
