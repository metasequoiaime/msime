// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsExternalMeta } from "../../../../packages/ui/src/settings/settings-external-meta";

afterEach(cleanup);

test("renders metadata as the requested semantic element and keeps its status role", () => {
  render(
    <>
      <SettingsExternalMeta as="span">package details</SettingsExternalMeta>
      <SettingsExternalMeta role="alert">directory failed</SettingsExternalMeta>
    </>,
  );

  expect(screen.getByText("package details").tagName).toBe("SPAN");
  const alert = screen.getByRole("alert");
  expect(alert.tagName).toBe("P");
  expect(alert.textContent).toBe("directory failed");
});
