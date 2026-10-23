import { expect, test } from "vitest";
import {
  mcpConfigWithFlags,
  mcpGrantMessage,
  mcpInstallCommand,
  mcpRemoveCommand,
  mcpServerProgram,
  mcpTerminalInstructions,
  type McpServerStatus,
} from "../../../../packages/ui/src/settings/mcp-command-model";
import { tokensText } from "../../../../packages/ui/src/settings/mcp-syntax";

const server: McpServerStatus = {
  command: "/opt/synthetic/msime-mcp",
  installed: true,
  options: "/state/synthetic-options.json",
  config: `{
  "mcpServers": {
    "msime": {
      "command": "/opt/synthetic/msime-mcp",
      "args": ["--options", "/state/synthetic-options.json"]
    }
  }
}`,
  clients: [],
};

test("命令模型保留运行参数和权限顺序", () => {
  const flags = ["--allow-write", "--allow-dictionary-read"] as const;
  expect(tokensText(mcpServerProgram(server, flags))).toBe(
    "/opt/synthetic/msime-mcp --options /state/synthetic-options.json --allow-write --allow-dictionary-read",
  );
  expect(tokensText(mcpInstallCommand("claude_code", server, flags))).toBe(
    "claude mcp add --scope user msime -- /opt/synthetic/msime-mcp --options /state/synthetic-options.json --allow-write --allow-dictionary-read",
  );
  expect(tokensText(mcpRemoveCommand("codex"))).toBe("codex mcp remove msime");
});

test("命令行说明和权限提示使用同一组参数", () => {
  const flags = ["--allow-dictionary-read"] as const;
  const instructions = tokensText(mcpTerminalInstructions(server, flags));
  expect(instructions).toContain("--allow-dictionary-read tools");
  expect(instructions).toContain("prompt diagnose");
  expect(mcpGrantMessage("合成助手", flags)).toContain("读取你的用户词库");
  expect(mcpGrantMessage("合成助手", flags)).not.toContain("修改设置和快捷短语");
});

test("JSON 配置只在形状有效时追加权限，空权限保留原文格式", () => {
  const flags = ["--allow-write"] as const;
  expect(tokensText(mcpConfigWithFlags(server.config!, flags))).toContain('"--allow-write"');
  expect(tokensText(mcpConfigWithFlags('{"other":true}', flags))).toBe('{"other":true}');
  expect(tokensText(mcpConfigWithFlags("{malformed", flags))).toBe("{malformed");
  expect(tokensText(mcpConfigWithFlags('{"mcpServers":{"msime":{"args":"bad"}}}', flags))).toBe(
    '{"mcpServers":{"msime":{"args":"bad"}}}',
  );
  expect(tokensText(mcpConfigWithFlags(server.config!, []))).toBe(server.config);
});

test("Windows 路径使用双引号并转义引号", () => {
  const windows: McpServerStatus = {
    ...server,
    command: "C:\\Program Files\\MSIME\\mcp.exe",
    options: 'C:\\Users\\synthetic\\state "one".json',
  };
  expect(tokensText(mcpServerProgram(windows, []))).toBe(
    '"C:\\Program Files\\MSIME\\mcp.exe" --options "C:\\Users\\synthetic\\state \\\"one\\\".json"',
  );
});

test("POSIX 路径中的单引号使用 shell 安全转义", () => {
  const posix: McpServerStatus = {
    ...server,
    command: "/opt/synthetic/MSIME's-mcp",
    options: "/state/user's-options.json",
  };
  expect(tokensText(mcpServerProgram(posix, ["--allow-write"]))).toBe(
    "'/opt/synthetic/MSIME'\\''s-mcp' --options '/state/user'\\''s-options.json' --allow-write",
  );
});
