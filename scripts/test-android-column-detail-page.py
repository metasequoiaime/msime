#!/usr/bin/env python3
"""Check shared Android detail-page content-column ownership."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"


def main() -> int:
    errors = []
    base = HOME / "ColumnDetailPage.java"
    if not base.exists():
        errors.append("ColumnDetailPage: missing shared content-column owner")
    else:
        source = base.read_text(encoding="utf-8")
        for snippet in (
            "public abstract class ColumnDetailPage extends DetailPage",
            "@Nullable private LinearLayout column;",
            "@Override protected final void buildContent(LinearLayout column, Bundle args)",
            "this.column = column;",
            "onColumnReady(args);",
            "protected abstract void onColumnReady(Bundle args);",
            "@Override public void onDestroyView()",
            "column = null;",
            "onContentDestroyed();",
            "protected void onContentDestroyed()",
            "@Nullable protected final LinearLayout contentColumn()",
        ):
            if snippet not in source:
                errors.append(f"ColumnDetailPage: missing {snippet}")

    editor = (HOME / "ClipboardEditPage.java").read_text(encoding="utf-8")
    for snippet in (
        "public final class ClipboardEditPage extends ColumnDetailPage",
        "@Override protected void onColumnReady(Bundle args)",
        "if (state == State.LOADING) load(args.getString(ClipboardHistoryPolicy.EDIT_ENTRY_ARG));",
        "@Override protected void onContentDestroyed()",
        "save = null;",
        "saving = false;",
        "LinearLayout target = contentColumn();",
    ):
        if snippet not in editor:
            errors.append(f"ClipboardEditPage: missing {snippet}")

    ai_skin = (HOME / "AiSkinPage.java").read_text(encoding="utf-8")
    for snippet in (
        "public final class AiSkinPage extends ColumnDetailPage",
        "@Override protected void onColumnReady(Bundle args)",
        "current.observer = this::onStateChanged;",
        "@Override protected void onContentDestroyed()",
        "current.observer = null;",
        "saving = false;",
        "if (!isAdded() || contentColumn() == null) return;",
        "LinearLayout target = contentColumn();",
    ):
        if snippet not in ai_skin:
            errors.append(f"AiSkinPage: missing {snippet}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android detail pages share content-column ownership")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
