// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { KeyboardEvent } from "react";
import { EmojiClipboardEntries } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const raw = "合成\r\t记录\n下一行\n\n";
const items: readonly string[] = Object.freeze([raw, "合成第二条"]);

function actions() {
  return { onCopy: vi.fn(), onItemKeyDown: vi.fn(), onPaste: vi.fn(), onRemove: vi.fn() };
}

test("列表保留顺序和导航标记，预览折叠控制字符，动作传原文", () => {
  const callbacks = actions();
  const view = render(<EmojiClipboardEntries items={items} busy={false} canCopy {...callbacks} />);
  const rows = view.container.querySelectorAll<HTMLElement>("[data-clipboard-row]");
  expect(rows).toHaveLength(2);
  expect(rows[0].parentElement).toBe(view.container.firstElementChild);
  const entry = rows[0].querySelector<HTMLButtonElement>("[data-clipboard-item]")!;
  expect(entry.closest("[data-clipboard-row]")).toBe(rows[0]);
  expect(entry.hasAttribute("data-emoji-navigation-item")).toBe(true);
  expect(entry.textContent).toBe("合成  记录 下一行  ");
  // 长文本预览改由气泡呈现，不再交给桌面主题的原生 tooltip。
  expect(entry.hasAttribute("title")).toBe(false);
  expect(rows[1].querySelector("[data-clipboard-item]")?.textContent).toBe(items[1]);
  expect(
    within(rows[0])
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["合成  记录 下一行  ", "×", "粘贴"]);
  const remove = within(rows[0]).getByRole("button", { name: "删除此条历史记录" });
  expect(remove.title).toBe("删除此条历史记录，不会清空系统剪贴板");
  fireEvent.click(entry);
  fireEvent.click(within(rows[0]).getByRole("button", { name: "粘贴此条记录到原应用" }));
  fireEvent.click(remove);
  expect(callbacks.onCopy.mock.calls).toEqual([[raw]]);
  expect(callbacks.onPaste.mock.calls).toEqual([[raw]]);
  expect(callbacks.onRemove.mock.calls).toEqual([[raw]]);
});

test("鼠标悬停片刻显示长文本预览气泡，按 Unicode 字符截断，复制保留完整原文", () => {
  vi.useFakeTimers();
  const text = "😀".repeat(201) + "\n\n";
  const callbacks = actions();
  const view = render(<EmojiClipboardEntries items={[text]} busy={false} canCopy {...callbacks} />);
  const entry = screen.getAllByRole("button")[0];
  const bubble = () => view.container.querySelector("[data-clipboard-preview]");
  fireEvent.pointerEnter(entry, { pointerType: "mouse" });
  expect(bubble()).toBeNull();
  act(() => vi.advanceTimersByTime(400));
  expect(bubble()?.textContent).toBe("😀".repeat(200) + "…");
  // 气泡只是视觉预览，整行文字已经是按钮的名称。
  expect(bubble()?.getAttribute("aria-hidden")).toBe("true");
  fireEvent.pointerLeave(entry, { pointerType: "mouse" });
  expect(bubble()).toBeNull();
  fireEvent.click(entry);
  expect(callbacks.onCopy.mock.calls).toEqual([[text]]);
});

test("触屏和点击聚焦不弹预览，键盘移来的焦点才弹", () => {
  vi.useFakeTimers();
  const callbacks = actions();
  const view = render(<EmojiClipboardEntries items={[raw]} busy={false} canCopy {...callbacks} />);
  const entry = screen.getAllByRole("button")[0];
  const bubble = () => view.container.querySelector("[data-clipboard-preview]");
  fireEvent.pointerEnter(entry, { pointerType: "touch" });
  fireEvent.pointerDown(entry, { pointerType: "touch" });
  fireEvent.focus(entry);
  act(() => vi.advanceTimersByTime(400));
  expect(bubble()).toBeNull();
  fireEvent.blur(entry);
  fireEvent.focus(entry);
  act(() => vi.advanceTimersByTime(400));
  expect(bubble()?.textContent).toBe("合成  记录\n下一行");
});

test("移动到另一行重新等待预览，内容更新和卸载取消待显示气泡", () => {
  vi.useFakeTimers();
  const props = { busy: false, canCopy: true, ...actions() };
  const view = render(<EmojiClipboardEntries items={items} {...props} />);
  const entries = view.container.querySelectorAll<HTMLButtonElement>("[data-clipboard-item]");
  const bubble = () => view.container.querySelector("[data-clipboard-preview]");
  fireEvent.pointerEnter(entries[0], { pointerType: "mouse" });
  act(() => vi.advanceTimersByTime(349));
  fireEvent.pointerEnter(entries[1], { pointerType: "mouse" });
  act(() => vi.advanceTimersByTime(349));
  expect(bubble()).toBeNull();
  act(() => vi.advanceTimersByTime(1));
  expect(bubble()?.textContent).toBe(items[1]);

  fireEvent.pointerEnter(entries[0], { pointerType: "mouse" });
  view.rerender(<EmojiClipboardEntries items={[items[1]]} {...props} />);
  act(() => vi.advanceTimersByTime(350));
  expect(bubble()).toBeNull();
  fireEvent.pointerEnter(screen.getAllByRole("button")[0], { pointerType: "mouse" });
  view.unmount();
  expect(vi.getTimerCount()).toBe(0);
});

test.each([
  // 窗口顶部附近的第一行：一行的短预览在上方放得下，不盖住下面的行。
  { rowTop: 120, height: 40, above: true, top: "112px" },
  // 12 行的长预览在上方放不下，翻到行的下方。
  { rowTop: 120, height: 230, above: false, top: "160px" },
  { rowTop: 300, height: 230, above: true, top: "292px" },
])("预览气泡量出高度后定上下：行顶 $rowTop、气泡高 $height", ({ rowTop, height, above, top }) => {
  vi.useFakeTimers();
  const rect = vi
    .spyOn(HTMLElement.prototype, "getBoundingClientRect")
    .mockImplementation(function (this: HTMLElement) {
      const box = this.hasAttribute("data-clipboard-preview")
        ? { top: 0, bottom: height, height }
        : { top: rowTop, bottom: rowTop + 32, height: 32 };
      return { ...box, left: 10, right: 330, width: 320, x: 10, y: box.top } as DOMRect;
    });
  const view = render(<EmojiClipboardEntries items={[raw]} busy={false} canCopy {...actions()} />);
  fireEvent.pointerEnter(screen.getAllByRole("button")[0], { pointerType: "mouse" });
  act(() => vi.advanceTimersByTime(400));
  const bubble = view.container.querySelector<HTMLElement>("[data-clipboard-preview]")!;
  expect(bubble.style.top).toBe(top);
  expect(bubble.style.transform).toBe(above ? "translateY(-100%)" : "");
  rect.mockRestore();
});

test("右键菜单把这一条发到云剪贴板，Esc 关闭菜单而不交给面板", () => {
  const callbacks = actions();
  const onSendToCloud = vi.fn();
  const onKeyDown = vi.fn();
  render(
    <div onKeyDown={onKeyDown}>
      <EmojiClipboardEntries
        items={items}
        busy={false}
        canCopy
        {...callbacks}
        onSendToCloud={onSendToCloud}
      />
    </div>,
  );
  const second = screen.getByRole("button", { name: "合成第二条" });
  expect(second.getAttribute("aria-haspopup")).toBe("menu");
  fireEvent.contextMenu(second, { clientX: 40, clientY: 50 });
  fireEvent.click(screen.getByRole("menuitem", { name: "发到云剪贴板" }));
  expect(onSendToCloud.mock.calls).toEqual([["合成第二条"]]);
  expect(screen.queryByRole("menu")).toBeNull();
  // 鼠标右键打开时焦点留在行上。
  second.focus();
  fireEvent.contextMenu(second, { clientX: 40, clientY: 50 });
  expect(document.activeElement).toBe(second);
  fireEvent.keyDown(second, { key: "Escape" });
  expect(screen.queryByRole("menu")).toBeNull();
  expect(onKeyDown).not.toHaveBeenCalled();
  // 菜单键或 Shift+F10 打开时焦点进菜单，Esc 关闭并回到那一行。浏览器给键盘触发的 contextmenu 带上行的坐标，不是 0,0。
  for (const key of [{ key: "ContextMenu" }, { key: "F10", shiftKey: true }]) {
    fireEvent.keyDown(second, key);
    fireEvent.contextMenu(second, { clientX: 12, clientY: 64 });
    const item = screen.getByRole("menuitem", { name: "发到云剪贴板" });
    expect(document.activeElement).toBe(item);
    fireEvent.keyDown(item, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(second);
  }
  expect(onKeyDown.mock.calls.map(([event]) => event.key)).toEqual(["ContextMenu", "F10"]);
  expect(onSendToCloud).toHaveBeenCalledTimes(1);
  expect(callbacks.onCopy).not.toHaveBeenCalled();
});

test("宿主没有云剪贴板时不拦截右键菜单", () => {
  const callbacks = actions();
  render(<EmojiClipboardEntries items={[raw]} busy={false} canCopy {...callbacks} />);
  const entry = screen.getAllByRole("button")[0];
  expect(entry.hasAttribute("aria-haspopup")).toBe(false);
  expect(fireEvent.contextMenu(entry)).toBe(true);
  expect(screen.queryByRole("menu")).toBeNull();
});

test.each([
  [false, false, null],
  [true, false, "Control+c"],
  [false, true, "Delete"],
  [true, true, "Control+c Delete"],
] as const)("复制能力 %s、删除能力 %s 保持对应快捷键声明", (canCopy, removable, shortcuts) => {
  const callbacks = actions();
  render(
    <EmojiClipboardEntries
      items={[raw]}
      busy={false}
      canCopy={canCopy}
      onCopy={callbacks.onCopy}
      onItemKeyDown={callbacks.onItemKeyDown}
      onRemove={removable ? callbacks.onRemove : undefined}
    />,
  );
  const entry = screen.getAllByRole("button")[0];
  expect(entry.getAttribute("aria-keyshortcuts")).toBe(shortcuts);
  expect(screen.queryByRole("button", { name: "删除此条历史记录" }) !== null).toBe(removable);
  expect(screen.queryByRole("button", { name: "粘贴此条记录到原应用" })).toBeNull();
  fireEvent.click(entry);
  expect(callbacks.onCopy.mock.calls).toEqual([[raw]]);
});

test("键盘事件原样交给调用方，不在视图里执行复制或删除", () => {
  const callbacks = actions();
  const onItemKeyDown = vi.fn((event: KeyboardEvent<HTMLButtonElement>, text: string) => {
    expect(event.currentTarget).toBe(screen.getAllByRole("button")[0]);
    expect(text).toBe(raw);
    event.preventDefault();
  });
  render(
    <EmojiClipboardEntries
      items={[raw]}
      busy={false}
      canCopy
      {...callbacks}
      onItemKeyDown={onItemKeyDown}
    />,
  );
  const entry = screen.getAllByRole("button")[0];
  expect(fireEvent.keyDown(entry, { key: "C", ctrlKey: true })).toBe(false);
  expect(onItemKeyDown).toHaveBeenCalledOnce();
  expect(onItemKeyDown.mock.calls[0][0].key).toBe("C");
  expect(onItemKeyDown.mock.calls[0][0].ctrlKey).toBe(true);
  expect(callbacks.onCopy).not.toHaveBeenCalled();
  expect(callbacks.onRemove).not.toHaveBeenCalled();
});

test("忙时禁用全部按钮，恢复后可操作，并响应可选能力变化", () => {
  const callbacks = actions();
  const view = render(<EmojiClipboardEntries items={[raw]} busy canCopy {...callbacks} />);
  for (const button of screen.getAllByRole("button")) {
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
  }
  expect(callbacks.onCopy).not.toHaveBeenCalled();
  expect(callbacks.onPaste).not.toHaveBeenCalled();
  expect(callbacks.onRemove).not.toHaveBeenCalled();
  view.rerender(
    <EmojiClipboardEntries
      items={[raw]}
      busy={false}
      canCopy
      onCopy={callbacks.onCopy}
      onItemKeyDown={callbacks.onItemKeyDown}
    />,
  );
  expect(screen.getAllByRole("button")).toHaveLength(1);
  expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button"));
  expect(callbacks.onCopy.mock.calls).toEqual([[raw]]);
});

test("空列表仅呈现原列表容器，状态文案由面板提供", () => {
  const callbacks = actions();
  const view = render(<EmojiClipboardEntries items={[]} busy={false} canCopy {...callbacks} />);
  expect(view.container.childElementCount).toBe(1);
  expect(view.container.firstElementChild?.childElementCount).toBe(0);
});

test("面板轮询给出内容相同的新数组时，正在显示的预览和菜单不收起；内容变了才收起", () => {
  vi.useFakeTimers();
  const callbacks = actions();
  const onSendToCloud = vi.fn();
  const props = { busy: false, canCopy: true, ...callbacks, onSendToCloud };
  const view = render(<EmojiClipboardEntries items={[...items]} {...props} />);
  const entry = view.container.querySelectorAll<HTMLButtonElement>("[data-clipboard-item]")[1];
  fireEvent.pointerEnter(entry, { pointerType: "mouse" });
  act(() => vi.advanceTimersByTime(400));
  view.rerender(<EmojiClipboardEntries items={[...items]} {...props} />);
  expect(view.container.querySelector("[data-clipboard-preview]")).not.toBeNull();
  fireEvent.contextMenu(entry, { clientX: 10, clientY: 10 });
  view.rerender(<EmojiClipboardEntries items={[...items]} {...props} />);
  expect(screen.getByRole("menu")).toBeTruthy();
  view.rerender(<EmojiClipboardEntries items={[items[1]]} {...props} />);
  expect(screen.queryByRole("menu")).toBeNull();
  expect(view.container.querySelector("[data-clipboard-preview]")).toBeNull();
});

test("没有悬停的触屏设备上用行旁带文字的「删除」，不用要悬停才出现的小 ×", () => {
  vi.stubGlobal(
    "matchMedia",
    vi.fn((query: string) => ({ matches: query === "(hover: none)", media: query })),
  );
  try {
    const callbacks = actions();
    const view = render(
      <EmojiClipboardEntries items={items} busy={false} canCopy {...callbacks} />,
    );
    const row = view.container.querySelector<HTMLElement>("[data-clipboard-row]")!;
    expect(
      within(row)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["合成  记录 下一行  ", "粘贴", "删除"]);
    // 行内不再给 × 留出右侧空位。
    expect(row.querySelector("[data-clipboard-item]")?.className).not.toContain("pr-10");
    fireEvent.click(within(row).getByRole("button", { name: "删除此条历史记录" }));
    expect(callbacks.onRemove.mock.calls).toEqual([[raw]]);
  } finally {
    vi.unstubAllGlobals();
  }
});
