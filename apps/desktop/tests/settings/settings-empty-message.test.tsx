// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsEmptyMessage } from "@msime/ui";

afterEach(cleanup);

test("renders the shared settings empty message style", () => {
  render(<SettingsEmptyMessage>没有内容</SettingsEmptyMessage>);

  const message = screen.getByText("没有内容");
  expect(message.tagName).toBe("P");
  expect(message.className).toBe(
    "m-0 px-4 py-6 text-center [font-size:var(--p-sub-fs)] [color:var(--p-sub)]",
  );
});

test("forwards message attributes and appends a local class", () => {
  render(
    <SettingsEmptyMessage className="compact" role="status" aria-label="状态" data-testid="message">
      读取中
    </SettingsEmptyMessage>,
  );

  const message = screen.getByTestId("message");
  expect(message.className).toContain("compact");
  expect(message.getAttribute("role")).toBe("status");
  expect(message.getAttribute("aria-label")).toBe("状态");
});

test("supports the compact text-only empty message style", () => {
  render(<SettingsEmptyMessage compact>点击查询后查看词条</SettingsEmptyMessage>);

  expect(screen.getByText("点击查询后查看词条").className).toBe("text-muted");
});
