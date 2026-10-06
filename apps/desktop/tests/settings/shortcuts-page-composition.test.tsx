// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("../../../../packages/ui/src/settings/shortcuts-settings-section", () => ({
  ShortcutsSettingsSection: ({ hidden }: { hidden: boolean }) =>
    hidden ? null : <section aria-label="共享快捷键设置" />,
}));

import { SettingsPage, type Snapshot } from "@msime/ui";

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("the shortcuts settings page composes the shared shortcuts section", async () => {
  render(
    <SettingsPage
      initialPage="shortcuts"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
      }}
    />,
  );

  await settingsFormReady();
  expect(screen.getByRole("region", { name: "共享快捷键设置" })).toBeTruthy();
});
