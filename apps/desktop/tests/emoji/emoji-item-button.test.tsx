// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { EmojiItemButton } from "@msime/ui";

afterEach(cleanup);

test("条目直接呈现原文，中文提示优先，点击转发同一个完整条目", () => {
  const item = Object.freeze({ text: "  (合成🦉)\n", keywords: "synthetic 合成提示 owl" });
  const onSelect = vi.fn();
  const view = render(<EmojiItemButton item={item} onSelect={onSelect} />);
  const button = screen.getByRole("button");
  expect(view.container.childElementCount).toBe(1);
  expect(view.container.firstElementChild).toBe(button);
  expect(button.getAttribute("type")).toBe("button");
  expect(button.textContent).toBe(item.text);
  expect(button.title).toBe("合成提示");
  expect(button.hasAttribute("data-emoji-navigation-item")).toBe(false);
  fireEvent.click(button);
  expect(onSelect).toHaveBeenCalledExactlyOnceWith(item);
  expect(onSelect.mock.calls[0][0]).toBe(item);
});

test.each([
  ["synthetic first", "synthetic"],
  ["", "🦉"],
  ["  \t ", "🦉"],
])("关键词 %j 沿用既有提示回退", (keywords, title) => {
  render(<EmojiItemButton item={{ text: "🦉", keywords }} onSelect={vi.fn()} />);
  expect(screen.getByRole("button").title).toBe(title);
});

test.each([
  [false, false, "min-h-[66px]"],
  [false, true, "h-10"],
  [true, false, "whitespace-pre-wrap"],
  [true, true, "whitespace-pre-wrap"],
])("流式 %s、紧凑 %s 保留对应几何与附加样式", (flow, compact, sizeClass) => {
  render(
    <EmojiItemButton
      item={{ text: "合成", keywords: "" }}
      onSelect={vi.fn()}
      flow={flow}
      compact={compact}
      navigable
      className="shrink-0 min-w-10"
    />,
  );
  const button = screen.getByRole("button");
  expect(button.classList.contains(sizeClass)).toBe(true);
  expect(button.classList.contains("shrink-0")).toBe(true);
  expect(button.classList.contains("min-w-10")).toBe(true);
  expect(button.getAttribute("data-emoji-navigation-item")).toBe("true");
  expect((button as HTMLButtonElement).disabled).toBe(false);
});

test("禁用阻止选择，属性更新后使用新条目与回调并移除导航标记", () => {
  const oldSelect = vi.fn();
  const onSelect = vi.fn();
  const item = { text: "合成新条目", keywords: "new 新提示" };
  const view = render(
    <EmojiItemButton
      item={{ text: "合成旧条目", keywords: "旧提示" }}
      onSelect={oldSelect}
      disabled
      navigable
    />,
  );
  expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button"));
  expect(oldSelect).not.toHaveBeenCalled();
  view.rerender(<EmojiItemButton item={item} onSelect={onSelect} />);
  const button = screen.getByRole("button");
  expect((button as HTMLButtonElement).disabled).toBe(false);
  expect(button.textContent).toBe(item.text);
  expect(button.title).toBe("新提示");
  expect(button.hasAttribute("data-emoji-navigation-item")).toBe(false);
  fireEvent.click(button);
  expect(onSelect).toHaveBeenCalledExactlyOnceWith(item);
  expect(oldSelect).not.toHaveBeenCalled();
});
