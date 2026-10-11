#!/usr/bin/env python3
"""Desktop AI requests must not replay bearer tokens across redirects."""

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    # 桌面的模型列表与测试请求、iOS 移动端请求都经 client-core 的 `ai::endpoint::blocking_client_builder` 建客户端，重定向策略在那里统一关闭。桌面两个请求的实现在 client-core 的 `ai::settings`（原生 Windows 设置窗口经 C 接口共用），`ai.rs` 只转发给它。
    builder = (ROOT / "crates/client-core/src/ai/endpoint.rs").read_text()
    start = builder.index("pub fn blocking_client_builder")
    body = builder[start : builder.index("\n}\n", start)]
    if "redirect(reqwest::redirect::Policy::none())" not in body:
        raise SystemExit("ai::endpoint::blocking_client_builder must disable redirects")
    for path, call, minimum in (
        ("crates/client-core/src/ai/settings.rs", "endpoint::blocking_client_builder(", 1),
        ("apps/desktop/src-tauri/src/shared/mobile_ai.rs", "ai::endpoint::blocking_client_builder(", 2),
    ):
        source = (ROOT / path).read_text()
        if source.count(call) < minimum:
            raise SystemExit(f"{path}: AI model and test requests must use ai::endpoint::blocking_client_builder")
        if "reqwest::blocking::Client::builder()" in source:
            raise SystemExit(f"{path}: build AI clients with ai::endpoint::blocking_client_builder, which disables redirects")
    desktop = (ROOT / "apps/desktop/src-tauri/src/ai.rs").read_text()
    for call in ("ai::settings::fetch_models(", "ai::settings::test_polish("):
        if call not in desktop:
            raise SystemExit(f"apps/desktop/src-tauri/src/ai.rs: the desktop AI commands must delegate to {call}")
    if "reqwest::" in desktop:
        raise SystemExit("apps/desktop/src-tauri/src/ai.rs: build AI clients in client-core's ai::settings, which disables redirects")
    print("desktop AI requests reject redirects")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
