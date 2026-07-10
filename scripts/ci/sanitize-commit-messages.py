#!/usr/bin/env python3
"""Message callback for git filter-repo: strip legacy brand/fork names from commit subjects."""
import re
import sys

# git filter-repo sets `message` (bytes); script must assign back to `message`.
text = message.decode("utf-8", errors="replace")

text = re.sub(
    r"Merge pull request #(\d+) from \S+/\S+",
    r"Merge pull request #\1",
    text,
)

pairs = [
    ("Qwen, SiliconFlow, and CSSwitch", "legacy names"),
    ("Qwen/SiliconFlow", "legacy provider names"),
    ("Qwen and SiliconFlow", "legacy provider names"),
    ("Qwen test class", "legacy provider test class"),
    ("non deepseek/qwen", "non-default profile sources"),
    ("for qwen path", "for legacy provider path"),
    ("Qwen", "legacy provider"),
    ("SiliconFlow", "legacy provider"),
    ("DashScope", "legacy API"),
    ("dashscope", "legacy-api"),
    ("DASHSCOPE", "legacy API"),
    ("CSSWITCH", "CSP"),
    ("CSSwitch", "CSP"),
    ("csswitch", "csp"),
    ("CC Switch", "prior multi-API switcher"),
    ("cc-switch", "external-reference"),
    ("SuperJJ007", "contributor"),
    ("硅基", "relay"),
]
for old, new in pairs:
    text = text.replace(old, new)

message = text.encode("utf-8")
