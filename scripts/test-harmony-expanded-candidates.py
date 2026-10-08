#!/usr/bin/env python3
"""Guard the expanded-candidate width contract taken from the Apple Japanese tests."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    view = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets").read_text()
    policy = (
        root
        / "platforms/harmony/entry/src/main/ets/keyboard/candidate/ExpandedCandidateLayout.ts"
    ).read_text()
    start = view.index("  expandedFace()")
    # 格子在 expandedGrid() 里画，整块面板和全拼九键三栏面板的中栏共用，所以一直看到它的末尾。
    grid = view.index("  expandedGrid()", start)
    expanded = view[start : view.index("\n  @Builder\n", grid)]
    required = {
        "shared measured width": "ExpandedCandidateLayout.width" in view
        and ".width(this.expandedCandidateWidth(index))" in expanded,
        "visible-width cap": "Math.min(natural, availableWidth)" in policy,
        "single-line candidate": "Text(this.expanded[index].text)" in expanded
        and ".maxLines(1)" in expanded
        and "TextOverflow.Ellipsis" in expanded,
        "constrained text column": ".layoutWeight(1)" in expanded,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing expanded candidate guard: {problem}")
        return 1
    print("harmony expanded candidates: measured cells stay single-line and within the panel")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
