#!/usr/bin/env python3
"""Keep Android settings pages on the shared keyboard height policy."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OPTIONS = ROOT / "platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java"
FRAGMENT = ROOT / "platforms/android/java/app/msime/android/home/KeyboardFragment.java"
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"


def require(text: str, needle: str, path: Path) -> None:
    if needle not in text:
        raise AssertionError(f"{path}: missing {needle!r}")


def main() -> None:
    options = OPTIONS.read_text()
    fragment = FRAGMENT.read_text()
    ui = UI.read_text()

    require(options, "import app.msime.android.KeyboardHeightPolicy;", OPTIONS)
    require(fragment, "import app.msime.android.KeyboardHeightPolicy;", FRAGMENT)
    require(options, "KeyboardHeightPolicy::displayPercent", OPTIONS)
    require(options, "KeyboardSpacingPolicy::display, value -> savePreference(\"touch_key_spacing_tenths\", value)", OPTIONS)
    require(options, "KeyboardSpacingPolicy::display, value -> savePreference(\"touch_row_spacing_tenths\", value)", OPTIONS)
    if "KeyboardGeometry::displayPercent" in options:
        raise AssertionError(f"{OPTIONS}: stale KeyboardGeometry::displayPercent reference")
    if "KeyboardGeometry::display" in options:
        raise AssertionError(f"{OPTIONS}: stale KeyboardGeometry::display reference")
    if "import app.msime.android.KeyboardGeometry;" in fragment:
        raise AssertionError(f"{FRAGMENT}: stale KeyboardGeometry import")
    require(ui, "import androidx.annotation.ColorInt;", UI)


if __name__ == "__main__":
    main()
