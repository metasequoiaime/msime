// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsManagerNote } from "@msime/ui";

afterEach(cleanup);

test("renders a settings manager note as a paragraph with the shared manager note style", () => {
  render(<SettingsManagerNote>连接说明</SettingsManagerNote>);

  const note = screen.getByText("连接说明");
  expect(note.tagName).toBe("P");
  expect(note.className).toBe(
    "m-0 leading-relaxed [font-size:var(--p-sub-fs)] [color:var(--p-sub)]",
  );
});
