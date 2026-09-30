import { expect, test } from "vitest";

test("screen keyboard page and input panel share mobile feedback composition", () => {
  const page = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/settings/pages/screen-keyboard-page.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/input-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain(
    'import { MobileKeyboardFeedbackSettings } from "../mobile-keyboard-feedback-settings";',
  );
  expect(panel).toContain(
    'import { MobileKeyboardFeedbackSettings } from "./mobile-keyboard-feedback-settings";',
  );
  expect(page).toContain("<MobileKeyboardFeedbackSettings");
  expect(panel).toContain("<MobileKeyboardFeedbackSettings");
  for (const source of [page, panel]) {
    expect(source).not.toContain("<MobileKeyboardFeedbackSection");
    expect(source).not.toContain("client.mobileKeyboardFeedback && mobileKeyboardFeedback");
  }
});
