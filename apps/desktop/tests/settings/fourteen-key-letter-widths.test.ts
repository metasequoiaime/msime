import { expect, test } from "vitest";
import { withFourteenKeyLetterWidths } from "../../../../packages/ui/src/keyboard/fourteen-key-letter-widths";
import { harmonyTouchKeyboardRows } from "../../../../packages/ui/src/keyboard/keyboard-layouts";
import { FourteenKeyLayout } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/input/FourteenKeyLayout";
import { EighteenKeyLayout } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/input/EighteenKeyLayout";
import { fourteenKeyLetterWidthPlan } from "../../../../platforms/harmony/entry/src/main/ets/keyboard/input/FourteenKeyLetterWidths";

test.each(["fourteen_key", "eighteen_key"] as const)(
  "%s shuangpin preview keeps the native single-letter width for L;",
  (layout) => {
    const rows = layout === "fourteen_key" ? FourteenKeyLayout.rows() : EighteenKeyLayout.rows();
    for (const mode of ["equal", "varied"] as const) {
      const native = fourteenKeyLetterWidthPlan(rows, mode, 1.4);
      for (const face of [layout, `${layout}_shuangpin`] as const) {
        const preview = withFourteenKeyLetterWidths(
          harmonyTouchKeyboardRows[face].slice(0, 3),
          mode,
        );
        expect(preview.map((row) => row.map((key) => key.weight))).toEqual(
          native.map((row) => row.map((cell) => cell.weight)),
        );
        if (mode === "varied") {
          expect(preview[1].find((key) => key.label.startsWith("L"))?.weight).toBe(0.75);
        }
      }
    }
  },
);
