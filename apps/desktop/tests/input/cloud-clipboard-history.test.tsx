// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CloudClipboardHistory, type CloudClipboardItem } from "@msime/ui";

afterEach(cleanup);

const items: readonly CloudClipboardItem[] = [
  { id: "synthetic-first", text: "合成记录", pinned: true },
  { id: "synthetic-second", text: "合成记录", device: "synthetic-device" },
];

test("空列表显示原空态，更新条目后按顺序显示独立记录", () => {
  const props = { busy: false, copyAvailable: true, onChoose: vi.fn(), onRemove: vi.fn() };
  const view = render(<CloudClipboardHistory {...props} items={[]} />);
  expect(screen.getByLabelText("云端历史").querySelector("p")?.textContent).toBe("暂无云端历史");
  expect(screen.queryAllByRole("button")).toHaveLength(0);
  view.rerender(<CloudClipboardHistory {...props} items={items} />);
  expect(screen.queryByText("暂无云端历史")).toBeNull();
  expect(screen.getAllByRole("article")).toHaveLength(2);
  const metas = ["已置顶", "synthetic-device"];
  screen.getAllByRole("article").forEach((article, index) => {
    expect(
      within(article)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual([`合成记录${metas[index]}`, "复制", "删除"]);
  });
});

test("条目下方按 macOS 的拼法显示「已置顶 · 设备」，按钮名称仍是条目文字", () => {
  const props = { busy: false, copyAvailable: false, onChoose: vi.fn(), onRemove: vi.fn() };
  render(
    <CloudClipboardHistory
      {...props}
      items={[
        { id: "synthetic-both", text: "合成甲", pinned: true, device: "合成笔记本" },
        { id: "synthetic-none", text: "合成乙", pinned: false },
      ]}
    />,
  );
  const both = screen.getByRole("button", { name: "合成甲" });
  expect(both.querySelector("small")?.textContent).toBe("已置顶 · 合成笔记本");
  expect(document.getElementById(both.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
    "已置顶 · 合成笔记本",
  );
  const none = screen.getByRole("button", { name: "合成乙" });
  expect(none.querySelector("small")).toBeNull();
  expect(none.hasAttribute("aria-describedby")).toBe(false);
});

test("重复文本按条目转发选择与复制，删除只传对应 id", () => {
  const onChoose = vi.fn();
  const onRemove = vi.fn();
  render(
    <CloudClipboardHistory
      items={items}
      busy={false}
      copyAvailable
      onChoose={onChoose}
      onRemove={onRemove}
    />,
  );
  const articles = screen.getAllByRole("article");
  fireEvent.click(within(articles[0]).getByRole("button", { name: "合成记录" }));
  fireEvent.click(within(articles[1]).getByRole("button", { name: "合成记录" }));
  fireEvent.click(within(articles[1]).getByRole("button", { name: "复制此条云端记录" }));
  fireEvent.click(within(articles[1]).getByRole("button", { name: "删除 合成记录" }));
  expect(onChoose.mock.calls).toEqual([[items[0]], [items[1]], [items[1], true]]);
  expect(onChoose.mock.calls[1][0]).toBe(items[1]);
  expect(onRemove.mock.calls).toEqual([[items[1].id]]);
});

test("忙时所有动作禁用，解除忙状态后恢复", () => {
  const onChoose = vi.fn();
  const onRemove = vi.fn();
  const props = { items, copyAvailable: true, onChoose, onRemove };
  const view = render(<CloudClipboardHistory {...props} busy />);
  for (const button of screen.getAllByRole("button")) {
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
  }
  expect(onChoose).not.toHaveBeenCalled();
  expect(onRemove).not.toHaveBeenCalled();
  view.rerender(<CloudClipboardHistory {...props} busy={false} />);
  for (const button of screen.getAllByRole("button"))
    expect((button as HTMLButtonElement).disabled).toBe(false);
});

test("复制能力变化只控制复制按钮", () => {
  const props = { items, busy: false, onChoose: vi.fn(), onRemove: vi.fn() };
  const view = render(<CloudClipboardHistory {...props} copyAvailable={false} />);
  expect(screen.queryByRole("button", { name: "复制此条云端记录" })).toBeNull();
  expect(screen.getAllByRole("button")).toHaveLength(4);
  fireEvent.click(screen.getAllByRole("button", { name: "合成记录" })[0]);
  expect(props.onChoose.mock.calls).toEqual([[items[0]]]);
  view.rerender(<CloudClipboardHistory {...props} copyAvailable />);
  expect(screen.getAllByRole("button", { name: "复制此条云端记录" })).toHaveLength(2);
});
