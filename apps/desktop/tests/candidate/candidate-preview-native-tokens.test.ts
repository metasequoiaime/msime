import { expect, test } from "vitest";
import { candidatePaletteStyle, themeEntry, type CandidateThemePalette } from "@msime/ui";
import { utilityCss } from "../support/utility-css";

const card = utilityCss("skin-card-preview").replace(/\s+/g, "");

const empty: CandidateThemePalette = {
  surface: null,
  border: null,
  text: null,
  number: null,
  secondary: null,
  accent: null,
  selected: null,
  selected_text: null,
  selected_number: null,
  hover: null,
  show_selected_bar: null,
};

function style(palette: Partial<CandidateThemePalette>): Record<string, string> {
  return candidatePaletteStyle({ ...empty, ...palette }) as Record<string, string>;
}

test("a theme's selected foregrounds reach the preview", () => {
  const light = themeEntry("light").candidate!;
  const drawn = candidatePaletteStyle(light) as Record<string, string>;
  expect(drawn["--cand-selected-text"]).toBe(light.selected_text);
  expect(drawn["--cand-selected-num"]).toBe(light.selected_number);
  // 内置主题的竖条开关是 null：交给平台默认值，不写进内联样式。
  expect(drawn).not.toHaveProperty("--cand-selected-bar");
});

test("the selected-bar switch is written only when the palette sets it", () => {
  expect(style({ show_selected_bar: true })["--cand-selected-bar"]).toBe("block");
  expect(style({ show_selected_bar: false })["--cand-selected-bar"]).toBe("none");
  expect(style({ accent: "#2C7A4B" })).not.toHaveProperty("--cand-selected-bar");
  // `system` 没有调色板，什么都不写。
  expect(candidatePaletteStyle(null)).toEqual({});
});

test("the macOS selected foregrounds follow the fill that is drawn when the palette leaves them unset", () => {
  // 没给 selected 时选中行跟随强调色；浅色强调色上取深字，序号同色 82% 不透明。
  expect(style({ accent: "#FFEE88" })).toMatchObject({
    "--cand-mac-selected-text": "#1A1A1A",
    "--cand-mac-selected-num": "#1A1A1AD1",
  });
  expect(style({ accent: "#112233" })).toMatchObject({
    "--cand-mac-selected-text": "#FFFFFF",
    "--cand-mac-selected-num": "#FFFFFFD1",
  });
  // 半透明的选中底（不透明度低于 0.85）：文字和序号都回到普通行的颜色。
  expect(style({ selected: "#11223340", accent: "#FFEE88" })).toMatchObject({
    "--cand-mac-selected-text": "var(--cand-text)",
    "--cand-mac-selected-num": "var(--cand-num)",
  });
  // 调色板自己给了选中前景色时不再推导。
  const given = style({
    selected: "#112233",
    selected_text: "#00FF00",
    selected_number: "#00FF0080",
  });
  expect(given).not.toHaveProperty("--cand-mac-selected-text");
  expect(given).not.toHaveProperty("--cand-mac-selected-num");
  // 既没有 selected 也没有 accent：原生画的是平台强调色，交给样式表里的白字。
  expect(style({ surface: "#FFFFFF" })).not.toHaveProperty("--cand-mac-selected-text");
});

test("the selected row reads the selected foregrounds and the bar switch, falling back to the old look", () => {
  expect(card).toContain(".first.text{color:var(--cand-selected-text,var(--cand-text));}");
  expect(card).toContain(".first.num{color:var(--cand-selected-num,var(--cand-num));}");
  expect(card).toContain(".first.cand-no{color:var(--cand-selected-num,var(--cand-num));}");
  expect(card.match(/display:var\(--cand-selected-bar,block\)/g)).toHaveLength(2);
});
