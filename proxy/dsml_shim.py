"""CSSwitch DSML 兜底 shim：把 DeepSeek 泄漏成纯文本的 DSML 工具调用还原成 tool_use。
纯函数分段器（本文件）+ 流式状态机 + 字节检测器（后续 Task）。不依赖第三方。"""
import json
import re

# 分隔符：一到两个全角竖线 U+FF5C（vLLM 文档单、issue #8 实测双）。
_P = r"[｜]{1,2}"
_WRAP = r"(?:tool_calls|function_calls)"
_OPEN_RE = re.compile(r"<" + _P + r"DSML" + _P + _WRAP + r">")
_TOOLCALLS_RE = re.compile(
    r"<" + _P + r"DSML" + _P + _WRAP + r">(.*?)</" + _P + r"DSML" + _P + _WRAP + r">", re.S)
_INVOKE_RE = re.compile(
    r"<" + _P + r'DSML' + _P + r'invoke\s+name="([^"]+)"\s*>(.*?)</' + _P + r"DSML" + _P + r"invoke>",
    re.S)
_PARAM_RE = re.compile(
    r"<" + _P + r'DSML' + _P + r'parameter\s+name="([^"]+)"(?:\s+string="(true|false)")?\s*>'
    + r"(.*?)</" + _P + r"DSML" + _P + r"parameter>", re.S)


def _coerce_param(pname, string_attr, raw, schema):
    # Task 1 版：只认字符串（issue #8 全是 string="true"）。Task 2 折入 schema 转型 / wrapper 解包。
    return raw


def _parse_invoke(name, body, known_tools):
    inp = {}
    schema = (known_tools.get(name) or {}).get("properties") or {}
    for pn, sattr, raw in _PARAM_RE.findall(body):
        inp[pn] = _coerce_param(pn, sattr, raw, schema.get(pn))
    return {"name": name, "input": inp}


def parse_dsml_tool_calls(wrapper_region, known_tools):
    """解析一个（或多个）tool_calls 段的内容。段内任一工具名未声明 → 返回 []（保守整块）。"""
    known_tools = known_tools or {}
    out = []
    for m in _TOOLCALLS_RE.finditer(wrapper_region):
        inner = m.group(1)
        invokes = _INVOKE_RE.findall(inner)
        if not invokes:
            return []
        for name, body in invokes:
            if name not in known_tools:
                return []      # 未知工具 → 整块作废（安全）
            out.append(_parse_invoke(name, body, known_tools))
    return out


def segment_dsml_text(text, known_tools):
    """把文本按 DSML tool_calls 段切成有序分段，保留交错。无 DSML → 单 text 分段。"""
    if not text:
        return []
    known_tools = known_tools or {}
    segs = []
    pos = 0
    for m in _TOOLCALLS_RE.finditer(text):
        calls = parse_dsml_tool_calls(m.group(0), known_tools)
        if not calls:
            continue           # 未知工具/坏格式：不切，整段留作文本（下面按文本收）
        if m.start() > pos:
            segs.append({"type": "text", "text": text[pos:m.start()]})
        for c in calls:
            segs.append({"type": "tool_use", "name": c["name"], "input": c["input"]})
        pos = m.end()
    if pos < len(text):
        tail = text[pos:]
        if tail:
            segs.append({"type": "text", "text": tail})
    if not segs:
        return [{"type": "text", "text": text}]
    return segs
