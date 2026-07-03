"""CSSwitch DSML 兜底 shim：把 DeepSeek 泄漏成纯文本的 DSML 工具调用还原成 tool_use。
纯函数分段器（本文件）+ 流式状态机 + 字节检测器（后续 Task）。不依赖第三方。"""
import codecs
import json
import os
import re

DSML_MARKER_BYTES = (
    "｜DSML｜".encode("utf-8"),
    "｜｜DSML｜｜".encode("utf-8"),
)


def shim_mode(prov_name, prov):
    """off | detect | rewrite。本轮 relay 恒 off（deepseek-only）；deepseek 且 dsml_capable 才读环境变量。"""
    if prov_name == "relay":
        return "off"
    if not (prov or {}).get("dsml_capable"):
        return "off"
    m = os.environ.get("CSSWITCH_TOOLUSE_SHIM", "").lower()
    return m if m in ("detect", "rewrite") else "off"

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


def _coerce_param(pname, string_attr, raw, prop_schema):
    """string="true" → 原始字符串；string="false"/缺 → 按 schema type 转型，失败退 json.loads 再退字符串。"""
    if string_attr == "true":
        return raw
    typ = (prop_schema or {}).get("type")
    try:
        if typ == "integer":
            return int(raw)
        if typ == "number":
            return float(raw)
        if typ == "boolean":
            return raw.strip().lower() in ("true", "1", "yes")
        if typ in ("object", "array"):
            return json.loads(raw)
    except (ValueError, TypeError, json.JSONDecodeError):
        pass
    try:
        return json.loads(raw)
    except (ValueError, TypeError, json.JSONDecodeError):
        return raw


def _type_ok(val, typ):
    """基础类型宽松校验：明显冲突才判 False（第三轮 P2）。"""
    if typ in (None, "string"):
        return isinstance(val, str) if typ == "string" else True
    if typ == "integer":
        return (isinstance(val, int) and not isinstance(val, bool)) or \
               (isinstance(val, str) and val.strip().lstrip("+-").isdigit())
    if typ == "number":
        if isinstance(val, bool):
            return False
        if isinstance(val, (int, float)):
            return True
        try:
            float(val)
            return True
        except (ValueError, TypeError):
            return False
    if typ == "boolean":
        return isinstance(val, bool) or (isinstance(val, str)
                and val.strip().lower() in ("true", "false", "1", "0", "yes", "no"))
    if typ == "object":
        return isinstance(val, dict)
    if typ == "array":
        return isinstance(val, list)
    return True


def _validate_input(inp, schema):
    """required 齐 + 各值基础类型相容；不过返回 False（调用方整段按文本放行）。"""
    schema = schema or {}
    for req in schema.get("required") or []:
        if req not in inp:
            return False
    props = schema.get("properties") or {}
    for k, v in inp.items():
        if k in props and not _type_ok(v, props[k].get("type")):
            return False
    return True


def _parse_invoke(name, body, known_tools):
    """解析一个 invoke → {"name","input"}；参数不合 schema 返回 None（调用方整段作废）。"""
    schema = known_tools.get(name) or {}
    schema_props = schema.get("properties") or {}
    inp = {}
    for pn, sattr, raw in _PARAM_RE.findall(body):
        inp[pn] = _coerce_param(pn, sattr, raw, schema_props.get(pn))
    # wrapper 解包：单个名为 arguments/input 的参数、且非工具真实字段 → 解包其对象
    if len(inp) == 1:
        only = next(iter(inp))
        if only in ("arguments", "input") and only not in schema_props:
            val = inp[only]
            if isinstance(val, str):
                try:
                    val = json.loads(val)
                except (ValueError, json.JSONDecodeError):
                    val = None
            if isinstance(val, dict):
                inp = val
    if not _validate_input(inp, schema):
        return None
    return {"name": name, "input": inp}


def parse_dsml_tool_calls(wrapper_region, known_tools):
    """解析 tool_calls 段。任一工具名未声明或参数不合 schema → 返回 []（保守整块）。"""
    known_tools = known_tools or {}
    out = []
    for m in _TOOLCALLS_RE.finditer(wrapper_region):
        invokes = _INVOKE_RE.findall(m.group(1))
        if not invokes:
            return []
        for name, body in invokes:
            if name not in known_tools:
                return []
            call = _parse_invoke(name, body, known_tools)
            if call is None:      # 参数不合 schema → 整块作废
                return []
            out.append(call)
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


class DsmlStreamRewriter:
    """流式 SSE 改写状态机。Task 4：透明重映射（自管下游索引、通用 delta/stop 映射、增量 UTF-8）。
    Task 5 在此基础上加 text_delta 的 DSML 检测与 tool_use 合成。"""

    def __init__(self, known_tools, nonce=""):
        self.known_tools = known_tools or {}
        self.nonce = nonce or "x"
        self._dec = codecs.getincrementaldecoder("utf-8")()
        self._buf = ""            # 已解码、未成帧的文本
        self.next_out = 0
        self.cur_out = None       # 当前打开的下游块索引
        self.cur_type = None      # 当前上游块类型
        self.synthesized = False
        self.tool_n = 0
        # Task 5 用：
        self.state = "PASS"
        self.scan_buf = ""
        self.cap_buf = ""

    # ---- 对外 ----
    def feed(self, data):
        self._buf += self._dec.decode(data)
        return self._drain_frames()

    def finalize(self):
        # 冲掉解码器残留 + 未成帧尾巴 + Task 5 的扣留文本
        self._buf += self._dec.decode(b"", final=True)
        out = self._drain_frames(flush_tail=True)
        out += self._finalize_text()      # Task 5 覆盖；Task 4 为 b""
        return out

    # ---- 帧循环 ----
    def _drain_frames(self, flush_tail=False):
        out = []
        while True:
            idx = self._buf.find("\n\n")
            if idx < 0:
                # 兼容 \r\n\r\n
                idx = self._buf.find("\r\n\r\n")
                sep = 4
            else:
                sep = 2
            if idx < 0:
                break
            frame = self._buf[:idx]
            self._buf = self._buf[idx + sep:]
            out.append(self._handle_frame(frame))
        return b"".join(out)

    def _finalize_text(self):
        return b""   # Task 5 覆盖

    def _flush_pending(self):
        return b""   # Task 5 覆盖：message 边界前 flush 扣留文本

    # ---- 单帧处理 ----
    def _handle_frame(self, frame):
        event, obj = self._parse_frame(frame)
        if obj is None or not isinstance(obj, dict):
            return self._raw(frame)              # 注释/未知/非 JSON：原样
        t = obj.get("type")
        if t == "content_block_start":
            self.cur_type = (obj.get("content_block") or {}).get("type")
            self.cur_out = self.next_out
            self.next_out += 1
            return self._emit("content_block_start",
                              {**obj, "index": self.cur_out})
        if t == "content_block_delta":
            dtype = (obj.get("delta") or {}).get("type")
            if self.cur_type == "text" and dtype == "text_delta":
                return self._on_text_delta(obj.get("delta", {}).get("text", ""))
            return self._emit("content_block_delta", {**obj, "index": self.cur_out})
        if t == "content_block_stop":
            return self._on_block_stop()
        if t == "message_delta":
            return self._flush_pending() + self._on_message_delta(obj)
        if t == "message_stop":
            return self._flush_pending() + self._raw(frame)
        # message_start / ping / 其它：原样
        return self._raw(frame)

    def _on_text_delta(self, text):
        # Task 4：不检测，原样重发；Task 5 覆盖为扫描器。
        return self._emit("content_block_delta", {"type": "content_block_delta",
                          "index": self.cur_out, "delta": {"type": "text_delta", "text": text}})

    def _on_block_stop(self):
        if self.cur_out is not None:
            out = self._emit("content_block_stop", {"type": "content_block_stop", "index": self.cur_out})
            self.cur_out = None
            return out
        return b""     # 已主动关闭 → 吞掉上游冗余 stop

    def _on_message_delta(self, obj):
        if self.synthesized:
            d = dict(obj.get("delta") or {})
            if d.get("stop_reason") in ("end_turn", "stop", None):
                d["stop_reason"] = "tool_use"
            obj = {**obj, "delta": d}
        return self._emit("message_delta", obj)

    # ---- 工具 ----
    @staticmethod
    def _parse_frame(frame):
        event, data_lines = None, []
        for line in frame.split("\n"):
            line = line.rstrip("\r")
            if line.startswith("event:"):
                event = line[6:].strip()
            elif line.startswith("data:"):
                data_lines.append(line[5:].lstrip())
        if not data_lines:
            return event, None
        try:
            return event, json.loads("\n".join(data_lines))
        except (ValueError, json.JSONDecodeError):
            return event, None

    @staticmethod
    def _emit(event, obj):
        return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n".encode("utf-8")

    @staticmethod
    def _raw(frame):
        return (frame + "\n\n").encode("utf-8")
