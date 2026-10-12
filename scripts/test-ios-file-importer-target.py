#!/usr/bin/env python3
"""Keep the iOS file pickers' "who asked" state apart from their isPresented flag.

SwiftUI sets a fileImporter's isPresented to false before it calls onCompletion. A picker presented through a derived binding such as `Binding(get: { target != nil }, set: { if !$0 { target = nil } })` therefore clears `target` before the completion reads it, and the completion falls back to a default. That is how an import started from the 双拼 section of the helpcode page was selected into 全拼 (#7176). Every fileImporter/fileExporter under platforms/ios must be presented by a plain Bool state, with the target kept in its own state, as LocalSpeechModelsView (importTarget + importPickerShown) and CloudDictionaryFilesView (choosingSnapshot + choosing) do.
"""

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
IOS = ROOT / "platforms/ios"
PICKER = re.compile(r"\.(fileImporter|fileExporter|fileMover)\(\s*isPresented:\s*([^,]+),")
HELPCODE = IOS / "App/Sources/settings/HelpcodeSettingsView.swift"


def main() -> None:
    if not IOS.is_dir():
        print("skip: platforms/ios is missing")
        return
    failures = []
    pickers = 0
    for path in sorted(IOS.rglob("*.swift")):
        if "/build/" in path.as_posix() or "/Pods/" in path.as_posix():
            continue
        source = path.read_text()
        for match in PICKER.finditer(source):
            pickers += 1
            binding = match.group(2).strip()
            if not binding.startswith("$"):
                line = source.count("\n", 0, match.start()) + 1
                failures.append(f"{path.relative_to(ROOT)}:{line}: .{match.group(1)} isPresented is `{binding}`, not a plain $state flag")
    if pickers == 0:
        failures.append("found no file pickers under platforms/ios; the pattern no longer matches the sources")

    helpcode = HELPCODE.read_text()
    start = helpcode.index(".fileImporter(")
    importer = helpcode[start:helpcode.index(".alert(", start)]
    if "isPresented: $importing" not in importer or "into: importScheme" not in importer:
        failures.append(f"{HELPCODE.relative_to(ROOT)}: the helpcode import must open on $importing and import into importScheme")
    if re.search(r"importScheme\s*=\s*nil", helpcode):
        failures.append(f"{HELPCODE.relative_to(ROOT)}: importScheme must not be cleared, the completion reads it after the picker closes")

    if failures:
        raise SystemExit("\n".join(failures))
    print(f"iOS file pickers ({pickers}) keep their target apart from isPresented")


if __name__ == "__main__":
    main()
