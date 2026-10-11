#!/usr/bin/env python3
"""Community resource sheets must resolve the current account session per action."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "shared/backend-ui/dictionary/BackendCommunityResourcesView.swift").read_text(
    encoding="utf-8"
)

detail_start = SOURCE.index("private struct CommunityResourceDetailView")
editor_start = SOURCE.index("private struct BackendCommunityResourceEditor")
detail = SOURCE[detail_start:editor_start]
editor = SOURCE[editor_start:SOURCE.index("private extension View", editor_start)]
parent = SOURCE[:detail_start]

required = {
    "detail receives a current-session resolver":
        "let credentials: @MainActor () async throws -> (userID: String, token: String, sessionID: UUID)" in detail,
    "editor receives a current-session resolver":
        "let credentials: @MainActor () async throws -> (userID: String, token: String, sessionID: UUID)" in editor,
    "detail sheet receives the resolver": "session: BackendAccountSession.shared, credentials: authorize" in parent,
    "editor sheet receives the resolver": parent.count("session: BackendAccountSession.shared, credentials: authorize") >= 2,
    "parent does not keep a frozen session ID": "@State private var sessionID: UUID?" not in parent,
    "detail does not freeze a session ID": "let sessionID: UUID?" not in detail,
    "editor does not freeze a session ID": "let sessionID: UUID?" not in editor,
    "detail actions resolve credentials": detail.count("credentials()") > 0,
    "editor actions resolve credentials": editor.count("credentials()") > 0,
    "detail forwards the resolved session": "matchingSessionID: identity.sessionID" in detail,
    "editor forwards the resolved session": "matchingSessionID: identity.sessionID" in editor,
    "detail never forwards a frozen session": "matchingSessionID: sessionID" not in detail,
    "editor never forwards a frozen session": "matchingSessionID: sessionID" not in editor,
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("community sheets freeze an old account session: " + ", ".join(missing))

print("community resource sheets resolve the current account session")
