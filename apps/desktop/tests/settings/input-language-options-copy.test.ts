import { expect, test } from "vitest";

test("expression page uses the grouped input language options component", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/expression-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain(
    'import { InputLanguageOptionsSection } from "../input-language-options-section";',
  );
  expect(page).toContain("<InputLanguageOptionsSection");
  expect(page).toContain("grouped");
  expect(page).not.toContain("<MixedInputSection");
  expect(page).not.toContain("<CandidateEnglishGlossSection");
  // 唯一直接的英文联想行是 HarmonyOS 手机「智能」分组里的「英文联想」；其他所有宿主都通过分组组件得到它。
  expect(page.match(/<EnglishSuggestionsSection/g)).toHaveLength(1);
  expect(page).toContain('title="英文联想"');
});
