#!/usr/bin/env python3
"""macOS 云词库的子页面（候选、目录）必须绑定在父页面的账号上。

#8201 4c 以后会话在 Rust 里，C 上拿不到会话代次：父页面把打开时的 user.id 和注册好的 Rust 账号交给子页面，子页面每个请求都经同一个 `request`，在请求前后核对 user.id，退出或换号时结果作废（CancellationError），不会落到另一个账号的页面上。
"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
PARENT = (ROOT / "platforms/macos/src/backend/dictionary/BackendDictionaryView.swift").read_text(encoding="utf-8")
CATALOG = (ROOT / "shared/backend-ui/dictionary/CloudDictionaryCatalogView.swift").read_text(encoding="utf-8")
CANDIDATES = (ROOT / "shared/backend-ui/candidate/CloudCandidatesView.swift").read_text(encoding="utf-8")

CHECKED_REQUEST = re.compile(
    r"private func request\(_ action: \[String: Any\]\) async throws -> Data \{\n"
    r"\s*guard let rust else \{ throw BackendRustClient\.Failure\.unavailable \}\n"
    r"\s*guard try await rust\.status\(\)\?\.id == accountID else \{ throw CancellationError\(\) \}\n"
    r"\s*let data = try await rust\.cloudDictionary\(action\)\n"
    r"\s*guard try await rust\.status\(\)\?\.id == accountID else \{ throw CancellationError\(\) \}\n"
)


def checked(source: str) -> bool:
    # 只有那一个 request 直接调 Rust，别的操作都经它。
    return bool(CHECKED_REQUEST.search(source)) and source.count("cloudDictionary(") == 1


required = {
    "parent passes account to catalog":
        "CloudDictionaryCatalogView(kind: model.kind, accountID: model.cloudAccountID, rust: model.cloudRust)" in PARENT,
    "parent passes account to candidates":
        "CloudCandidatesView(kind: model.kind, accountID: model.cloudAccountID, rust: model.cloudRust)" in PARENT,
    "catalog checks the account around every request": checked(CATALOG),
    "candidates check the account around every request": checked(CANDIDATES),
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("missing macOS cloud dictionary session binding: " + ", ".join(missing))

print("macOS cloud dictionary child views stay bound to the account")
