import { jsonTokens, plain, type SyntaxToken } from "./mcp-syntax";

/** MCP 客户端配置与命令模型；不依赖 React 或设置页状态。 */
export type McpClientId = "claude_desktop" | "cursor";

export type McpFlag = "--allow-write" | "--allow-dictionary-read";

export type McpClientStatus = {
  id: McpClientId;
  path: string;
  configured: boolean;
  flags: McpFlag[];
  stale: boolean;
};

export type McpServerStatus = {
  command: string;
  installed: boolean;
  options: string | null;
  config: string | null;
  clients: McpClientStatus[];
};

export type McpInstallOutcome = "added" | "updated" | "replaced" | "unchanged";

function shellQuote(value: string, windows: boolean): string {
  if (/^[\w@%+=:,./-]+$/.test(value)) return value;
  if (windows) return `"${value.replace(/"/g, '\\"')}"`;
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function isWindowsPath(path: string): boolean {
  return /^[A-Za-z]:\\/.test(path);
}

const program = (text: string): SyntaxToken => ({ text, kind: "program" });
const flag = (text: string): SyntaxToken => ({ text, kind: "flag" });
const placeholder = (text: string): SyntaxToken => ({ text, kind: "placeholder" });

/** 返回带运行选项和权限参数的 `msime-mcp` 命令。 */
export function mcpServerProgram(
  server: McpServerStatus,
  flags: readonly McpFlag[],
): SyntaxToken[] {
  const windows = isWindowsPath(server.command);
  const options: SyntaxToken[] = server.options
    ? [flag("--options"), plain(" "), { text: shellQuote(server.options, windows), kind: "string" }]
    : [];
  return [
    program(shellQuote(server.command, windows)),
    ...[options, ...flags.map((name) => [flag(name)])]
      .filter((part) => part.length > 0)
      .flatMap((part) => [plain(" "), ...part]),
  ];
}

export function mcpInstallCommand(
  assistant: "claude_code" | "codex",
  server: McpServerStatus,
  flags: readonly McpFlag[],
): SyntaxToken[] {
  const head =
    assistant === "claude_code"
      ? [program("claude"), plain(" mcp add "), flag("--scope"), plain(" user msime ")]
      : [program("codex"), plain(" mcp add msime ")];
  return [...head, flag("--"), plain(" "), ...mcpServerProgram(server, flags)];
}

export function mcpTerminalInstructions(
  server: McpServerStatus,
  flags: readonly McpFlag[],
): SyntaxToken[] {
  const msime = mcpServerProgram(server, flags);
  const call = isWindowsPath(server.command)
    ? [
        plain("把 JSON 参数以 UTF-8 写进一个文件，再运行 "),
        ...msime,
        plain(" call "),
        placeholder("<工具名>"),
        plain(" "),
        placeholder("@<文件路径>"),
      ]
    : [
        ...msime,
        plain(" call "),
        placeholder("<工具名>"),
        plain(" "),
        { text: "'<JSON 参数>'", kind: "string" } as const,
        plain("，参数中有单引号时改为写进 UTF-8 文件并传 "),
        placeholder("@<文件路径>"),
      ];
  return [
    plain(
      "这是给终端或 AI 助手使用的命令。通常不需要修改路径：复制整行到终端，按回车运行。\n\n" +
        "1. 先查看可用的工具和参数：",
    ),
    ...msime,
    plain(" tools\n   （不知道工具名时，先运行这一条。）\n\n2. 调用一个工具："),
    ...call,
    plain(
      "\n   参数是 JSON 对象，命令会输出 JSON。\n\n3. 排查输入法问题（卡顿、候选窗口不见了）：",
    ),
    ...msime,
    plain(
      " prompt diagnose\n\n命令中的权限参数由下方开关决定；修改开关后，请重新复制命令。" +
        "如果要配置 Claude Code 或 Codex，请切换到对应页签，使用那里的一键安装命令。",
    ),
  ];
}

export function mcpRemoveCommand(assistant: "claude_code" | "codex"): SyntaxToken[] {
  return assistant === "claude_code"
    ? [program("claude"), plain(" mcp remove "), flag("--scope"), plain(" user msime")]
    : [program("codex"), plain(" mcp remove msime")];
}

/** 生成授权确认文案，避免展示层重复组合权限说明。 */
export function mcpGrantMessage(name: string, flags: readonly McpFlag[]): string {
  const write = flags.includes("--allow-write");
  const read = flags.includes("--allow-dictionary-read");
  const abilities = [
    write
      ? "修改设置和快捷短语、制作、切换和删除候选窗口皮肤，用你在设置里登录的账号把皮肤发布到社区"
      : undefined,
    read ? "读取你的用户词库、查看编码的候选" : undefined,
    write && read ? "增删、调整和导入词" : undefined,
  ].filter((ability) => ability !== undefined);
  return `写入后，${name} 里的 AI 助手可以${abilities.join("；")}。只在你信任它时允许；之后关掉开关再更新即可收回。`;
}

/** 按宿主已有格式渲染配置；结构不符合 MCP 条目时保留原文。 */
export function mcpConfigWithFlags(config: string, flags: readonly McpFlag[]): SyntaxToken[] {
  try {
    const parsed = JSON.parse(config) as { mcpServers?: { msime?: { args?: unknown } } };
    if (flags.length === 0) {
      return JSON.stringify(parsed, null, 2) === config ? jsonTokens(parsed) : [plain(config)];
    }
    const entry = parsed.mcpServers?.msime;
    if (!entry || !Array.isArray(entry.args)) return [plain(config)];
    entry.args = [...entry.args, ...flags];
    return jsonTokens(parsed);
  } catch {
    return [plain(config)];
  }
}
