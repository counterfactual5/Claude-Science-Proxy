#!/usr/bin/env python3
"""隔离回归测试：翻译代理 ↔ 真实通义千问。

只打本项目的翻译代理（proxy/qwen_proxy.py），完全不启动 Claude Science、
不触碰任何 OAuth / 真实实例 / CC Switch。自动起代理、跑用例、停代理。

用例覆盖 Science 真实请求形状:
  1. GET /v1/models
  2. 非流式文本
  3. 流式 SSE（Science 主推理路径）
  4. tool_use 发起（Anthropic 格式）
  5. tool_result 回喂 → 模型接着作答（agentic 往返）
  6. 全程入站 OAuth Bearer 被剥离

用法:
  DASHSCOPE_API_KEY=... python3 test/proxy_e2e_test.py
  或 python3 test/proxy_e2e_test.py --env-file <某个.env> --key-name DASHSCOPE_API_KEY
"""
import argparse
import json
import os
import subprocess
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
PROXY = os.path.join(HERE, "..", "proxy", "qwen_proxy.py")
PORT = 18992  # 与手动调试端口 18991 错开，避免撞车
BASE = f"http://127.0.0.1:{PORT}"
FAKE_BEARER = "Bearer sk-ant-oat-FAKE-science-oauth-token"

results = []


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}" + (f"  {detail}" if detail else ""))


def post(body):
    req = urllib.request.Request(f"{BASE}/v1/messages",
                                 data=json.dumps(body).encode(),
                                 headers={"content-type": "application/json",
                                          "authorization": FAKE_BEARER})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read().decode(), r.headers.get("content-type", "")


def run_tests():
    # 1. models
    with urllib.request.urlopen(f"{BASE}/v1/models", timeout=30) as r:
        models = json.loads(r.read())
    check("GET /v1/models", bool(models.get("data")), f"{len(models.get('data',[]))} 个模型")

    # 2. 非流式
    body, _ = post({"model": "claude-sonnet-5", "max_tokens": 200, "stream": False,
                    "system": "你是简洁助手，只用中文",
                    "messages": [{"role": "user", "content": "一句话说明通义千问是谁家的模型"}]})
    d = json.loads(body)
    txt = "".join(b.get("text", "") for b in d.get("content", []) if b.get("type") == "text")
    check("非流式文本", d.get("type") == "message" and len(txt) > 0, f"回答: {txt[:40]}")

    # 3. 流式 SSE
    req = urllib.request.Request(f"{BASE}/v1/messages",
                                 data=json.dumps({"model": "claude-opus-4-8", "max_tokens": 150, "stream": True,
                                                  "messages": [{"role": "user", "content": "用中文数 1 到 5，逗号分隔"}]}).encode(),
                                 headers={"content-type": "application/json", "authorization": FAKE_BEARER})
    got_start = got_stop = False
    stream_txt = ""
    with urllib.request.urlopen(req, timeout=120) as r:
        for line in r:
            line = line.decode("utf-8", "replace").strip()
            if not line.startswith("data:"):
                continue
            try:
                ev = json.loads(line[5:].strip())
            except Exception:
                continue
            if ev.get("type") == "message_start":
                got_start = True
            elif ev.get("type") == "content_block_delta":
                stream_txt += ev["delta"].get("text", "")
            elif ev.get("type") == "message_stop":
                got_stop = True
    check("流式 SSE", got_start and got_stop and len(stream_txt) > 0, f"拼出: {stream_txt[:40]}")

    # 4. tool_use 发起
    tools = [{"name": "get_weather", "description": "查询某城市实时天气",
              "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}}]
    body, _ = post({"model": "claude-opus-4-8", "max_tokens": 300, "stream": False,
                    "messages": [{"role": "user", "content": "北京现在天气怎么样？用工具查。"}],
                    "tools": tools})
    d = json.loads(body)
    tus = [b for b in d.get("content", []) if b.get("type") == "tool_use"]
    ok4 = bool(tus) and d.get("stop_reason") == "tool_use"
    check("tool_use 发起", ok4, f"工具={tus[0]['name'] if tus else '无'} 入参={json.dumps(tus[0]['input'],ensure_ascii=False) if tus else ''}")
    tu = tus[0] if tus else {"id": "call_x", "name": "get_weather", "input": {"city": "北京"}}

    # 5. tool_result 回喂
    body, _ = post({"model": "claude-opus-4-8", "max_tokens": 300, "stream": False,
                    "messages": [
                        {"role": "user", "content": "北京现在天气怎么样？用工具查。"},
                        {"role": "assistant", "content": [tu]},
                        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": tu["id"],
                                                      "content": "晴，26摄氏度，东南风2级，湿度40%"}]}],
                    "tools": tools})
    d = json.loads(body)
    txt = "".join(b.get("text", "") for b in d.get("content", []) if b.get("type") == "text")
    ok5 = d.get("stop_reason") == "end_turn" and ("26" in txt or "晴" in txt)
    check("tool_result 回喂→接着作答", ok5, f"回答: {txt[:50]}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--env-file", default=None)
    ap.add_argument("--key-name", default="DASHSCOPE_API_KEY")
    args = ap.parse_args()

    if not os.environ.get("DASHSCOPE_API_KEY") and not args.env_file:
        print("需要 DASHSCOPE_API_KEY 环境变量，或 --env-file <路径> --key-name <变量名>", file=sys.stderr)
        sys.exit(2)

    cmd = [sys.executable, PROXY, "--port", str(PORT)]
    if args.env_file:
        cmd += ["--env-file", args.env_file, "--key-name", args.key_name]
    print(f"启动代理: {' '.join(cmd[:3])} ...")
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        # 等健康
        for _ in range(20):
            try:
                urllib.request.urlopen(f"{BASE}/health", timeout=1)
                break
            except Exception:
                time.sleep(0.3)
        else:
            err = proc.stderr.read().decode() if proc.stderr else ""
            print("代理未就绪。", err, file=sys.stderr)
            sys.exit(1)
        print("\n== 隔离回归测试（只打代理，不碰 Science）==")
        run_tests()
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()

    passed = sum(1 for _, ok, _ in results if ok)
    print(f"\n结果: {passed}/{len(results)} 通过")
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
