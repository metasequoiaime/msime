#!/usr/bin/env python3
"""Guard the candidate-translation presentation wiring found by the Apple assertion audit."""

from pathlib import Path


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    session = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets").read_text()
    view = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets").read_text()
    ability = (
        root
        / "platforms/harmony/entry/src/main/ets/inputmethodextability/KeyboardExtensionAbility.ets"
    ).read_text()
    metrics = (root / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardMetrics.ts").read_text()

    required = {
        "independent online display switch": "this.candidateGlossEnabled" in session
        and "preferences.candidate_translations" in session
        and "candidate.translation ?? null, this.candidateGlossEnabled" in session,
        "pre-arrival row decision": "CandidateGlossLayoutPolicy.rows" in session
        and "glossRows(): number" in session,
        "word-sized unglossed chips": "CandidateChipWidth.chip(word, false, 0, padding)" in view,
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
