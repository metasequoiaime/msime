#!/usr/bin/env python3
"""Importing a community word pack must apply to the account the user confirmed.

The button reads the signed-in account when it is pressed and remembers it in the preview; the confirmation sends resource_apply through a CommunityResourceAccount bound to that account (#8201 migration plan 4c), so an account switch between the two cancels instead of importing into the new account.
"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "shared/backend-ui/dictionary/BackendCommunityResourcesView.swift").read_text(
    encoding="utf-8"
)
start = SOURCE.index("struct CommunityCloudImportButton")
body_start = SOURCE.index("  var body:", start)
body_end = SOURCE.index("  private func run", body_start)
body = SOURCE[body_start:body_end]

required = {
    "preview remembers the account it was opened for": "Preview(resourceID: id, resourceRevision: version, accountID: current)" in body,
    "preview refuses another page's account": "if let accountID, accountID != current { throw CancellationError() }" in body,
    "confirmation is bound to the previewed account":
        "CommunityResourceAccount(accountID: selected.accountID, rust: rust)" in body,
    "confirmation applies the viewed revision":
        '"operation": "resource_apply"' in body and '"resource_revision": selected.resourceRevision' in body,
    "import flow holds no Swift session or token": "BackendAccountSession" not in body and "token" not in body,
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("community cloud import is not bound to its account: " + ", ".join(missing))

print("community cloud import applies to the account the user confirmed")
