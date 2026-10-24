#!/usr/bin/env python3
"""Harmony 词库刷新不能在修改完成后发布过期结果。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "packages/ui/src/dictionary/use-dictionary-collections-state.ts"
TEXT = SOURCE.read_text(encoding="utf-8")
PAGE = (ROOT / "packages/ui/src/settings/harmony-phone-dictionary.tsx").read_text(encoding="utf-8")

required = {
    "page consumes shared collection state":
        'from "../dictionary/use-dictionary-collections-state"' in PAGE
        and re.search(r"=\s*useDictionaryCollectionsState\(\{", PAGE) is not None,
    "file preparation captures the current owner":
        "const isCurrent = captureOwner();" in PAGE
        and "const current = runner.generation.current;" in TEXT
        and "return () => mounted.current && current === runner.generation.current;" in TEXT,
    "reload owns a generation":
        "const reloadGeneration = useAsyncGeneration(collections, dictionary, galleryOpen);" in TEXT,
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
