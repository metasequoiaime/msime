import { expect, test } from "vitest";

test("screen keyboard page uses the shared mobile feedback composition", () => {
  const page = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/pages/screen-keyboard-page.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];
  expect(page).toContain(
    'import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";',
  );
  expect(page).toContain("<MobileKeyboardFeedbackSettings");
  expect(page).not.toContain("<MobileKeyboardFeedbackSection");
  expect(page).not.toContain("client.mobileKeyboardFeedback && mobileKeyboardFeedback");
});
