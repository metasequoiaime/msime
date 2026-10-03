// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SkinCardHeader } from "@msime/ui";

afterEach(cleanup);

test("renders the shared skin card title, theme, selection, description, and actions", () => {
  render(
    <SkinCardHeader
      title="自定义"
      theme="dark"
      selected
      description="主题说明"
      details="额外信息"
      actions={<button type="button">选择</button>}
    />,
  );

  expect(screen.getByText("自定义（深色）")).toBeTruthy();
  expect(screen.getByText("使用中")).toBeTruthy();
  expect(screen.getByText("主题说明")).toBeTruthy();
  expect(screen.getByText("额外信息")).toBeTruthy();
  expect(screen.getByRole("button", { name: "选择" })).toBeTruthy();
});

test("keeps the unselected light title without a selection marker", () => {
  render(
    <SkinCardHeader
      title="浅色"
      theme="light"
      selected={false}
      description="说明"
      actions={null}
    />,
  );

  expect(screen.getByText("浅色（浅色）")).toBeTruthy();
  expect(screen.queryByText("使用中")).toBeNull();
});
