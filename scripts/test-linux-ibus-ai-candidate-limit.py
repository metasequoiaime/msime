#!/usr/bin/env python3
"""IBus 的在线查询守卫不能拿存下的 AI 候选数量去比 host-api 给的固定值。

IBus 宿主的 `online_dispatch` 把读到的偏好快照里的 `ai_assistant` 和 `msime_client_online_query` 返回的 `ai_assistant` 逐键比较，有一键不等就把 AI 配置从查询里删掉，防止推迟应用的偏好改动让请求带着旧设置发出。Linux 进了 `HostPlatform::fixed_ai_candidate_limit` 以后，查询里的 `candidate_limit` 永远是固定值，快照里却是文档存的原值；存的不是 3 时这一键永远不等，AI 请求就再也发不出去。

真要复现得起一个 IBus 会话，本地门禁没有这一步，所以这里做静态检查：只要 client-core 让 Linux 固定候选数量，比较的键里就不能有 `candidate_limit`。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOST_SURFACE = ROOT / "crates/client-core/src/host_surface.rs"
SOURCE = ROOT / "platforms/linux/src/core/ClientEngine.cpp"

surface = HOST_SURFACE.read_text(encoding="utf-8")
fixed = re.search(
    r"pub fn fixed_ai_candidate_limit\(self\) -> Option<u8> \{(?P<body>.*?)\n    \}",
    surface,
    re.DOTALL,
)
if not fixed:
    sys.exit(f"missing HostPlatform::fixed_ai_candidate_limit in {HOST_SURFACE}")
linux_fixed = "HostPlatform::Linux" in fixed.group("body")

text = SOURCE.read_text(encoding="utf-8")
dispatch = re.search(
    r"void online_dispatch\(IBusEngine \*engine,[^)]*\)\s*\{(?P<body>.*?)\n\}\n",
    text,
    re.DOTALL,
)
if not dispatch:
    sys.exit(f"missing online_dispatch in {SOURCE}")
keys = re.search(
    r"for \(const auto \*key : \{(?P<keys>[^}]*)\}\)",
    dispatch.group("body"),
)
if not keys:
    sys.exit("online_dispatch no longer compares the stored AI settings with the online query key by key")
compared = re.findall(r'"([a-z_0-9]+)"', keys.group("keys"))
if "provider" not in compared or "model" not in compared:
    sys.exit(f"online_dispatch AI settings comparison lost its provider keys: {compared}")

if linux_fixed and "candidate_limit" in compared:
    sys.exit(
        "online_dispatch compares ai_assistant.candidate_limit, but host-api fixes it on Linux "
        "(HostPlatform::fixed_ai_candidate_limit): a stored limit other than the fixed one "
        "drops ai_assistant from every provider query"
    )

print(
    "linux IBus online dispatch: AI settings guard "
    + ("skips the fixed candidate_limit" if linux_fixed else "follows the stored candidate_limit")
)
