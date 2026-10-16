// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { PluginNavRow } from "@msime/ui";

afterEach(cleanup);

test("renders title, description, marker and row identity", () => {
  render(
    <PluginNavRow
      title="示例插件"
      description="版本 1.0"
      marker="已启用"
      rowKey="pack/sound/example"
      onOpen={vi.fn()}
    />,
  );

  const row = screen.getByRole("button", { name: "示例插件" });
  expect(row.getAttribute("data-plugin-row")).toBe("pack/sound/example");
  expect(screen.getByText("版本 1.0")).toBeTruthy();
  expect(screen.getByText("已启用")).toBeTruthy();
});

test("describes only content that exists", () => {
  const { rerender } = render(
    <PluginNavRow
      title="完整插件"
      description="详细说明"
      marker="状态"
      rowKey="complete"
      onOpen={vi.fn()}
    />,
  );
  const complete = screen.getByRole("button", { name: "完整插件" });
  const describedBy = complete.getAttribute("aria-describedby")?.split(" ") ?? [];
  expect(describedBy).toHaveLength(2);
  expect(describedBy.every((id) => document.getElementById(id))).toBe(true);

  rerender(<PluginNavRow title="简洁插件" rowKey="plain" onOpen={vi.fn()} />);
  expect(screen.getByRole("button", { name: "简洁插件" }).getAttribute("aria-describedby")).toBe(
    null,
  );
});

test("opens the target without submitting an outer form", () => {
  const onOpen = vi.fn();
  const onSubmit = vi.fn();
  render(
    <form onSubmit={onSubmit}>
      <PluginNavRow title="打开插件" rowKey="target" onOpen={onOpen} />
    </form>,
  );

  fireEvent.click(screen.getByRole("button", { name: "打开插件" }));
  expect(onOpen).toHaveBeenCalledOnce();
  expect(onSubmit).not.toHaveBeenCalled();
});
