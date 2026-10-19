// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EmojiPanel } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const emoji = { text: "🦉", keywords: "synthetic 合成猫头鹰 owl" };
const kaomoji = { text: "(合成ヾ)", keywords: "synthetic 合成颜文字" };
const symbol = { text: "★", keywords: "synthetic 合成星形 star" };
const loadCatalog = async () => ({
  emoji: [{ title: "合成表情组", icon: "🦉", items: [emoji] }],
  kaomoji: [{ title: "合成颜文字组", icon: "ヾ", items: [kaomoji] }],
  symbols: [{ title: "Stars", parent: "Stars and shapes", icon: "★", items: [symbol] }],
});

test.each([
  ["Emoji", "Emoji 目录", emoji, false],
  ["颜文字", "颜文字目录", kaomoji, true],
] as const)("%s 目录选择原文并保存完整关键词，保留导航与布局", async (page, name, item, flow) => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(<EmojiPanel client={{ close: async () => {}, loadCatalog, copyText }} />);
  fireEvent.click(screen.getByRole("button", { name: page }));
  const directory = screen.getByRole("region", { name });
  const button = await within(directory).findByRole("button", { name: item.text });
  expect(button.title).toBe(item.keywords.split(" ")[1]);
  expect(button.hasAttribute("data-emoji-navigation-item")).toBe(true);
  expect(button.classList.contains("whitespace-pre-wrap")).toBe(flow);
  fireEvent.click(button);
  await waitFor(() => expect(copyText).toHaveBeenCalledExactlyOnceWith(item.text));
  await waitFor(() =>
    expect(JSON.parse(localStorage.getItem("msime.emoji.recent") ?? "[]")).toEqual([item]),
  );
});

test("符号目录与最近符号共用原文动作，保留导航差异和复制输入模式", async () => {
  let completeInput!: () => void;
  const pending = new Promise<void>((resolve) => (completeInput = resolve));
  const sendText = vi.fn().mockReturnValue(pending);
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(<EmojiPanel client={{ close: async () => {}, loadCatalog, sendText, copyText }} />);
  fireEvent.click(screen.getByRole("button", { name: "符号" }));
  const directory = screen.getByRole("region", { name: "符号目录" });
  const recent = screen.getByRole("region", { name: "最近使用的符号" });
  // 回退目录也有星号，等合成目录替换完成后再选择。
  await waitFor(() =>
    expect(within(directory).getByRole("button", { name: symbol.text }).title).toBe("合成星形"),
  );
  const button = within(directory).getByRole("button", { name: symbol.text });
  expect(button.hasAttribute("data-emoji-navigation-item")).toBe(true);
  fireEvent.click(button);
  expect(sendText).toHaveBeenCalledExactlyOnceWith(symbol.text);
  expect(within(recent).queryByRole("button", { name: symbol.text })).toBeNull();
  await act(async () => completeInput());
  const recentButton = await within(recent).findByRole("button", { name: symbol.text });
  expect(recentButton.title).toBe(button.title);
  expect(recentButton.hasAttribute("data-emoji-navigation-item")).toBe(false);
  expect(recentButton.classList.contains("min-w-10")).toBe(true);
  expect(JSON.parse(localStorage.getItem("msime.symbols.recent") ?? "[]")).toEqual([symbol]);
  fireEvent.click(screen.getByRole("button", { name: /^复制$/ }));
  fireEvent.click(recentButton);
  await waitFor(() => expect(copyText).toHaveBeenCalledExactlyOnceWith(symbol.text));
  expect(sendText).toHaveBeenCalledOnce();
});

test("剪贴板修改期间只禁用通用目录，符号目录和最近符号保持原可用性", async () => {
  let completeClear!: () => void;
  const pending = new Promise<void>((resolve) => (completeClear = resolve));
  localStorage.setItem("msime.symbols.recent", JSON.stringify([symbol]));
  render(
    <EmojiPanel
      initialPage="clipboard"
      client={{
        close: async () => {},
        loadCatalog,
        copyText: vi.fn().mockResolvedValue(undefined),
        clipboard: {
          list: async () => ["合成剪贴板记录"],
          clear: () => pending,
          onChanged: async () => () => {},
        },
      }}
    />,
  );
  await screen.findByRole("button", { name: "合成剪贴板记录" });
  fireEvent.click(screen.getByRole("button", { name: "清空历史" }));
  expect((screen.getByRole("button", { name: "关闭" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
  const emojiButton = await screen.findByRole("button", { name: emoji.text });
  expect((emojiButton as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "符号" }));
  for (const name of ["符号目录", "最近使用的符号"]) {
    const button = within(screen.getByRole("region", { name })).getByRole("button", {
      name: symbol.text,
    });
    expect((button as HTMLButtonElement).disabled).toBe(false);
  }
  await act(async () => completeClear());
  fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
  await waitFor(() =>
    expect((screen.getByRole("button", { name: emoji.text }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
});
