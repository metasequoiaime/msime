import { expect, test } from "vitest";
import { canReloadSettingsPage, isSettingsFormPage, type SettingsPageId } from "@msime/ui";

const pages: SettingsPageId[] = [
  "home",
  "more",
  "appearance",
  "dictionary",
  "input",
  "skin",
  "floating-toolbar",
  "helpcode",
  "shortcuts",
  "tools",
  "help",
  "about",
  "screen-keyboard",
  "handwriting",
  "voice",
  "ai",
  "feedback",
  "account",
  "chat",
  "community",
  "typing-statistics",
  "vocabulary",
];

test("keeps standalone pages out of the shared form", () => {
  expect(pages.filter(isSettingsFormPage)).toEqual(
    pages.filter(
      (page) =>
        !["typing-statistics", "vocabulary", "account", "chat", "more", "community"].includes(page),
    ),
  );
});

test("keeps reload available on the shared form and more pages", () => {
  expect(pages.filter(canReloadSettingsPage)).toEqual(
    pages.filter(
      (page) => !["typing-statistics", "vocabulary", "account", "chat", "community"].includes(page),
    ),
  );
});
