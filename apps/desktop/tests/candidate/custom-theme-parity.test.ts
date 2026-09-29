import { expect, test } from "vitest";
import {
  customCandidatePalette,
  themeEntry,
  type CandidateThemePalette,
  type CustomCandidateColors,
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
  const mode = themeEntry(entry.base).appearance ?? (entry.dark ? "dark" : "light");
  const palette = entry.package ? drawnPackagePalette(entry.package, mode) : null;
  expect(customCandidatePalette(entry.base, entry.colors, palette)).toEqual(entry.expected);
});

test("the parity cases cover both preview regressions", () => {
  const names = (cases as Case[]).map((entry) => entry.name);
  expect(names).toContain("a text picker beats the package text");
  expect(names).toContain("a package accent derives the selected row over a built-in base");
});
