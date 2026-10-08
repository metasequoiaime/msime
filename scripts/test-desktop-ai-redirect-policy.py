#!/usr/bin/env python3
"""Desktop AI requests must not replay bearer tokens across redirects."""

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    # 桌面的模型列表与测试请求、iOS 移动端请求都经 client-core 的 `ai::endpoint::blocking_client_builder` 建客户端，重定向策略在那里统一关闭。
    builder = (ROOT / "crates/client-core/src/ai/endpoint.rs").read_text()
    start = builder.index("pub fn blocking_client_builder")
    body = builder[start : builder.index("\n}\n", start)]
    if "redirect(reqwest::redirect::Policy::none())" not in body:
        raise SystemExit("ai::endpoint::blocking_client_builder must disable redirects")
    for path, minimum in (
        ("apps/desktop/src-tauri/src/ai.rs", 2),
        ("apps/desktop/src-tauri/src/shared/mobile_ai.rs", 2),
    ):
        source = (ROOT / path).read_text()
        if source.count("ai::endpoint::blocking_client_builder(") < minimum:
            raise SystemExit(f"{path}: AI model and test requests must use ai::endpoint::blocking_client_builder")
        if "reqwest::blocking::Client::builder()" in source:
            raise SystemExit(f"{path}: build AI clients with ai::endpoint::blocking_client_builder, which disables redirects")
    print("desktop AI requests reject redirects")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
