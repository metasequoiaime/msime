import { expect, test } from "vitest";
import {
  customCandidatePalette,
  customDrawnBase,
  skinDrawsIn,
  themeEntry,
  type BaseGlobalTheme,
  type CandidateThemePalette,
  type CustomCandidateColors,
  type CustomTheme,
  type GlobalTheme,
  type PackageCandidatePalette,
} from "../../../../packages/ui/src/theme/global-theme";
import { drawnPackagePalette } from "../../../../packages/ui/src/skin/external-skins";
import cases from "./custom-theme-parity.json";

// Written by the Rust test `web_custom_theme_mirror_cases_match_resolve`, so `expected` is what `resolve()` draws.
type Case = {
  name: string;
  base: GlobalTheme;
  dark: boolean;
  package: {
    themes: string[];
    candidate: { light: PackageCandidatePalette; dark: PackageCandidatePalette };
  } | null;
  colors: CustomCandidateColors;
  expected: CandidateThemePalette | null;
};

test.each(cases as Case[])("the page mirror draws what resolve() draws: $name", (entry) => {
  // 与 Rust 用例同一个形状：`custom.base` 与包的 base 都是 `entry.base`，有包时 `candidate_skin` 就是它。
  const custom: CustomTheme = {
    base: entry.base as BaseGlobalTheme,
    candidate_skin: entry.package ? "sample" : null,
  };
  const drawn = entry.package !== null && skinDrawsIn(entry.base, entry.dark);
  const base = customDrawnBase(custom, drawn ? (entry.base as BaseGlobalTheme) : null, entry.dark);
  const mode = themeEntry(base).appearance ?? (entry.dark ? "dark" : "light");
  const palette = drawn && entry.package ? drawnPackagePalette(entry.package, mode) : null;
  expect(customCandidatePalette(base, entry.colors, palette)).toEqual(entry.expected);
});

test("the parity cases cover both preview regressions", () => {
  const names = (cases as Case[]).map((entry) => entry.name);
  expect(names).toContain("a text picker beats the package text");
  expect(names).toContain("a package accent derives the selected row over a built-in base");
});
