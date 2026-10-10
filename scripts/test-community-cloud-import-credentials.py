#!/usr/bin/env python3
"""Cloud community imports must use the credentials supplied by the button."""

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
    "preview catalog uses injected token":
        "dictionaryCatalog(.quick, code: \"\", token: identity.token)" in body,
    "confirmation applies with injected token":
        "applyResource(selected.resourceID, resourceRevision: selected.resourceRevision," in body
        and "dictionaryRevision: selected.dictionaryRevision, token: identity.token" in body,
    "import flow does not switch to the shared session":
        "BackendAccountSession.shared" not in body,
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("community cloud import credential binding missing: " + ", ".join(missing))

print("community cloud import uses its injected account credentials")
