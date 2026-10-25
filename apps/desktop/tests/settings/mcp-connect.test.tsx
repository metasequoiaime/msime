// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  McpConnectSection,
  type McpFlag,
  type McpInstallOutcome,
  SettingsPage,
  type McpServerStatus,
  type SettingsClient,
  type Snapshot,
} from "@msime/ui";

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** 记住的选择是两项都关，用于只关心只读条目本身的用例。 */
function readOnly() {
  window.localStorage.setItem("msime.mcp.flags", "[]");
}

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const config = `{
  "mcpServers": {
    "msime": {
      "command": "/opt/msime/msime-mcp",
      "args": ["--options", "/state/runtime-options.json"]
    }
  }
}`;

function status(configured = false, flags: McpFlag[] = [], stale = false): McpServerStatus {
  return {
    command: "/opt/msime/msime-mcp",
    installed: true,
    options: "/state/runtime-options.json",
    config,
    clients: [
      {
        id: "claude_desktop",
        path: "/home/someone/Library/Application Support/Claude/claude_desktop_config.json",
        configured,
        flags: configured ? flags : [],
        stale: configured && stale,
      },
      {
        id: "cursor",
        path: "/home/someone/.cursor/mcp.json",
        configured: false,
        flags: [],
        stale: false,
      },
    ],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function openMcp(extra: Partial<SettingsClient>) {
  const hasMcp = typeof extra.mcpServerStatus === "function";
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "macos" }),
        ...extra,
      }}
      initialPage={hasMcp ? "mcp" : undefined}
    />,
  );
  await settingsFormReady();
}

/** 等待宿主状态填入连接分组；手机的外层 fieldset 同名，因此取最里面的一组。 */
async function mcpGroup() {
  const group = (await screen.findAllByRole("group", { name: "连接 AI 助手" })).at(-1)!;
  await within(group).findByRole("radio", { name: "Claude Code" });
  return group;
}

test("a host without the server shows no section", async () => {
  await openMcp({});
  expect(screen.queryByRole("group", { name: "连接 AI 助手" })).toBeNull();
  expect(screen.queryByRole("tab", { name: "连接助手" })).toBeNull();
});

test("AI 顶部标签保留客户端与权限选择，切换不丢草稿", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "macos" }),
        mcpServerStatus: async () => status(),
        installMcpClient: vi.fn(),
      }}
      initialPage="mcp"
    />,
  );
  await settingsFormReady();
  const group = await mcpGroup();
  fireEvent.click(within(group).getByRole("radio", { name: "Cursor" }));
  fireEvent.click(within(group).getByRole("switch", { name: "允许读取词库" }));
  expect(within(group).getByRole("radio", { name: "Cursor" })).toHaveProperty("checked", true);
  expect(within(group).getByRole("switch", { name: "允许读取词库" })).toHaveProperty(
    "checked",
    false,
  );
  expect(screen.getByRole("heading", { name: "AI" })).toBeTruthy();
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  expect(within(sidebar).getByRole("button", { name: "AI" })).toBeTruthy();
  expect(within(sidebar).queryByRole("button", { name: "AI 助手" })).toBeNull();
  const connection = screen.getByRole("tab", { name: "连接助手" });
  expect(connection.getAttribute("aria-selected")).toBe("true");
  fireEvent.keyDown(connection, { key: "Home" });
  const assistance = screen.getByRole("tab", { name: "输入辅助" });
  expect(assistance.getAttribute("aria-selected")).toBe("true");
  const endpoint = screen.getByLabelText("AI 接口地址") as HTMLInputElement;
  fireEvent.change(endpoint, {
    target: { value: "https://synthetic.example/v1/chat/completions" },
  });
  fireEvent.keyDown(assistance, { key: "ArrowRight" });
  expect(within(await mcpGroup()).getByRole("radio", { name: "Cursor" })).toHaveProperty(
    "checked",
    true,
  );
  expect(within(group).getByRole("switch", { name: "允许读取词库" })).toHaveProperty(
    "checked",
    false,
  );
  fireEvent.click(assistance);
  expect(endpoint.value).toBe("https://synthetic.example/v1/chat/completions");
});

test("桌面旧 MCP 路由和同页重复请求打开 AI 连接标签", async () => {
  const client: SettingsClient = {
    load: async () => snapshot,
    save: vi.fn(),
    host: testHost({ platform: "linux" }),
    mcpServerStatus: async () => status(),
  };
  const view = render(<SettingsPage client={client} initialPage="mcp" />);
  await settingsFormReady();
  expect(screen.getByRole("heading", { name: "AI" })).toBeTruthy();
  expect(await mcpGroup()).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /^(维护与诊断|维护诊断)$/ }));
  expect(screen.queryByRole("group", { name: "连接 AI 助手" })).toBeNull();
  view.rerender(
    <SettingsPage client={client} initialPage="mcp" route={{ page: "mcp", nonce: 1 }} />,
  );
  expect(screen.getByRole("heading", { name: "AI" })).toBeTruthy();
  expect(await mcpGroup()).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "输入辅助" }));
  expect(screen.queryByRole("group", { name: "连接 AI 助手" })).toBeNull();
  view.rerender(<SettingsPage client={client} route={{ page: "mcp", nonce: 2 }} />);
  expect(await mcpGroup()).toBeTruthy();
  view.rerender(<SettingsPage client={client} route={{ page: "ai", nonce: 3 }} />);
  expect(screen.getByRole("tab", { name: "输入辅助" }).getAttribute("aria-selected")).toBe("true");
  view.rerender(<SettingsPage client={client} route={{ page: "developer", nonce: 4 }} />);
  expect(screen.getByRole("heading", { name: /^(维护与诊断|维护诊断)$/ })).toBeTruthy();
});

test("没有 MCP 能力时旧路由回退到输入页，不显示空连接标签", async () => {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), host: testHost({ platform: "macos" }) }}
      initialPage="mcp"
    />,
  );
  await settingsFormReady();
  await screen.findByRole("heading", { name: "输入" });
  expect(screen.queryByRole("tab", { name: "连接助手" })).toBeNull();
});

test("手机设置列表提供独立的 AI 助手入口", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "android" }),
        mcpServerStatus: async () => status(),
      }}
      initialPage="developer"
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "连接 AI 助手" }));
  expect(await mcpGroup()).toBeTruthy();
  expect(screen.getAllByRole("heading", { name: "连接 AI 助手" })).toHaveLength(2);
});

test("the entry is shown and copied as the host reports it", async () => {
  readOnly();
  const copyText = vi.fn(async () => {});
  await openMcp({ mcpServerStatus: async () => status(), copyText });
  const group = await mcpGroup();
  expect(group.textContent).toContain("--allow-write");
  // Without an install callback there is nothing to write into, so there is no tab for it either.
  expect(within(group).queryByRole("radio", { name: "Cursor" })).toBeNull();
  fireEvent.click(await within(group).findByRole("radio", { name: "其他" }));
  expect(within(group).getByLabelText("MCP 配置").textContent).toBe(config);
  fireEvent.click(within(group).getByRole("button", { name: "复制配置" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith(config));
});

test("both permissions are on by default and the commands carry them", async () => {
  await openMcp({ mcpServerStatus: async () => status() });
  const group = await mcpGroup();
  for (const name of ["允许修改设置", "允许读取词库"])
    expect(((await within(group).findByRole("switch", { name })) as HTMLInputElement).checked).toBe(
      true,
    );
  expect(within(group).getByLabelText("Claude Code 安装命令").textContent).toBe(
    "claude mcp add --scope user msime -- /opt/msime/msime-mcp --options /state/runtime-options.json --allow-write --allow-dictionary-read",
  );
});

test("the switches are remembered across visits", async () => {
  const view = render(<McpConnectSection status={() => Promise.resolve(status())} />);
  fireEvent.click(await screen.findByRole("switch", { name: "允许读取词库" }));
  view.unmount();
  expect(JSON.parse(window.localStorage.getItem("msime.mcp.flags")!)).toEqual(["--allow-write"]);

  render(<McpConnectSection status={() => Promise.resolve(status())} />);
  expect(
    ((await screen.findByRole("switch", { name: "允许读取词库" })) as HTMLInputElement).checked,
  ).toBe(false);
  expect((screen.getByRole("switch", { name: "允许修改设置" }) as HTMLInputElement).checked).toBe(
    true,
  );
  expect(screen.getByLabelText("Claude Code 安装命令").textContent).toMatch(/ --allow-write$/);
});

test("the install commands are built from the host's paths and follow the permission switches", async () => {
  readOnly();
  const copyText = vi.fn(async () => {});
  await openMcp({ mcpServerStatus: async () => status(), copyText });
  const group = await mcpGroup();
  expect(within(group).getByLabelText("Claude Code 安装命令").textContent).toBe(
    "claude mcp add --scope user msime -- /opt/msime/msime-mcp --options /state/runtime-options.json",
  );
  fireEvent.click(within(group).getByRole("button", { name: "复制命令" }));
  await waitFor(() =>
    expect(copyText).toHaveBeenCalledWith(
      "claude mcp add --scope user msime -- /opt/msime/msime-mcp --options /state/runtime-options.json",
    ),
  );

  // The switches add their flags in a fixed order, whatever order they are turned on in.
  fireEvent.click(within(group).getByRole("switch", { name: "允许读取词库" }));
  fireEvent.click(within(group).getByRole("switch", { name: "允许修改设置" }));
  fireEvent.click(await within(group).findByRole("radio", { name: "Codex" }));
  expect(within(group).getByLabelText("Codex 安装命令").textContent).toBe(
    "codex mcp add msime -- /opt/msime/msime-mcp --options /state/runtime-options.json --allow-write --allow-dictionary-read",
  );

  fireEvent.click(await within(group).findByRole("radio", { name: "其他" }));
  expect(JSON.parse(within(group).getByLabelText("MCP 配置").textContent!)).toEqual({
    mcpServers: {
      msime: {
        command: "/opt/msime/msime-mcp",
        args: [
          "--options",
          "/state/runtime-options.json",
          "--allow-write",
          "--allow-dictionary-read",
        ],
      },
    },
  });
});

test("each assistant tab offers the command that removes an earlier registration", async () => {
  const copyText = vi.fn(async () => {});
  await openMcp({ mcpServerStatus: async () => status(), copyText });
  const group = await mcpGroup();
  expect(within(group).getByLabelText("Claude Code 移除命令").textContent).toBe(
    "claude mcp remove --scope user msime",
  );
  fireEvent.click(within(group).getByRole("button", { name: "复制移除命令" }));
  await waitFor(() =>
    expect(copyText).toHaveBeenCalledWith("claude mcp remove --scope user msime"),
  );
  // Only the button that was pressed reads as copied.
  expect(within(group).getByRole("button", { name: "复制命令" })).toBeTruthy();

  fireEvent.click(await within(group).findByRole("radio", { name: "Codex" }));
  expect(within(group).getByLabelText("Codex 移除命令").textContent).toBe("codex mcp remove msime");
});

test("paths with spaces are quoted for the shell they are pasted into", async () => {
  readOnly();
  const posix = {
    ...status(),
    command: "/Applications/水杉 输入法.app/msime-mcp",
    options: "/it's/options.json",
  };
  const view = render(<McpConnectSection status={() => Promise.resolve(posix)} />);
  expect((await screen.findByLabelText("Claude Code 安装命令")).textContent).toBe(
    "claude mcp add --scope user msime -- '/Applications/水杉 输入法.app/msime-mcp' --options '/it'\\''s/options.json'",
  );
  view.unmount();

  const windows = {
    ...status(),
    command: "C:\\Program Files\\MSIME\\msime-mcp.exe",
    options: "C:\\Users\\someone\\options.json",
  };
  render(<McpConnectSection status={() => Promise.resolve(windows)} />);
  expect((await screen.findByLabelText("Claude Code 安装命令")).textContent).toBe(
    'claude mcp add --scope user msime -- "C:\\Program Files\\MSIME\\msime-mcp.exe" --options "C:\\Users\\someone\\options.json"',
  );
});

test("writing adds the entry and a different one is replaced only after confirming", async () => {
  readOnly();
  let current = status();
  const installMcpClient = vi.fn(async (client: string, replace: boolean) => {
    if (client === "claude_desktop" && !replace) throw { code: "mcp_entry_exists" };
    current = status(client === "claude_desktop");
    return client === "claude_desktop" ? ("replaced" as const) : ("added" as const);
  });
  await openMcp({ mcpServerStatus: async () => current, installMcpClient });
  const group = await mcpGroup();

  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));
  // 不带权限的条目不需要先确认。
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  await within(group).findByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。");
  expect(installMcpClient).toHaveBeenCalledWith("cursor", false, []);

  fireEvent.click(await within(group).findByRole("radio", { name: "Claude Desktop" }));

  // Declining leaves the other entry alone.
  fireEvent.click(within(group).getByRole("button", { name: "写入 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消" }));
  await waitFor(() =>
    expect(
      (within(group).getByRole("button", { name: "写入 Claude Desktop" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  expect(installMcpClient).not.toHaveBeenCalledWith("claude_desktop", true, []);

  fireEvent.click(within(group).getByRole("button", { name: "写入 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "替换" }));
  await within(group).findByText("已写入 Claude Desktop 的配置。重新启动 Claude Desktop 后生效。");
  expect(installMcpClient).toHaveBeenLastCalledWith("claude_desktop", true, []);
  expect(group.textContent).toContain("（已连接）");
});

test("writing an entry with permissions asks first, and cancelling writes nothing", async () => {
  const installMcpClient = vi.fn(async () => "added" as const);
  await openMcp({ mcpServerStatus: async () => status(), installMcpClient });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));

  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  const dialog = await screen.findByRole("alertdialog");
  expect(dialog.textContent).toContain(
    "修改设置和快捷短语、制作、切换和删除候选窗口皮肤，用你在设置里登录的账号把皮肤发布到社区",
  );
  expect(dialog.textContent).toContain("读取你的用户词库、查看编码的候选");
  expect(dialog.textContent).toContain("增删、调整和导入词");
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  await waitFor(() =>
    expect(
      (within(group).getByRole("button", { name: "写入 Cursor" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  expect(installMcpClient).not.toHaveBeenCalled();

  // 只开一项时，确认里只说这一项。
  fireEvent.click(within(group).getByRole("switch", { name: "允许修改设置" }));
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  const narrower = await screen.findByRole("alertdialog");
  expect(narrower.textContent).toContain("读取你的用户词库");
  expect(narrower.textContent).not.toContain("快捷短语");
  expect(narrower.textContent).not.toContain("导入词");
  fireEvent.click(within(narrower).getByRole("button", { name: "允许并写入" }));
  await within(group).findByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。");
  expect(installMcpClient).toHaveBeenCalledOnce();
  expect(installMcpClient).toHaveBeenCalledWith("cursor", false, ["--allow-dictionary-read"]);
});

test("a configured client's switches show its entry, and a change offers to update it", async () => {
  let current = status(true, ["--allow-write"]);
  const installMcpClient = vi.fn(
    async (_client: string, _replace: boolean, flags: readonly McpFlag[]) => {
      current = status(true, [...flags]);
      return "updated" as const;
    },
  );
  await openMcp({ mcpServerStatus: async () => current, installMcpClient });
  const group = await mcpGroup();
  const checked = (name: string) =>
    (within(group).getByRole("switch", { name }) as HTMLInputElement).checked;
  // 其它页按记住的选择（默认两项都开）。
  expect(checked("允许读取词库")).toBe(true);

  fireEvent.click(await within(group).findByRole("radio", { name: "Claude Desktop" }));
  expect(checked("允许修改设置")).toBe(true);
  expect(checked("允许读取词库")).toBe(false);
  expect(group.textContent).toContain("（已连接）");
  const write = within(group).getByRole("button", { name: "写入 Claude Desktop" });
  expect((write as HTMLButtonElement).disabled).toBe(true);

  fireEvent.click(within(group).getByRole("switch", { name: "允许读取词库" }));
  const update = within(group).getByRole("button", { name: "更新 Claude Desktop" });
  expect((update as HTMLButtonElement).disabled).toBe(false);
  // 改回去就没有要更新的了。
  fireEvent.click(within(group).getByRole("switch", { name: "允许读取词库" }));
  expect(within(group).queryByRole("button", { name: "更新 Claude Desktop" })).toBeNull();
  fireEvent.click(within(group).getByRole("switch", { name: "允许读取词库" }));

  fireEvent.click(within(group).getByRole("button", { name: "更新 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "允许并写入" }));
  await within(group).findByText("已更新 Claude Desktop 的配置。重新启动 Claude Desktop 后生效。");
  expect(installMcpClient).toHaveBeenCalledWith("claude_desktop", false, [
    "--allow-write",
    "--allow-dictionary-read",
  ]);
  await waitFor(() =>
    expect(
      (within(group).getByRole("button", { name: "写入 Claude Desktop" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true),
  );
  expect(checked("允许读取词库")).toBe(true);
});

test("an entry an earlier Nix version wrote is offered as an update that keeps its permissions", async () => {
  let current = status(true, ["--allow-write"], true);
  const installMcpClient = vi.fn(
    async (_client: string, _replace: boolean, flags: readonly McpFlag[]) => {
      current = status(true, [...flags]);
      return "updated" as const;
    },
  );
  await openMcp({ mcpServerStatus: async () => current, installMcpClient });
  const group = await mcpGroup();
  const checked = (name: string) =>
    (within(group).getByRole("switch", { name }) as HTMLInputElement).checked;

  fireEvent.click(await within(group).findByRole("radio", { name: "Claude Desktop" }));
  expect(group.textContent).toContain("（已连接）");
  expect(group.textContent).toContain("指向 Nix store");
  // 开关是条目原来的权限，不是记住的默认值。
  expect(checked("允许修改设置")).toBe(true);
  expect(checked("允许读取词库")).toBe(false);

  fireEvent.click(within(group).getByRole("button", { name: "更新 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "允许并写入" }));
  await within(group).findByText("已更新 Claude Desktop 的配置。重新启动 Claude Desktop 后生效。");
  // 不用确认替换，权限照旧。
  expect(installMcpClient).toHaveBeenCalledOnce();
  expect(installMcpClient).toHaveBeenCalledWith("claude_desktop", false, ["--allow-write"]);
  await waitFor(() =>
    expect(
      (within(group).getByRole("button", { name: "写入 Claude Desktop" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true),
  );
  expect(group.textContent).not.toContain("指向 Nix store");
});

test("ignores a same-tick duplicate MCP install", async () => {
  readOnly();
  const pending = deferred<McpInstallOutcome>();
  const install = vi.fn().mockReturnValue(pending.promise);
  render(<McpConnectSection status={() => Promise.resolve(status())} install={install} />);
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));
  const write = within(group).getByRole("button", { name: "写入 Cursor" });
  act(() => {
    fireEvent.click(write);
    fireEvent.click(write);
  });
  expect(install).toHaveBeenCalledOnce();
  pending.resolve("added");
  await within(group).findByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。");
});

test("a write response from a replaced host cannot update the new MCP section", async () => {
  readOnly();
  const pending = deferred<McpInstallOutcome>();
  const oldInstall = vi.fn(() => pending.promise);
  const nextInstall = vi.fn().mockResolvedValue("added" as const);
  const view = render(
    <McpConnectSection status={() => Promise.resolve(status())} install={oldInstall} />,
  );
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  view.rerender(
    <McpConnectSection status={() => Promise.resolve(status())} install={nextInstall} />,
  );
  pending.resolve("added");
  await Promise.resolve();
  expect(screen.queryByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。")).toBeNull();
  expect(
    (
      within(await mcpGroup()).getByRole("button", {
        name: "写入 Cursor",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
});

test("旧宿主迟到的条目冲突不能在新 MCP 面板打开替换确认", async () => {
  readOnly();
  const pending = deferred<McpInstallOutcome>();
  const oldInstall = vi.fn(() => pending.promise);
  const nextInstall = vi.fn().mockResolvedValue("added" as const);
  const view = render(
    <McpConnectSection status={() => Promise.resolve(status())} install={oldInstall} />,
  );
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  expect(oldInstall).toHaveBeenCalledWith("cursor", false, []);

  view.rerender(
    <McpConnectSection status={() => Promise.resolve(status())} install={nextInstall} />,
  );
  await act(async () => {
    pending.reject({ code: "mcp_entry_exists" });
    await Promise.resolve();
  });
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(oldInstall).toHaveBeenCalledOnce();

  fireEvent.click(screen.getByRole("button", { name: "写入 Cursor" }));
  await screen.findByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。");
  expect(nextInstall).toHaveBeenCalledOnce();
  expect(nextInstall).toHaveBeenCalledWith("cursor", false, []);
});

test("a copy response from a replaced host cannot mark the new MCP section copied", async () => {
  readOnly();
  const pending = deferred<void>();
  const oldCopy = vi.fn(() => pending.promise);
  const nextCopy = vi.fn().mockResolvedValue(undefined);
  const view = render(
    <McpConnectSection status={() => Promise.resolve(status())} copyText={oldCopy} />,
  );
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "其他" }));
  fireEvent.click(within(group).getByRole("button", { name: "复制配置" }));
  expect(oldCopy).toHaveBeenCalledWith(config);
  view.rerender(<McpConnectSection status={() => Promise.resolve(status())} copyText={nextCopy} />);
  await act(async () => {
    pending.resolve();
    await Promise.resolve();
  });
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
});

test("a configuration file that is not JSON is reported and left alone", async () => {
  readOnly();
  const installMcpClient = vi.fn(async () => {
    throw { code: "mcp_config_invalid" };
  });
  await openMcp({ mcpServerStatus: async () => status(), installMcpClient });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "Cursor" }));
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  await within(group).findByText("Cursor 的配置文件不是有效的 JSON，已保持原样。请先修正该文件。");
});

test("a late status response cannot update an unmounted section or replace a newer refresh", async () => {
  const first = deferred<McpServerStatus>();
  const second = deferred<McpServerStatus>();
  const view = render(<McpConnectSection status={() => first.promise} />);
  view.rerender(<McpConnectSection status={() => second.promise} />);

  first.resolve(status());
  await Promise.resolve();
  expect(screen.queryByLabelText("Claude Code 安装命令")).toBeNull();

  second.resolve(status(true));
  expect(await screen.findByLabelText("Claude Code 安装命令")).not.toBeNull();
  view.unmount();

  const late = deferred<McpServerStatus>();
  const unmounted = render(<McpConnectSection status={() => late.promise} />);
  unmounted.unmount();
  late.resolve(status());
  await Promise.resolve();
});

test.each([
  ["install", "成功"],
  ["install", "失败"],
  ["copyText", "成功"],
  ["copyText", "失败"],
] as const)("仅更换 %s 回调后重新加载状态，并忽略旧请求%s", async (owner, outcome) => {
  const first = deferred<McpServerStatus>();
  const second = deferred<McpServerStatus>();
  const readStatus = vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  const install = vi.fn().mockResolvedValue("added" as const);
  const copyText = vi.fn().mockResolvedValue(undefined);
  const view = render(
    <McpConnectSection status={readStatus} install={install} copyText={copyText} />,
  );
  view.rerender(
    <McpConnectSection
      status={readStatus}
      install={owner === "install" ? vi.fn().mockResolvedValue("added" as const) : install}
      copyText={owner === "copyText" ? vi.fn().mockResolvedValue(undefined) : copyText}
    />,
  );

  await act(async () => {
    if (outcome === "成功") first.resolve(status());
    else first.reject(new Error("synthetic-old-status-detail"));
  });
  expect(screen.queryByLabelText("Claude Code 安装命令")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();

  await act(async () => {
    second.resolve({ ...status(), command: "/synthetic/new/msime-mcp" });
  });
  expect(await screen.findByLabelText("Claude Code 安装命令")).toHaveProperty(
    "textContent",
    expect.stringContaining("/synthetic/new/msime-mcp"),
  );
  expect(readStatus).toHaveBeenCalledTimes(2);
});

test("the terminal tab tells an assistant how to run the tools directly, with the chosen flags", async () => {
  readOnly();
  const copyText = vi.fn(async () => {});
  await openMcp({ mcpServerStatus: async () => status(), copyText });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "命令行" }));
  const program = "/opt/msime/msime-mcp --options /state/runtime-options.json";
  expect(within(group).getByLabelText("命令行用法").textContent).toBe(
    [
      "这是给终端或 AI 助手使用的命令。通常不需要修改路径：复制整行到终端，按回车运行。",
      "",
      `1. 先查看可用的工具和参数：${program} tools`,
      "   （不知道工具名时，先运行这一条。）",
      "",
      `2. 调用一个工具：${program} call <工具名> '<JSON 参数>'，参数中有单引号时改为写进 UTF-8 文件并传 @<文件路径>`,
      "   参数是 JSON 对象，命令会输出 JSON。",
      "",
      `3. 排查输入法问题（卡顿、候选窗口不见了）：${program} prompt diagnose`,
      "",
      "命令中的权限参数由下方开关决定；修改开关后，请重新复制命令。如果要配置 Claude Code 或 Codex，请切换到对应页签，使用那里的一键安装命令。",
    ].join("\n"),
  );
  fireEvent.click(within(group).getByRole("switch", { name: "允许修改设置" }));
  const usage = within(group).getByLabelText("命令行用法").textContent!;
  expect(usage).toContain(`${program} --allow-write tools`);
  fireEvent.click(within(group).getByRole("button", { name: "复制说明" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith(usage));
});

test("on Windows the terminal instructions pass the arguments through a file", async () => {
  readOnly();
  await openMcp({
    mcpServerStatus: async () => ({
      ...status(),
      command: "C:\\Program Files\\MSIME\\msime-mcp.exe",
      options: "C:\\ProgramData\\MSIME\\runtime-options.json",
    }),
    copyText: vi.fn(async () => {}),
  });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "命令行" }));
  const usage = within(group).getByLabelText("命令行用法").textContent!;
  expect(usage).toContain(
    '"C:\\Program Files\\MSIME\\msime-mcp.exe" --options "C:\\ProgramData\\MSIME\\runtime-options.json" call <工具名> @<文件路径>',
  );
  expect(usage).not.toContain("'<JSON 参数>'");
});

/** The text of each coloured piece in a shown block, in order, by the utility that colours it. */
function coloured(block: HTMLElement, utility: string): string[] {
  return [...block.querySelectorAll(`.${utility}`)].map((span) => span.textContent ?? "");
}

test("commands are coloured by program, flag, quoted argument and placeholder", async () => {
  readOnly();
  await openMcp({
    mcpServerStatus: async () => ({ ...status(), options: "/Library/Application Support/o.json" }),
  });
  const group = await mcpGroup();
  const install = within(group).getByLabelText("Claude Code 安装命令");
  expect(coloured(install, "text-syntax-program")).toEqual(["claude", "/opt/msime/msime-mcp"]);
  expect(coloured(install, "text-syntax-flag")).toEqual(["--scope", "--", "--options"]);
  expect(coloured(install, "text-syntax-string")).toEqual([
    "'/Library/Application Support/o.json'",
  ]);

  fireEvent.click(await within(group).findByRole("radio", { name: "命令行" }));
  const usage = within(group).getByLabelText("命令行用法");
  expect(coloured(usage, "text-syntax-placeholder")).toEqual(["<工具名>", "@<文件路径>"]);
  expect(coloured(usage, "text-syntax-string")).toContain("'<JSON 参数>'");
  // The prose around the commands stays uncoloured.
  expect(coloured(usage, "text-syntax-program").join("")).not.toContain("水杉");
});

test("the configuration is coloured when it is laid out as the host's pretty printer writes it", async () => {
  readOnly();
  const pretty = JSON.stringify(JSON.parse(config), null, 2);
  const copyText = vi.fn(async () => {});
  await openMcp({ mcpServerStatus: async () => ({ ...status(), config: pretty }), copyText });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "其他" }));
  const block = within(group).getByLabelText("MCP 配置");
  expect(block.textContent).toBe(pretty);
  expect(coloured(block, "text-syntax-key")).toEqual([
    '"mcpServers"',
    '"msime"',
    '"command"',
    '"args"',
  ]);
  expect(coloured(block, "text-syntax-string")).toEqual([
    '"/opt/msime/msime-mcp"',
    '"--options"',
    '"/state/runtime-options.json"',
  ]);
  fireEvent.click(within(group).getByRole("button", { name: "复制配置" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith(pretty));
});

test("a configuration laid out differently is shown as written, uncoloured", async () => {
  readOnly();
  await openMcp({ mcpServerStatus: async () => status() });
  const group = await mcpGroup();
  fireEvent.click(await within(group).findByRole("radio", { name: "其他" }));
  const block = within(group).getByLabelText("MCP 配置");
  expect(block.textContent).toBe(config);
  expect(block.querySelector("span")).toBeNull();
});
