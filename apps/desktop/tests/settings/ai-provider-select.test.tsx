// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { AiProviderSelect } from "../../../../packages/ui/src/settings/ai-provider-select";
import { AI_PROVIDER_OPTIONS } from "../../../../packages/ui/src/settings/ai-provider-options";

afterEach(cleanup);

function setup() {
  const onChange = vi.fn();
  const props = { options: AI_PROVIDER_OPTIONS, value: "deepseek", onChange };
  const result = render(<AiProviderSelect {...props} />);
  const trigger = screen.getByRole("combobox", { name: "AI 服务提供商" });
  return { ...result, props, trigger, onChange };
}

test("服务商的收起控件与十二个展开选项都带图标，选择仍回传原目录标识", () => {
  const { trigger, onChange } = setup();
  expect(trigger.querySelector('[data-provider-icon="deepseek"]')).toBeTruthy();
  fireEvent.click(trigger);
  const list = screen.getByRole("listbox", { name: "AI 服务提供商" });
  const options = within(list).getAllByRole("option");
  expect(options).toHaveLength(AI_PROVIDER_OPTIONS.length);
  for (const item of options) expect(item.querySelector("[data-provider-icon]")).toBeTruthy();
  expect(within(list).getByRole("option", { selected: true }).textContent).toContain("DeepSeek");
  const custom = within(list).getByRole("option", { name: "自定义" });
  fireEvent.pointerDown(custom);
  expect(screen.getByRole("listbox")).toBe(list);
  fireEvent.click(custom);
  expect(onChange).toHaveBeenCalledExactlyOnceWith("custom");
  expect(screen.queryByRole("listbox")).toBeNull();
  expect(document.activeElement).toBe(trigger);
});

test("方向键只移动活动项，确认后提交；取消保持原值", () => {
  const { trigger, onChange } = setup();
  fireEvent.keyDown(trigger, { key: "ArrowDown" });
  fireEvent.keyDown(trigger, { key: "ArrowDown" });
  expect(onChange).not.toHaveBeenCalled();
  const activeId = trigger.getAttribute("aria-activedescendant")!;
  expect(document.getElementById(activeId)?.textContent).toContain("通义千问");
  fireEvent.keyDown(trigger, { key: "Escape" });
  expect(onChange).not.toHaveBeenCalled();
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
  fireEvent.keyDown(trigger, { key: "Enter" });
  fireEvent.keyDown(trigger, { key: "Home" });
  fireEvent.keyDown(trigger, { key: "ArrowUp" });
  fireEvent.keyDown(trigger, { key: " " });
  expect(onChange).toHaveBeenCalledExactlyOnceWith("custom");
});

test("文字查找能定位品牌，Tab 确认活动项并允许焦点继续移动", () => {
  const { trigger, onChange } = setup();
  fireEvent.keyDown(trigger, { key: "g" });
  fireEvent.keyDown(trigger, { key: "r" });
  expect(
    document.getElementById(trigger.getAttribute("aria-activedescendant")!)?.textContent,
  ).toContain("Groq");
  fireEvent.keyDown(trigger, { key: "Tab" });
  expect(onChange).toHaveBeenCalledExactlyOnceWith("groq");
  expect(screen.queryByRole("listbox")).toBeNull();
});

test("输入法合成按键不打开菜单，点击外部与失焦均取消", () => {
  const { trigger, onChange } = setup();
  fireEvent.keyDown(trigger, { key: "Enter", isComposing: true });
  expect(screen.queryByRole("listbox")).toBeNull();
  fireEvent.click(trigger);
  fireEvent.keyDown(trigger, { key: "End" });
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("listbox")).toBeNull();
  fireEvent.click(trigger);
  fireEvent.blur(trigger, { relatedTarget: document.body });
  expect(screen.queryByRole("listbox")).toBeNull();
  expect(onChange).not.toHaveBeenCalled();
});

test("禁用和页面隐藏会关闭菜单，重新显示时不复活旧菜单", () => {
  const { trigger, rerender, props } = setup();
  fireEvent.click(trigger);
  rerender(<AiProviderSelect {...props} disabled />);
  expect((trigger as HTMLButtonElement).disabled).toBe(true);
  expect(screen.queryByRole("listbox")).toBeNull();
  rerender(<AiProviderSelect {...props} />);
  fireEvent.click(trigger);
  rerender(<AiProviderSelect {...props} hidden />);
  rerender(<AiProviderSelect {...props} />);
  expect(screen.queryByRole("listbox")).toBeNull();
});

test("未知服务商保留原标识并显示连接图标；选回当前服务不重复提交", () => {
  const { trigger, rerender, props, onChange } = setup();
  fireEvent.click(trigger);
  fireEvent.keyDown(trigger, { key: "Enter" });
  expect(onChange).not.toHaveBeenCalled();
  rerender(<AiProviderSelect {...props} value="synthetic-provider" />);
  expect(trigger.textContent).toContain("synthetic-provider");
  expect(trigger.querySelector("svg")).toBeTruthy();
  fireEvent.click(trigger);
  fireEvent.keyDown(trigger, { key: "End" });
  fireEvent.keyDown(trigger, { key: "Enter" });
  expect(onChange).toHaveBeenCalledExactlyOnceWith("custom");
});

test("活动选项仅滚动菜单内部，外层滚动仍关闭菜单", () => {
  const { trigger } = setup();
  fireEvent.click(trigger);
  const list = screen.getByRole("listbox");
  const last = within(list).getByRole("option", { name: "自定义" });
  Object.defineProperty(list, "clientHeight", { value: 200 });
  vi.spyOn(list, "getBoundingClientRect").mockReturnValue({ top: 20 } as DOMRect);
  vi.spyOn(last, "getBoundingClientRect").mockReturnValue({ top: 260, bottom: 292 } as DOMRect);
  fireEvent.keyDown(trigger, { key: "End" });
  expect(list.scrollTop).toBe(72);
  fireEvent.scroll(list);
  expect(screen.getByRole("listbox")).toBe(list);
  fireEvent.scroll(document);
  expect(screen.queryByRole("listbox")).toBeNull();
});
