#!/usr/bin/env python3
"""检查候选翻译的展示接线，包含所有候选快照中的来源过滤与显示开关。"""

from pathlib import Path
import re


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    session = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text()
    view = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets").read_text()
    ability = (
        root
        / "platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
    ).read_text()
    metrics = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardMetrics.ts").read_text()

    # 注释不能代替接线；三个候选快照都必须同时传来源过滤结果和显示开关。
    presentation = re.sub(r"/\*.*?\*/|//[^\n]*", "", session, flags=re.DOTALL)
    gloss_calls = re.findall(
        r"CandidateGlossPolicy\.(annotation|annotationIsTranslation)\(\s*"
        r"CandidateGlossPolicy\.slotAnnotation\(hanja,\s*candidate\.annotation\),\s*"
        r"TranslationPolicy\.sourceGetsNoGloss\(candidate\.source\)\s*"
        r"\?\s*null\s*:\s*candidate\.translation\s*\?\?\s*null,\s*"
        r"this\.candidateGlossEnabled\s*\)",
        presentation,
    )
    required = {
        "independent online display switch": "this.candidateGlossEnabled" in session
        and "preferences.candidate_translations" in session
        and gloss_calls.count("annotation") == 3
        and gloss_calls.count("annotationIsTranslation") == 3,
        "pre-arrival row decision": "CandidateGlossLayoutPolicy.rows" in session
        and "glossRows(): number" in session,
        "word-sized unglossed chips": re.search(
            r"if\s*\(!KeyboardSession\.shared\.glossesExpected\(\)\)\s*\{\s*"
            r"return CandidateLayoutPolicy\.chipWidth\(this\.candidateGeometry\(\),\s*"
            r"this\.settings\.candidateFontSize,\s*word,\s*padding\);",
            view,
        ) is not None,
        "dedicated horizontal gloss line": "candidateUsesGlossLine()" in view
        and "this.candidateGlossLine(candidate)" in view
        and "CandidateGlossPolicy.glossLine(candidate.hunEum" in view,
        "fixed gloss height": "CANDIDATE_GLOSS_LINE_HEIGHT_VP: number = 14" in metrics
        and "KeyboardMetrics.glossHeightVp" in view,
        "native panel resize": "KeyboardSession.shared.glossRows()" in ability,
    }
    problems = [name for name, present in required.items() if not present]
    if problems:
        for problem in problems:
            print(f"missing candidate translation wiring: {problem}")
        return 1
    print("harmony candidate translation: online display and stable gloss rows are wired")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
