// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  McpConnectSection,
  type McpInstallOutcome,
  SettingsPage,
  type McpServerStatus,
  type SettingsClient,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

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

function status(configured = false): McpServerStatus {
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
      },
      { id: "cursor", path: "/home/someone/.cursor/mcp.json", configured: false },
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

async function openDeveloper(extra: Partial<SettingsClient>) {
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "macos" } as never,
        ...extra,
      }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  fireEvent.click(screen.getByRole("button", { name: "开发者选项" }));
}

test("a host without the server shows no section", async () => {
  await openDeveloper({});
  expect(screen.queryByRole("group", { name: "连接 AI 助手" })).toBeNull();
});

test("the entry is shown and copied as the host reports it", async () => {
  const copyText = vi.fn(async () => {});
  await openDeveloper({ mcpServerStatus: async () => status(), copyText });
  const group = await screen.findByRole("group", { name: "连接 AI 助手" });
  expect(within(group).getByLabelText("MCP 配置").textContent).toBe(config);
  expect(group.textContent).toContain("--allow-write");
  // Without an install callback there is nothing to write into.
  expect(within(group).queryByRole("button", { name: "写入 Cursor" })).toBeNull();
  fireEvent.click(within(group).getByRole("button", { name: "复制配置" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith(config));
});

test("writing adds the entry and a different one is replaced only after confirming", async () => {
  let current = status();
  const installMcpClient = vi.fn(async (client: string, replace: boolean) => {
    if (client === "claude_desktop" && !replace) throw { code: "mcp_entry_exists" };
    current = status(client === "claude_desktop");
    return client === "claude_desktop" ? ("replaced" as const) : ("added" as const);
  });
  await openDeveloper({ mcpServerStatus: async () => current, installMcpClient });
  const group = await screen.findByRole("group", { name: "连接 AI 助手" });

  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  await within(group).findByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。");
  expect(installMcpClient).toHaveBeenCalledWith("cursor", false);

  // Declining leaves the other entry alone.
  fireEvent.click(within(group).getByRole("button", { name: "写入 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消" }));
  await waitFor(() =>
    expect(
      (within(group).getByRole("button", { name: "写入 Claude Desktop" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  expect(installMcpClient).not.toHaveBeenCalledWith("claude_desktop", true);

  fireEvent.click(within(group).getByRole("button", { name: "写入 Claude Desktop" }));
  fireEvent.click(await screen.findByRole("button", { name: "替换" }));
  await within(group).findByText("已写入 Claude Desktop 的配置。重新启动 Claude Desktop 后生效。");
  expect(installMcpClient).toHaveBeenLastCalledWith("claude_desktop", true);
  expect(group.textContent).toContain("（已连接）");
});

test("a write response from a replaced host cannot update the new MCP section", async () => {
  const pending = deferred<McpInstallOutcome>();
  const oldInstall = vi.fn(() => pending.promise);
  const nextInstall = vi.fn().mockResolvedValue("added" as const);
  const view = render(
    <McpConnectSection status={() => Promise.resolve(status())} install={oldInstall} />,
  );
  const group = await screen.findByRole("group", { name: "连接 AI 助手" });
  fireEvent.click(within(group).getByRole("button", { name: "写入 Cursor" }));
  view.rerender(
    <McpConnectSection status={() => Promise.resolve(status())} install={nextInstall} />,
  );
  pending.resolve("added");
  await Promise.resolve();
  expect(screen.queryByText("已写入 Cursor 的配置。重新启动 Cursor 后生效。")).toBeNull();
  expect(
    (
      within(await screen.findByRole("group", { name: "连接 AI 助手" })).getByRole("button", {
        name: "写入 Cursor",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
});

test("a configuration file that is not JSON is reported and left alone", async () => {
  const installMcpClient = vi.fn(async () => {
    throw { code: "mcp_config_invalid" };
  });
  await openDeveloper({ mcpServerStatus: async () => status(), installMcpClient });
  const group = await screen.findByRole("group", { name: "连接 AI 助手" });
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
  expect(screen.queryByLabelText("MCP 配置")).toBeNull();

  second.resolve(status(true));
  expect(await screen.findByLabelText("MCP 配置")).not.toBeNull();
  view.unmount();

  const late = deferred<McpServerStatus>();
  const unmounted = render(<McpConnectSection status={() => late.promise} />);
  unmounted.unmount();
  late.resolve(status());
  await Promise.resolve();
});
