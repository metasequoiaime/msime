#!/usr/bin/env python3
"""Harmony dictionary reloads must not publish stale results after a mutation."""

from pathlib import Path


SOURCE = Path(__file__).resolve().parents[1] / "packages/ui/src/settings/harmony-phone-dictionary.tsx"
TEXT = SOURCE.read_text(encoding="utf-8")

required = {
    "reload owns a generation": "const reloadGeneration = useRef(0);" in TEXT,
    "reload captures its generation": "const current = ++reloadGeneration.current;" in TEXT,
    "builtin count is generation guarded":
        ".then(" in TEXT and "current === reloadGeneration.current && setBuiltinCount(count)" in TEXT,
    "collection result is generation guarded":
        "if (!mounted.current || current !== reloadGeneration.current) return;" in TEXT,
    "mutation invalidates pending reload": "reloadGeneration.current++;" in TEXT,
}

missing = [name for name, present in required.items() if not present]
if missing:
    raise SystemExit("missing Harmony dictionary reload generation guard: " + ", ".join(missing))

print("harmony dictionary reloads reject stale results")
