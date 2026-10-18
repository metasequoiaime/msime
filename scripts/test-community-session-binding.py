#!/usr/bin/env python3
"""Community resource sheets must send every request through the account they were opened for.

Since #8201 (migration plan 4c) the shared 词包与回复模板 pages call Rust's msime_client_community and msime_client_cloud_dictionary through CommunityResourceAccount, which reads status before and after each request and drops the result when the account changed. A sheet that went back to a Swift session or token, or a CommunityResourceAccount that stopped checking, would let one account's request finish against another.
"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "shared/backend-ui/dictionary/BackendCommunityResourcesView.swift").read_text(
    encoding="utf-8"
)

account_start = SOURCE.index("struct CommunityResourceAccount")
account = SOURCE[account_start:SOURCE.index("struct BackendCommunityResourcesView", account_start)]
detail_start = SOURCE.index("private struct CommunityResourceDetailView")
editor_start = SOURCE.index("private struct BackendCommunityResourceEditor")
detail = SOURCE[detail_start:editor_start]
editor = SOURCE[editor_start:SOURCE.index("private extension View", editor_start)]
parent = SOURCE[account_start:detail_start]

checked_start = account.index("private func checked")
checked = account[checked_start:]
required = {
    "requests check the account before sending":
        checked.index("rust.status()?.id == accountID") < checked.index("try await body(rust)"),
    "requests check the account after the answer":
        checked.rindex("rust.status()?.id == accountID") > checked.index("try await body(rust)"),
    "a changed account cancels": "throw CancellationError()" in checked,
    "detail receives the bound account": "let account: CommunityResourceAccount" in detail,
    "editor receives the bound account": "let account: CommunityResourceAccount" in editor,
    "sheets are handed the parent's account": parent.count("account: account)") >= 2,
    "detail actions go through the account": detail.count("account.community(") >= 4,
    "editor actions go through the account": "account.community(" in editor and "account.cloudDictionary(" in editor,
}
for name, section in (("parent", parent), ("detail", detail), ("editor", editor)):
    required[f"{name} holds no Swift session"] = "BackendAccountSession" not in section
    required[f"{name} sends no token"] = "token" not in section

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("community sheets do not stay on their account: " + ", ".join(missing))

print("community resource sheets send every request through their bound account")
