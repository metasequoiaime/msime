import { expect, test } from "vitest";
import { availableSettingsPages, type AvailablePageCapabilities } from "@msime/ui";

const capabilities: AvailablePageCapabilities = {
  home: false,
  typingStatistics: false,
  vocabularyReview: false,
  account: false,
  chat: false,
  community: false,
  floatingToolbar: false,
  mobile: false,
};

test("keeps common settings pages while hiding unavailable host entries", () => {
  const ids = availableSettingsPages(capabilities).map((page) => page.id);

  expect(ids).toContain("appearance");
  expect(ids).toContain("input");
  expect(ids).not.toContain("home");
  expect(ids).not.toContain("vocabulary");
  expect(ids).not.toContain("community");
  expect(ids).not.toContain("floating-toolbar");
  expect(ids).not.toContain("more");
});

test("exposes only the host-backed and mobile entries that are available", () => {
  const ids = availableSettingsPages({
    ...capabilities,
    home: true,
    account: true,
    vocabularyReview: true,
    mobile: true,
  }).map((page) => page.id);

  expect(ids).toContain("home");
  expect(ids).toContain("account");
  expect(ids).toContain("vocabulary");
  expect(ids).toContain("more");
  expect(ids).not.toContain("chat");
  expect(ids).not.toContain("typing-statistics");
});
