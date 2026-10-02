// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EmojiPanel } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

test("emoji panel restores persisted recent items", () => {
  localStorage.setItem("msime.emoji.recent", JSON.stringify([{ text: "⚙", keywords: "gear" }]));
  render(<EmojiPanel client={{ close: async () => {} }} />);
  expect(screen.getByRole("button", { name: "最近使用" })).toBeDefined();
});

test("emoji panel ignores malformed recent storage", () => {
  localStorage.setItem("msime.emoji.recent", "not json");
  render(<EmojiPanel client={{ close: async () => {} }} />);
  expect(screen.queryByRole("button", { name: "⚙" })).toBeNull();
});

test("emoji panel paginates catalog items and resets on search", async () => {
  const items = Array.from({ length: 60 }, (_, index) => ({
    text: `😀${index}`,
    keywords: `item${index}`,
  }));
  const client = {
    close: async () => {},
    loadCatalog: async () => ({
      emoji: [{ title: "All", icon: "😀", items }],
      kaomoji: [],
      symbols: [],
    }),
  };
  render(<EmojiPanel client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
  await waitFor(() => expect(screen.getByText("第 1 / 2 页")).toBeDefined());
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await waitFor(() => expect(screen.getByText("第 2 / 2 页")).toBeDefined());
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), { target: { value: "item0" } });
  await waitFor(() => expect(screen.queryByText("第 2 / 2 页")).toBeNull());
});

test("clipboard panel exposes host paste and keeps copy separate", async () => {
  const paste = vi.fn().mockResolvedValue(undefined);
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <EmojiPanel
      client={{
        close: async () => {},
        copyText,
        clipboard: { list: async () => ["synthetic clipboard entry"], paste },
      }}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText("synthetic clipboard entry")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "粘贴此条记录到原应用" }));
  await waitFor(() => expect(paste).toHaveBeenCalledWith("synthetic clipboard entry"));
  expect(copyText).not.toHaveBeenCalled();
});

const recentCatalogClient = {
  close: async () => {},
  loadCatalog: async () => ({
    emoji: [{ title: "Smileys", icon: "😀", items: [{ text: "😀", keywords: "grinning" }] }],
    kaomoji: [],
    symbols: [],
  }),
};

async function openRecentTab() {
  render(<EmojiPanel client={recentCatalogClient} />);
  fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
  fireEvent.click(await screen.findByRole("button", { name: "◷ 最近使用" }));
}

test("empty recent tab shows the source hint with or without a search", async () => {
  await openRecentTab();
  expect(await screen.findByText("Your recently used items will appear here")).toBeDefined();
  expect(screen.queryByText("暂无可显示内容")).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), {
    target: { value: "synthetic" },
  });
  await waitFor(() =>
    expect(screen.getByText("Your recently used items will appear here")).toBeDefined(),
  );
  expect(screen.queryByText("No results")).toBeNull();
});

test("recent tab with history but no search match shows no results", async () => {
  localStorage.setItem("msime.emoji.recent", JSON.stringify([{ text: "⚙", keywords: "gear" }]));
  await openRecentTab();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), {
    target: { value: "synthetic" },
  });
  expect(await screen.findByText("No results")).toBeDefined();
  expect(screen.queryByText("Your recently used items will appear here")).toBeNull();
});

test("plugin symbol groups keep their own category and search by group keywords", async () => {
  const client = {
    close: async () => {},
    loadCatalog: async () => ({
      emoji: [],
      kaomoji: [],
      symbols: [
        {
          title: "箭头",
          parent: "箭头",
          icon: "←",
          items: [{ text: "←", keywords: "左箭头" }],
        },
        {
          title: "箭头",
          parent: "箭头",
          pack: "arrows",
          keywords: "jiantou",
          icon: "⇒",
          items: [{ text: "⇒", keywords: "⇒" }],
        },
      ],
    }),
  };
  render(<EmojiPanel client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "符号" }));
  const categories = await screen.findByRole("navigation", { name: "符号子分类" });
  await waitFor(() =>
    expect(within(categories).getAllByRole("button", { name: /箭头/ })).toHaveLength(2),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), {
    target: { value: "jiantou" },
  });
  await waitFor(() => expect(screen.queryByRole("button", { name: "←" })).toBeNull());
  // 组的关键词只用于搜索：各项仍以自己的文本作提示。
  expect(screen.getByRole("button", { name: "⇒" }).getAttribute("title")).toBe("⇒");
});
