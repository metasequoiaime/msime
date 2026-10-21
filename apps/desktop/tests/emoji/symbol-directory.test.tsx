// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EmojiPanel } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.unstubAllGlobals();
});

function fixture(sendText = vi.fn().mockResolvedValue(undefined)) {
  return {
    close: async () => {},
    sendText,
    copyText: vi.fn().mockResolvedValue(undefined),
    loadCatalog: async () => ({
      emoji: [],
      kaomoji: [],
      symbols: [
        {
          title: "Stars",
          parent: "Stars and shapes",
          icon: "★",
          items: [{ text: "★", keywords: "星形" }],
        },
        {
          title: "Numbers",
          parent: "Math",
          icon: "①",
          items: Array.from({ length: 70 }, (_, index) => ({
            text: `①${index}`,
            keywords: `序号${index}`,
          })),
        },
        {
          title: "Greek letters",
          parent: "Letters",
          icon: "α",
          items: [{ text: "α", keywords: "alpha" }],
        },
        {
          title: "Latin letters",
          parent: "Letters",
          icon: "À",
          items: [{ text: "À", keywords: "latin" }],
        },
      ],
    }),
  };
}

async function open(client = fixture()) {
  render(<EmojiPanel client={client} />);
  fireEvent.click(screen.getByRole("button", { name: "符号" }));
  await screen.findByRole("button", { name: "数字序号" });
  return client;
}

test("符号大全显示中文侧栏，分类切换清空搜索且完整展示长分类", async () => {
  await open();
  const categories = screen.getByRole("navigation", { name: "符号子分类" });
  expect(within(categories).getByRole("button", { name: "星形" })).toBeDefined();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), { target: { value: "数学" } });
  expect(screen.queryByRole("button", { name: "★" })).toBeNull();
  expect(screen.getByRole("button", { name: "①69" })).toBeDefined();
  fireEvent.click(within(categories).getByRole("button", { name: "星形" }));
  expect((screen.getByRole("textbox", { name: "搜索" }) as HTMLInputElement).value).toBe("");
  expect(screen.getByRole("button", { name: "★" })).toBeDefined();
  expect(screen.queryByRole("button", { name: "①69" })).toBeNull();
});

test("希腊和拉丁叶分类直接可选且父级中文搜索仍跨组匹配", async () => {
  await open();
  const categories = screen.getByRole("navigation", { name: "符号子分类" });
  const directory = screen.getByRole("region", { name: "符号目录" });
  fireEvent.click(within(categories).getByRole("button", { name: "希腊字母" }));
  expect(within(directory).getByRole("button", { name: "α" })).toBeDefined();
  expect(within(directory).queryByRole("button", { name: "À" })).toBeNull();
  fireEvent.click(within(categories).getByRole("button", { name: "拉丁字母" }));
  expect(within(directory).queryByRole("button", { name: "α" })).toBeNull();
  expect(within(directory).getByRole("button", { name: "À" })).toBeDefined();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), {
    target: { value: "字母与文字" },
  });
  expect(within(directory).getByRole("button", { name: "α" })).toBeDefined();
  expect(within(directory).getByRole("button", { name: "À" })).toBeDefined();
});

test("不同父级下的同名叶分类保持独立，插件名称不作为内置名称翻译", async () => {
  const client = fixture();
  const original = await client.loadCatalog();
  client.loadCatalog = async () => ({
    ...original,
    symbols: [
      ...original.symbols,
      { title: "Numbers", parent: "More", icon: "Ⅱ", items: [{ text: "Ⅱ", keywords: "roman" }] },
      {
        title: "合成插件",
        parent: "Math",
        pack: "synthetic",
        icon: "→",
        items: [{ text: "→", keywords: "arrow" }],
      },
    ],
  });
  // 同名入口不适合按全局名称等待，目录按实际资源异步加载。
  render(<EmojiPanel client={client} />);
  fireEvent.click(screen.getByRole("button", { name: "符号" }));
  const categories = screen.getByRole("navigation", { name: "符号子分类" });
  const matches = await within(categories).findAllByRole("button", { name: "数字序号" });
  expect(matches).toHaveLength(2);
  const directory = screen.getByRole("region", { name: "符号目录" });
  fireEvent.click(matches[0]);
  expect(within(directory).queryByRole("button", { name: "Ⅱ" })).toBeNull();
  fireEvent.click(matches[1]);
  expect(within(directory).getByRole("button", { name: "Ⅱ" })).toBeDefined();
  expect(within(directory).queryByRole("button", { name: "①69" })).toBeNull();
  fireEvent.click(within(categories).getByRole("button", { name: "Math" }));
  expect(within(directory).getByRole("button", { name: "→" })).toBeDefined();
  expect(within(directory).queryByRole("button", { name: "Ⅱ" })).toBeNull();
});

test("面板边缘缩放不吞内部点选，宿主拒绝时显示提示", async () => {
  vi.stubGlobal("PointerEvent", MouseEvent);
  const resizeWindow = vi.fn().mockResolvedValue(undefined);
  const client = { ...fixture(), resizeWindow };
  await open(client);
  const panel = screen.getByRole("main", { name: "表情与符号" });
  vi.spyOn(panel, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: 720,
    bottom: 720,
    width: 720,
    height: 720,
    x: 0,
    y: 0,
    toJSON() {},
  });
  fireEvent.pointerDown(panel, { button: 0, clientX: 300, clientY: 300 });
  fireEvent.pointerDown(panel, { button: 2, clientX: 719, clientY: 719 });
  expect(resizeWindow).not.toHaveBeenCalled();
  for (const [x, y, edge] of [
    [360, 1, "n"],
    [360, 719, "s"],
    [719, 360, "e"],
    [1, 360, "w"],
    [719, 1, "ne"],
    [1, 1, "nw"],
    [719, 719, "se"],
    [1, 719, "sw"],
  ] as const) {
    fireEvent.pointerDown(panel, { button: 0, clientX: x, clientY: y });
    expect(resizeWindow).toHaveBeenLastCalledWith(edge);
  }
  fireEvent.click(
    within(screen.getByRole("region", { name: "符号目录" })).getByRole("button", { name: "α" }),
  );
  await waitFor(() => expect(client.sendText).toHaveBeenCalledWith("α"));
  resizeWindow.mockRejectedValueOnce(new Error("synthetic rejection"));
  fireEvent.pointerDown(panel, { button: 0, clientX: 719, clientY: 719 });
  await screen.findByText("无法调整窗口大小，请重试。");
});

test("点击默认输入，成功后记录并恢复最近符号，复制仍可选择", async () => {
  const client = await open();
  const directory = screen.getByRole("region", { name: "符号目录" });
  fireEvent.click(within(directory).getByRole("button", { name: "★" }));
  await waitFor(() => expect(client.sendText).toHaveBeenCalledWith("★"));
  await waitFor(() =>
    expect(
      within(screen.getByRole("region", { name: "最近使用的符号" })).getByRole("button", {
        name: "★",
      }),
    ).toBeDefined(),
  );
  expect(client.copyText).not.toHaveBeenCalled();
  cleanup();
  await open(client);
  expect(
    within(screen.getByRole("region", { name: "最近使用的符号" })).getByRole("button", {
      name: "★",
    }),
  ).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: /^复制$/ }));
  fireEvent.click(
    within(screen.getByRole("region", { name: "符号目录" })).getByRole("button", { name: "★" }),
  );
  await waitFor(() => expect(client.copyText).toHaveBeenCalledWith("★"));
  fireEvent.click(screen.getByRole("button", { name: "清除符号记录" }));
  expect(localStorage.getItem("msime.symbols.recent")).toBe("[]");
});

test("输入被拒绝时不写最近使用", async () => {
  await open(fixture(vi.fn().mockRejectedValue(new Error("synthetic rejection"))));
  fireEvent.click(
    within(screen.getByRole("region", { name: "符号目录" })).getByRole("button", { name: "★" }),
  );
  await screen.findByText("输入失败，可切换复制后手动粘贴");
  expect(localStorage.getItem("msime.symbols.recent")).toBe("[]");
});

test("符号搜索支持 Ctrl+F 和下方向键进入网格", async () => {
  await open();
  const search = screen.getByRole("textbox", { name: "搜索" });
  const first = within(screen.getByRole("region", { name: "符号目录" })).getByRole("button", {
    name: "★",
  });
  search.scrollIntoView = vi.fn();
  first.scrollIntoView = vi.fn();
  first.focus();
  fireEvent.keyDown(first, { key: "f", ctrlKey: true });
  expect(document.activeElement).toBe(search);
  fireEvent.keyDown(search, { key: "ArrowDown" });
  expect(document.activeElement).toBe(first);
});
