#!/usr/bin/env python3
"""macOS cloud dictionary child views must use the owning account session."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PARENT = (ROOT / "platforms/macos/src/backend/dictionary/BackendDictionaryView.swift").read_text(encoding="utf-8")
CATALOG = (ROOT / "shared/backend-ui/dictionary/CloudDictionaryCatalogView.swift").read_text(encoding="utf-8")
CANDIDATES = (ROOT / "shared/backend-ui/candidate/CloudCandidatesView.swift").read_text(encoding="utf-8")

required = {
    "parent passes account session to catalog":
        "CloudDictionaryCatalogView(kind: model.kind, authorize: { try await model.authorize() }," in PARENT
        and ("accountID: model.accountID, session: model.account" in PARENT
             or "accountID: model.cloudAccountID, session: model.cloudAccountSession" in PARENT),
    "parent passes account session to candidates":
        "CloudCandidatesView(kind: model.kind, authorize: { try await model.authorize() }," in PARENT
        and ("accountID: model.accountID, session: model.account" in PARENT
             or "accountID: model.cloudAccountID, session: model.cloudAccountSession" in PARENT),
    "catalog authenticates through session":
        "session.authenticated(matchingUserID: identity.userID" in CATALOG,
    "candidates authenticate through session":
        "session.authenticated(matchingUserID: identity.userID" in CANDIDATES,
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("missing macOS cloud dictionary session binding: " + ", ".join(missing))

print("macOS cloud dictionary child views stay bound to the account session")
