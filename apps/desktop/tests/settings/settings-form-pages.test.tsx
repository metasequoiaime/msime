// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ReactNode } from "react";

vi.mock("../../../../packages/ui/src/settings/settings-form-frame", () => ({
  SettingsFormFrame: ({ children }: { children: ReactNode }) => (
    <div data-testid="settings-form-frame">{children}</div>
  ),
}));

import { SettingsFormPages } from "../../../../packages/ui/src/settings/settings-form-pages";

test("renders the shared settings form pages in their stable order", () => {
  render(
    <SettingsFormPages
      frame={{} as never}
      visual={<div data-page="visual" />}
      dictionary={<div data-page="dictionary" />}
      input={<div data-page="input" />}
      utility={<div data-page="utility" />}
      about={<div data-page="about" />}
      interaction={<div data-page="interaction" />}
      voiceAi={<div data-page="voice-ai" />}
      feedback={<div data-page="feedback" />}
      footer={<div data-page="footer" />}
    />,
  );

  expect(screen.getByTestId("settings-form-frame").innerHTML).toBe(
    '<div data-page="visual"></div><div data-page="dictionary"></div><div data-page="input"></div><div data-page="utility"></div><div data-page="about"></div><div data-page="interaction"></div><div data-page="voice-ai"></div><div data-page="feedback"></div><div data-page="footer"></div>',
  );
});
