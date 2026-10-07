#!/usr/bin/env python3
"""Keep bounded community-library reads off the Android IME main thread."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/android/java/app/msime/android/core/ImePanels.java").read_text()


def body(start: str, end: str) -> str:
    begin = SOURCE.index(start)
    finish = SOURCE.index(end, begin)
    return SOURCE[begin:finish]


def main() -> None:
    reply_generation = body("    void generateReply(", "    void useReply(")
    reply_menu = body("    void showReplyTemplates()", "    void showSkinMenu(")
    skin_picker = body("    void renderSkinPicker()", "    private static String choiceKey(")
    for name, value in (("generateReply", reply_generation), ("showReplyTemplates", reply_menu),
                        ("renderSkinPicker", skin_picker)):
        ui_prefix = value.split("s.preferencesWorker.execute(() ->", 1)[0]
        if ".read()" in ui_prefix or "CustomSkinLibrary.read" in ui_prefix or "CommunitySkinCache.read" in ui_prefix:
            raise SystemExit(f"{name} performs synchronous community-library I/O on the UI path")
    if SOURCE.count("s.preferencesWorker.execute(() ->") < 3:
        raise SystemExit("community UI reads must use preferencesWorker")
    print("Android community UI library reads are dispatched to preferencesWorker")


if __name__ == "__main__":
    main()
