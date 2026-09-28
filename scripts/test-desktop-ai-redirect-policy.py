#!/usr/bin/env python3
"""Desktop AI requests must not replay bearer tokens across redirects."""

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    source = (ROOT / "apps/desktop/src-tauri/src/ai.rs").read_text()
    expected = "redirect(reqwest::redirect::Policy::none())"
    if source.count(expected) < 2:
        raise SystemExit("desktop AI model and test requests must disable redirects")
    print("desktop AI requests reject redirects")


if __name__ == "__main__":
    main()
