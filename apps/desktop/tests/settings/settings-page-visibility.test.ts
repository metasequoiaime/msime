import { expect, test } from "vitest";
import {
  canReloadSettingsPage,
  canRestoreDefaultsOnPage,
  isSettingsFormPage,
  type SettingsPageId,
} from "@msime/ui";

const pages: SettingsPageId[] = [
  "home",
  "appearance",
  "dictionary",
  "input",
  "skin",
  "floating-toolbar",
  "expression",
  "shortcuts",
  "tools",
  "plugins",
  "developer",
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
  "try-keyboard",
];

test("keeps standalone pages out of the shared form", () => {
  expect(pages.filter(isSettingsFormPage)).toEqual(
    pages.filter(
      (page) =>
        ![
          "typing-statistics",
          "vocabulary",
          "account",
          "chat",
          "community",
          "try-keyboard",
        ].includes(page),
    ),
  );
});

test("keeps reload available on the shared form and more pages", () => {
  expect(pages.filter(canReloadSettingsPage)).toEqual(
    pages.filter(
      (page) =>
        ![
          "typing-statistics",
          "vocabulary",
          "account",
          "chat",
          "community",
          "try-keyboard",
        ].includes(page),
    ),
  );
});

test("offers restore defaults only on the pages that edit preferences", () => {
  expect(pages.filter(canRestoreDefaultsOnPage)).toEqual([
    "appearance",
    "dictionary",
    "input",
    "skin",
    "floating-toolbar",
    "expression",
    "shortcuts",
    "tools",
    "plugins",
    "screen-keyboard",
    "voice",
    "ai",
  ]);
  for (const page of ["home", "developer", "help", "about", "feedback"] as const)
    expect(canRestoreDefaultsOnPage(page)).toBe(false);
});

test("手写页不提供全局偏好重置", () => {
  expect(canRestoreDefaultsOnPage("handwriting")).toBe(false);
});
