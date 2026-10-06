// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsServiceRow } from "@msime/ui";

afterEach(cleanup);

test("renders a service row with the shared service layout", () => {
  render(<SettingsServiceRow>服务内容</SettingsServiceRow>);

  const row = screen.getByText("服务内容");
  expect(row.tagName).toBe("DIV");
  expect(row.className).toBe(
    "flex items-center justify-between gap-4 pt-3 [&>div]:flex [&>div]:flex-wrap [&>div]:items-center [&>div]:gap-2 [&_.secondary]:mt-0 [&_[role=status]]:text-xs [&_[role=status]]:text-muted [&_[role=alert]]:text-xs [&_[role=alert]]:text-muted",
  );
});

test("preserves row attributes and appends a local class", () => {
  render(
    <SettingsServiceRow className="danger" role="group" aria-label="服务">
      服务区域
    </SettingsServiceRow>,
  );

  const row = screen.getByRole("group", { name: "服务" });
  expect(row.className).toContain("danger");
});
