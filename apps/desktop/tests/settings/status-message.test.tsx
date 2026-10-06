// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { StatusMessage } from "@msime/ui";

afterEach(cleanup);

test("renders a plain live status paragraph and forwards attributes", () => {
  render(
    <StatusMessage role="status" aria-label="同步状态">
      正在同步…
    </StatusMessage>,
  );

  const status = screen.getByRole("status", { name: "同步状态" });
  expect(status.tagName).toBe("P");
  expect(status.textContent).toBe("正在同步…");
});
