// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { DesktopEmojiPanel } from "../../src/input/desktop-emoji-panel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn(() => true) }));

afterEach(() => {
  cleanup();
  localStorage.clear();
  vi.resetAllMocks();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => (resolve = accept));
  return { promise, resolve };
}

function panelClient(text = "合成剪贴板历史") {
  return {
    close: vi.fn(async () => {}),
    copyText: vi.fn(async (_text: string) => {}),
    loadCatalog: vi.fn(async () => ({ emoji: [], kaomoji: [], symbols: [] })),
    clipboard: {
      list: vi.fn(async () => [text]),
      copy: vi.fn(async (_text: string) => {}),
      onChanged: vi.fn(async () => vi.fn()),
    },
  };
}

function tauri(supported: boolean) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockResolvedValue(supported);
}

test("能力查询结束前显示连接状态，随后透传初始页和主题", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  const capability = deferred<boolean>();
  vi.mocked(invoke).mockReturnValue(capability.promise);
  const client = panelClient();
  render(<DesktopEmojiPanel client={client} theme="light" initialPage="clipboard" />);
  expect(screen.getByRole("status").textContent).toBe("正在连接面板…");
  expect(client.clipboard.list).not.toHaveBeenCalled();
  await act(async () => capability.resolve(true));
  expect(await screen.findByText("合成剪贴板历史")).toBeTruthy();
  expect(screen.getByRole("main", { name: "表情与符号" }).getAttribute("data-panel-theme")).toBe(
    "light",
  );
  expect(screen.getByRole("button", { name: "粘贴此条记录到原应用" })).toBeTruthy();
});

test("宿主粘贴与复制分别执行，原客户端不被修改", async () => {
  tauri(true);
  const client = panelClient();
  render(<DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" />);
  const entry = await screen.findByText("合成剪贴板历史");
  fireEvent.click(screen.getByRole("button", { name: "粘贴此条记录到原应用" }));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("paste_clipboard_text", { text: "合成剪贴板历史" }),
  );
  expect(client.clipboard.copy).not.toHaveBeenCalled();
  await waitFor(() => expect((entry as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(entry);
  await waitFor(() => expect(client.clipboard.copy).toHaveBeenCalledWith("合成剪贴板历史"));
  expect(client.copyText).not.toHaveBeenCalled();
  expect(client.clipboard).not.toHaveProperty("paste");
});

test.each(["browser", "unsupported", "failure"] as const)(
  "%s 不新增宿主粘贴动作，仍可复制历史",
  async (mode) => {
    vi.mocked(isTauri).mockReturnValue(mode !== "browser");
    if (mode === "failure") vi.mocked(invoke).mockRejectedValue(new Error("合成能力查询失败"));
    else vi.mocked(invoke).mockResolvedValue(false);
    const client = panelClient();
    render(<DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" />);
    fireEvent.click(await screen.findByText("合成剪贴板历史"));
    await waitFor(() => expect(client.clipboard.copy).toHaveBeenCalledWith("合成剪贴板历史"));
    expect(screen.queryByRole("button", { name: "粘贴此条记录到原应用" })).toBeNull();
    if (mode === "browser") expect(invoke).not.toHaveBeenCalled();
    else expect(invoke).toHaveBeenCalledExactlyOnceWith("supports_clipboard_paste");
  },
);

test.each([false, true])("宿主粘贴能力为 %s 时正确选择调用方或桌面动作", async (supported) => {
  tauri(supported);
  const paste = vi.fn(async (_text: string) => {});
  const base = panelClient();
  const client = { ...base, clipboard: { ...base.clipboard, paste } };
  render(<DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" />);
  fireEvent.click(await screen.findByRole("button", { name: "粘贴此条记录到原应用" }));
  if (supported) {
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("paste_clipboard_text", { text: "合成剪贴板历史" }),
    );
    expect(paste).not.toHaveBeenCalled();
  } else {
    await waitFor(() => expect(paste).toHaveBeenCalledExactlyOnceWith("合成剪贴板历史"));
    expect(invoke).not.toHaveBeenCalledWith("paste_clipboard_text", expect.anything());
  }
});

test("支持粘贴但没有剪贴板客户端时不添加粘贴动作", async () => {
  tauri(true);
  const original = panelClient();
  const client = { close: original.close, loadCatalog: original.loadCatalog };
  render(<DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" />);
  expect(await screen.findByRole("main", { name: "表情与符号" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "粘贴此条记录到原应用" })).toBeNull();
  expect(original.clipboard.list).not.toHaveBeenCalled();
});

test.each([
  ["home", "emoji-panel"],
  ["clipboard", "clipboard-panel"],
] as const)("%s 关闭对应独立窗口", async (initialPage, label) => {
  tauri(false);
  const client = panelClient();
  render(<DesktopEmojiPanel client={client} theme="dark" initialPage={initialPage} />);
  fireEvent.click(await screen.findByRole("button", { name: "关闭" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("close_panel", { label }));
  expect(client.close).not.toHaveBeenCalled();
});

test("设置页内面板使用调用方关闭回调", async () => {
  tauri(false);
  const client = panelClient();
  const close = vi.fn(async () => {});
  render(<DesktopEmojiPanel client={client} theme="dark" close={close} />);
  fireEvent.click(await screen.findByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
  expect(client.close).not.toHaveBeenCalled();
  expect(invoke).not.toHaveBeenCalledWith("close_panel", expect.anything());
});

test("旧客户端的迟到能力回答不能替换新面板", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  const old = deferred<boolean>();
  vi.mocked(invoke).mockReturnValueOnce(old.promise).mockResolvedValue(false);
  const previous = panelClient("合成旧历史");
  const next = panelClient("合成新历史");
  const view = render(<DesktopEmojiPanel client={previous} theme="dark" initialPage="clipboard" />);
  view.rerender(<DesktopEmojiPanel client={next} theme="dark" initialPage="clipboard" />);
  expect(await screen.findByText("合成新历史")).toBeTruthy();
  await act(async () => old.resolve(true));
  expect(screen.queryByText("合成旧历史")).toBeNull();
  expect(screen.queryByRole("button", { name: "粘贴此条记录到原应用" })).toBeNull();
  expect(previous.clipboard.list).not.toHaveBeenCalled();
});

test("StrictMode 重放与卸载后忽略能力回答", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  const first = deferred<boolean>();
  const second = deferred<boolean>();
  vi.mocked(invoke).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const client = panelClient();
  const view = render(
    <StrictMode>
      <DesktopEmojiPanel client={client} theme="dark" />
    </StrictMode>,
  );
  await act(async () => first.resolve(true));
  expect(screen.getByRole("status").textContent).toBe("正在连接面板…");
  view.unmount();
  await act(async () => second.resolve(true));
  expect(client.loadCatalog).not.toHaveBeenCalled();
});

/** 按命令名回答的宿主：粘贴能力、采集状态和云剪贴板请求各有各的结果。 */
function linuxHost(captureStatus: string | null = "read_unavailable_x11") {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "supports_clipboard_paste") return true;
    if (command === "clipboard_capture_status") return captureStatus;
    return undefined;
  });
}

test("Linux 的独立剪贴板窗口用自己的页头，不显示表情分类，并显示采集状态", async () => {
  linuxHost();
  const client = panelClient();
  render(
    <DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" hostPlatform="linux" />,
  );
  const main = await screen.findByRole("main", { name: "剪贴板" });
  expect(main.querySelector("[data-panel-title], h1")?.textContent ?? main.textContent).toContain(
    "剪贴板",
  );
  expect(screen.queryByRole("navigation", { name: "面板分类" })).toBeNull();
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
  expect(
    await screen.findByText("读不到系统剪贴板，剪贴板采集已暂停：请安装 xclip 或 xsel"),
  ).toBeTruthy();
});

test.each([
  ["read_unavailable_wayland", "读不到系统剪贴板，剪贴板采集已暂停：请安装 wl-clipboard"],
  ["read_unavailable", "当前会话里读不到系统剪贴板，剪贴板采集已暂停"],
  ["monitoring", "历史开启时，保存新复制的文本"],
] as const)("Linux 剪贴板采集状态 %s 按会话给出提示", async (status, text) => {
  linuxHost(status);
  render(
    <DesktopEmojiPanel
      client={panelClient()}
      theme="dark"
      initialPage="clipboard"
      hostPlatform="linux"
    />,
  );
  expect(await screen.findByText(text)).toBeTruthy();
});

test("Linux 粘贴成功后关闭独立窗口，与 macOS 一样只插入一次", async () => {
  linuxHost("monitoring");
  const client = panelClient();
  render(
    <DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" hostPlatform="linux" />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "粘贴此条记录到原应用" }));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("close_panel", { label: "clipboard-panel" }),
  );
  expect(invoke).toHaveBeenCalledWith("paste_clipboard_text", { text: "合成剪贴板历史" });
});

test("Linux 输入到原应用成功后关闭表情窗口，关窗前记下最近使用；复制不关闭", async () => {
  linuxHost();
  const sendText = vi.fn(async (_text: string) => {});
  const client = {
    ...panelClient(),
    sendText,
    loadCatalog: vi.fn(async () => ({
      emoji: [{ title: "合成目录", icon: "★", items: [{ text: "★", keywords: "合成星" }] }],
      kaomoji: [],
      symbols: [],
    })),
  };
  render(<DesktopEmojiPanel client={client} theme="dark" hostPlatform="linux" />);
  fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
  fireEvent.click(await screen.findByRole("button", { name: "★" }));
  await waitFor(() => expect(client.copyText).toHaveBeenCalledWith("★"));
  expect(invoke).not.toHaveBeenCalledWith("close_panel", expect.anything());
  fireEvent.click(screen.getByRole("button", { name: "输入到原应用" }));
  fireEvent.click(screen.getByRole("button", { name: "★" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("close_panel", { label: "emoji-panel" }));
  expect(sendText).toHaveBeenCalledWith("★");
  expect(JSON.parse(localStorage.getItem("msime.emoji.recent") ?? "[]")[0]).toEqual({
    text: "★",
    keywords: "合成星",
  });
});

test("剪贴板行的右键菜单经 cloud_clipboard_request 发到云剪贴板", async () => {
  linuxHost("monitoring");
  const client = panelClient();
  render(
    <DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" hostPlatform="linux" />,
  );
  fireEvent.contextMenu(await screen.findByRole("button", { name: "合成剪贴板历史" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "发到云剪贴板" }));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("cloud_clipboard_request", {
      action: { operation: "add", text: "合成剪贴板历史" },
    }),
  );
  expect(await screen.findByText("已发到云剪贴板")).toBeTruthy();
});

test.each(["windows", undefined])(
  "%s 不改变原有表现：表情面板页头、不关窗、不发云剪贴板",
  async (hostPlatform) => {
    tauri(true);
    const client = panelClient();
    render(
      <DesktopEmojiPanel
        client={client}
        theme="dark"
        initialPage="clipboard"
        hostPlatform={hostPlatform}
      />,
    );
    expect(await screen.findByRole("main", { name: "表情与符号" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "面板分类" })).toBeTruthy();
    const entry = await screen.findByRole("button", { name: "合成剪贴板历史" });
    expect(entry.hasAttribute("aria-haspopup")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "粘贴此条记录到原应用" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("paste_clipboard_text", { text: "合成剪贴板历史" }),
    );
    expect(invoke).not.toHaveBeenCalledWith("close_panel", expect.anything());
  },
);

test("独立剪贴板窗口里 Esc 关闭鼠标打开的菜单，再按一次关闭窗口而不是回到表情首页", async () => {
  linuxHost("monitoring");
  const client = panelClient();
  render(
    <DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" hostPlatform="linux" />,
  );
  const entry = await screen.findByRole("button", { name: "合成剪贴板历史" });
  entry.focus();
  fireEvent.contextMenu(entry, { clientX: 30, clientY: 30 });
  expect(screen.getByRole("menu")).toBeTruthy();
  fireEvent.keyDown(entry, { key: "Escape" });
  expect(screen.queryByRole("menu")).toBeNull();
  expect(invoke).not.toHaveBeenCalledWith("close_panel", expect.anything());
  expect(screen.getByRole("main", { name: "剪贴板" })).toBeTruthy();
  fireEvent.keyDown(entry, { key: "Escape" });
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("close_panel", { label: "clipboard-panel" }),
  );
  expect(screen.queryByRole("heading", { name: "Emoji" })).toBeNull();
});

test("读不到采集状态时只是不显示状态行，列表照常显示", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "supports_clipboard_paste") return true;
    if (command === "clipboard_capture_status") throw { code: "unavailable" };
    return undefined;
  });
  const client = panelClient();
  const view = render(
    <DesktopEmojiPanel client={client} theme="dark" initialPage="clipboard" hostPlatform="linux" />,
  );
  expect(await screen.findByRole("button", { name: "合成剪贴板历史" })).toBeTruthy();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("clipboard_capture_status"));
  expect(view.container.querySelector("[data-clipboard-capture]")).toBeNull();
  expect(screen.queryByText("无法读取剪贴板历史，请点击“刷新列表”重试")).toBeNull();
});
