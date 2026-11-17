// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render } from "@testing-library/react";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import { ScreenKeyboardPreview } from "../../../../packages/ui/src/keyboard/screen-keyboard-preview";
import {
  harmonyTouchRowsForPreferences,
  keyLayoutPreviewRows,
} from "../../../../packages/ui/src/settings/touch-key-layout-settings";

afterEach(cleanup);

test.each(["fourteen_key", "eighteen_key"] as const)(
  "%s 的全拼和双拼预览在非零键距下共用三排字母单位",
  (layout) => {
    for (const scheme of [layout, "xiaohe"] as const) {
      const preferences: Preferences = {
        scheme: "quanpin",
        shuangpin_profile: "xiaohe",
        candidate_page_size: 5,
        learning: true,
        chinese_punctuation: true,
        touch_shuangpin_layout: layout,
        touch_fourteen_key_letter_widths: "varied",
        touch_number_row: true,
      };
      const rows = harmonyTouchRowsForPreferences(preferences, scheme);
      const panel = keyLayoutPreviewRows("fourteen", preferences, scheme);
      expect(panel.map((row) => row.keys[0]?.letterPitch === true)).toEqual([
        false,
        true,
        true,
        true,
        false,
      ]);
      for (const gap of [3, 4.5, 6]) {
        const { container, unmount } = render(
          <ScreenKeyboardPreview
            theme="light"
            phone
            touchRows={rows}
            keySpacingTenths={gap * 10}
          />,
        );
        const units: number[] = [];
        for (let at = 1; at <= 3; at++) {
          const row = container.querySelector(`[data-keyboard-row='${at}']`)!;
          let previousRight: number | undefined;
          for (const key of rows[at].filter((key) => key.label)) {
            const drawn = [...row.querySelectorAll("[data-keyboard-key]")].find(
              (node) => node.getAttribute("data-keyboard-key") === key.label,
            )!;
            const center = Number(drawn.querySelector("text")!.getAttribute("x"));
            // 圆角键路径的第一条 Q 指向键帽右边缘，结合文字中心求出实际绘制宽度。
            const path = drawn.querySelector("path")!.getAttribute("d")!;
            const right = Number(path.match(/Q([\d.e+-]+) /)![1]);
            const width = 2 * (right - center);
            const left = right - width;
            units.push((width + gap) / key.weight);
            if (previousRight !== undefined) expect(left - previousRight).toBeCloseTo(gap, 8);
            previousRight = right;
          }
        }
        for (const unit of units) expect(unit).toBeCloseTo(units[0], 8);
        unmount();
      }
      const equal = keyLayoutPreviewRows(
        "fourteen",
        { ...preferences, touch_fourteen_key_letter_widths: "equal" },
        scheme,
      );
      expect(equal.every((row) => row.keys.every((key) => !key.letterPitch))).toBe(true);
      expect(
        keyLayoutPreviewRows("twentySix", preferences, "quanpin").every((row) =>
          row.keys.every((key) => !key.letterPitch),
        ),
      ).toBe(true);
    }
  },
);
