import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import * as settings from "./settings-style";
import { GroupList, Segmented, Switch } from "../core/platform-controls";
import { mcpFailureMessage } from "./mcp-errors";
import { jsonTokens, plain, SyntaxBlock, type SyntaxToken, tokensText } from "./mcp-syntax";
import { ActionButton } from "./action-button";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";

/** The assistants the host can write the entry for. */
export type McpClientId = "claude_desktop" | "cursor";

/** 放宽助手权限的 `msime-mcp` 参数。 */
export type McpFlag = "--allow-write" | "--allow-dictionary-read";

export type McpClientStatus = {
  id: McpClientId;
  /** The assistant's configuration file. */
  path: string;
  /** 文件里的 `msime` 条目就是这里的服务器，可能带了 `flags` 里的权限参数。 */
  configured: boolean;
  /** 已写入条目带的权限参数，按固定顺序；未连接时为空。 */
  flags: McpFlag[];
};

export type McpServerStatus = {
  /** The absolute path of `msime-mcp` beside the settings app. */
  command: string;
  /** Whether that file exists. */
  installed: boolean;
  /** The runtime options the entry points at; absent before the input method is set up. */
  options: string | null;
  /** `{"mcpServers": {"msime": ...}}` to paste into any assistant. */
  config: string | null;
  clients: McpClientStatus[];
};

export type McpInstallOutcome = "added" | "updated" | "replaced" | "unchanged";

const clientNames: Record<McpClientId, string> = {
  claude_desktop: "Claude Desktop",
  cursor: "Cursor",
};

const code =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed";
const command =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed whitespace-pre-wrap break-all";

/** 放宽助手权限的两个开关，按写进 `args` 的固定顺序排列。`msime-mcp` 不带参数时两项都是关的；设置页默认替用户打开。 */
const permissionFlags = [
  {
    flag: "--allow-write",
    title: "允许修改设置",
    detail: "修改快捷短语和设置、制作候选窗口皮肤",
  },
  {
    flag: "--allow-dictionary-read",
    title: "允许读取词库",
    detail: "读取用户词库、查看编码的候选；与上一项同时开启才能增删、调整和导入词",
  },
] as const;

const allFlags: McpFlag[] = permissionFlags.map((permission) => permission.flag);

/** 用户上次选的开关，跨次打开设置页保留。 */
const flagsStorageKey = "msime.mcp.flags";

function savedFlags(): McpFlag[] {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(flagsStorageKey) ?? "null");
    if (Array.isArray(value)) return allFlags.filter((flag) => value.includes(flag));
  } catch {
    /* 受限的 webview 可能拒绝访问存储，这时按默认值 */
  }
  return allFlags;
}

function saveFlags(flags: readonly McpFlag[]) {
  try {
    window.localStorage.setItem(flagsStorageKey, JSON.stringify(flags));
  } catch {
    /* 记不住只影响下次打开时的默认值 */
  }
}

function sameFlags(a: readonly McpFlag[], b: readonly McpFlag[]): boolean {
  return a.length === b.length && a.every((flag) => b.includes(flag));
}

/** 一键写入带权限的条目前，用大白话说清楚助手将能做什么。 */
function grantMessage(name: string, flags: readonly McpFlag[]): string {
  const write = flags.includes("--allow-write");
  const read = flags.includes("--allow-dictionary-read");
  const abilities = [
    write ? "修改设置和快捷短语、制作候选窗口皮肤" : undefined,
    read ? "读取你的用户词库、查看编码的候选" : undefined,
    write && read ? "增删、调整和导入词" : undefined,
  ].filter((ability) => ability !== undefined);
  return `写入后，${name} 里的 AI 助手可以${abilities.join("；")}。只在你信任它时允许；之后关掉开关再更新即可收回。`;
}

/** The tabs: two command-line assistants, running the tools straight from a terminal, the clients the host writes into, and the JSON for anything else. */
type McpTab = "claude_code" | "codex" | "terminal" | McpClientId | "json";

/** One argument quoted for the shell the command is pasted into: double quotes for a Windows path, single quotes for a POSIX shell, and nothing when the argument needs none. */
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

/** `msime-mcp` with the runtime options and the chosen flags, quoted for the shell. */
function serverProgram(server: McpServerStatus, flags: readonly McpFlag[]): SyntaxToken[] {
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

function installCommand(
  assistant: "claude_code" | "codex",
  server: McpServerStatus,
  flags: readonly McpFlag[],
): SyntaxToken[] {
  const head =
    assistant === "claude_code"
      ? [program("claude"), plain(" mcp add "), flag("--scope"), plain(" user msime ")]
      : [program("codex"), plain(" mcp add msime ")];
  return [...head, flag("--"), plain(" "), ...serverProgram(server, flags)];
}

/** What to tell an assistant that works in a terminal, such as in its AGENTS.md or CLAUDE.md: the same tools without registering a server, one command per tool. */
function terminalInstructions(server: McpServerStatus, flags: readonly McpFlag[]): SyntaxToken[] {
  const msime = serverProgram(server, flags);
  // Neither cmd nor Windows PowerShell passes a quoted JSON argument intact, so on Windows the arguments go through a file.
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
    plain("水杉输入法（MSIME）可以在终端里直接管理：\n- 查看可用的工具和参数："),
    ...msime,
    plain(" tools\n- 调用一个工具，参数是 JSON 对象，输出 JSON："),
    ...call,
    plain("\n- 排查输入法问题（卡顿、候选窗口不见了）的步骤："),
    ...msime,
    plain(" prompt diagnose"),
  ];
}

/** Removes an earlier registration: both assistants refuse to add a name that is already there, so changing the permissions means removing it first. */
function removeCommand(assistant: "claude_code" | "codex"): SyntaxToken[] {
  return assistant === "claude_code"
    ? [program("claude"), plain(" mcp remove "), flag("--scope"), plain(" user msime")]
    : [program("codex"), plain(" mcp remove msime")];
}

/** The host's JSON entry with the chosen flags added to `args`; the entry as the host wrote it, uncoloured, when no flag is chosen and it is not laid out as `JSON.stringify` would, or when it is not the expected shape. */
function configWithFlags(config: string, flags: readonly McpFlag[]): SyntaxToken[] {
  try {
    const parsed = JSON.parse(config) as { mcpServers?: { msime?: { args?: unknown } } };
    if (flags.length === 0) {
      // The host writes it with serde_json's pretty printer, which lays it out exactly as JSON.stringify does; anything else is shown as written rather than reformatted.
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

/**
 * 「连接 AI 助手」: the `msime-mcp` entry an assistant runs, to copy or to write into Claude Desktop's or Cursor's configuration, or the commands a terminal assistant runs the tools with directly.
 *
 * The callbacks are passed in rather than read from the settings client so that the page decides, at the call site in `index.tsx`, which of them a host offers; that is where the settings action guard looks.
 */
export function McpConnectSection({
  status,
  install,
  copyText,
}: {
  status: () => Promise<McpServerStatus>;
  install?: (
    client: McpClientId,
    replace: boolean,
    flags: readonly McpFlag[],
  ) => Promise<McpInstallOutcome>;
  copyText?: (text: string) => Promise<void>;
}) {
  const { confirm, confirmation } = useConfirm();
  const [server, setServer] = useState<McpServerStatus>();
  const [loadFailed, setLoadFailed] = useState(false);
  const [busy, setBusy] = useState<McpClientId>();
  const [result, setResult] = useState<string>();
  const [copied, setCopied] = useState<string>();
  const [tab, setTab] = useState<McpTab>("claude_code");
  // 用户想要的权限，复制的命令和配置、未连接的助手都按它来。
  const [preferred, setPreferred] = useState<McpFlag[]>(savedFlags);
  // 已连接的助手页上，开关先显示它现有条目的权限；用户改过之后记在这里，直到写入。
  const [drafts, setDrafts] = useState<Partial<Record<McpClientId, McpFlag[]>>>({});
  const mounted = useRef(true);
  const refreshGeneration = useRef(0);
  const clientGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    actionRunning.current = false;
    setBusy(undefined);
    return () => {
      mounted.current = false;
      refreshGeneration.current += 1;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [status, install, copyText]);

  const refresh = useCallback(() => {
    const generation = ++refreshGeneration.current;
    return status().then(
      (next) => {
        if (!mounted.current || refreshGeneration.current !== generation) return;
        setServer(next);
        setLoadFailed(false);
      },
      () => {
        if (!mounted.current || refreshGeneration.current !== generation) return;
        setLoadFailed(true);
      },
    );
  }, [status]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function write(id: McpClientId, flags: readonly McpFlag[]) {
    if (!install || busy !== undefined || actionRunning.current) return;
    const generation = clientGeneration.current;
    const name = clientNames[id];
    actionRunning.current = true;
    setBusy(id);
    setResult(undefined);
    try {
      if (flags.length > 0) {
        const granted = await confirm({
          title: `允许 ${name} 中的助手使用这些权限？`,
          message: grantMessage(name, flags),
          confirmLabel: "允许并写入",
        });
        if (!granted) return;
        if (!mounted.current || generation !== clientGeneration.current) return;
      }
      let outcome: McpInstallOutcome;
      try {
        outcome = await install(id, false, flags);
      } catch (error) {
        if (errorCode(error) !== "mcp_entry_exists") throw error;
        const replace = await confirm({
          title: `替换 ${name} 中的 msime？`,
          message: `${name} 的配置里已有另一个名为 msime 的服务器。替换后，它原来的命令和参数（包括手动加上的 --allow-write）会被这里的设置覆盖。`,
          confirmLabel: "替换",
        });
        if (!replace) return;
        if (!mounted.current || generation !== clientGeneration.current) return;
        outcome = await install(id, true, flags);
      }
      if (mounted.current && generation === clientGeneration.current) {
        setDrafts((current) => {
          const next = { ...current };
          delete next[id];
          return next;
        });
        setResult(
          outcome === "unchanged"
            ? `${name} 已经连接，无需改动。`
            : outcome === "updated"
              ? `已更新 ${name} 的配置。重新启动 ${name} 后生效。`
              : `已写入 ${name} 的配置。重新启动 ${name} 后生效。`,
        );
      }
      if (!mounted.current || generation !== clientGeneration.current) return;
      await refresh();
    } catch (error) {
      if (mounted.current && generation === clientGeneration.current)
        setResult(mcpFailureMessage(error, name));
    } finally {
      if (mounted.current && generation === clientGeneration.current) {
        actionRunning.current = false;
        setBusy(undefined);
      }
    }
  }

  function copy(key: string, text: string) {
    if (!copyText) return;
    const generation = clientGeneration.current;
    void copyText(text).then(() => {
      if (!mounted.current || generation !== clientGeneration.current) return;
      setCopied(key);
      window.setTimeout(() => {
        if (mounted.current && generation === clientGeneration.current)
          setCopied((current) => (current === key ? undefined : current));
      }, 1600);
    });
  }

  const writableClients = install && server?.installed ? server.clients : [];
  const tabs: { value: McpTab; label: string }[] = [
    { value: "claude_code", label: "Claude Code" },
    { value: "codex", label: "Codex" },
    { value: "terminal", label: "命令行" },
    ...writableClients.map((client) => ({ value: client.id, label: clientNames[client.id] })),
    { value: "json", label: "其他" },
  ];
  const shownTab = tabs.some((option) => option.value === tab) ? tab : "claude_code";
  const client = writableClients.find((candidate) => candidate.id === shownTab);
  // 已连接的助手页显示它真实的权限（或用户刚改的），其它页显示用户想要的权限。
  const flags = client?.configured ? (drafts[client.id] ?? client.flags) : preferred;
  const outdated = client?.configured === true && !sameFlags(flags, client.flags);

  function setFlag(flag: McpFlag, on: boolean) {
    const toggled = (current: readonly McpFlag[]) =>
      allFlags.filter((candidate) => (candidate === flag ? on : current.includes(candidate)));
    if (client?.configured) setDrafts((current) => ({ ...current, [client.id]: toggled(flags) }));
    const next = toggled(preferred);
    setPreferred(next);
    saveFlags(next);
  }

  function copyButton(key: string, label: string, tokens: readonly SyntaxToken[]) {
    if (!copyText) return null;
    const text = tokensText(tokens);
    return (
      <SettingsManagerActions>
        <ActionButton action={() => copy(key, text)} label={copied === key ? "已复制" : label} />
      </SettingsManagerActions>
    );
  }

  return (
    <GroupList title="连接 AI 助手">
      <SettingsManagerBlock role="group" aria-label="连接 AI 助手">
        <SettingsManagerNote>
          连接后，把输入法的问题（卡顿、候选窗口不见了）直接告诉 AI
          助手：它会打开诊断日志、请你重做一遍出问题的操作，再读日志找原因；也能读取快捷短语、设置、打字统计和已安装的候选窗口皮肤。通过
          MCP
          在本机运行，不联网。助手还能做什么由下面两个开关决定，默认都开；复制的命令、配置和一键写入都带上开着的权限。两个都关时只读，除了开关诊断日志不改动任何设置。
        </SettingsManagerNote>
        {loadFailed && <p role="alert">无法读取 MCP 服务器的状态。</p>}
        {server &&
          (server.config ? (
            <>
              <Segmented
                aria-label="AI 助手"
                options={tabs}
                value={shownTab}
                onChange={(next) => {
                  setTab(next);
                  setResult(undefined);
                }}
              />
              {(shownTab === "claude_code" || shownTab === "codex") && (
                <>
                  <SettingsManagerNote>
                    在终端运行下面的命令，然后重新启动{" "}
                    {shownTab === "claude_code" ? "Claude Code" : "Codex"}：
                  </SettingsManagerNote>
                  <SyntaxBlock
                    className={command}
                    aria-label={
                      shownTab === "claude_code" ? "Claude Code 安装命令" : "Codex 安装命令"
                    }
                    tokens={installCommand(shownTab, server, flags)}
                  />
                  {copyButton(shownTab, "复制命令", installCommand(shownTab, server, flags))}
                  <SettingsManagerNote>之前添加过的，先运行这条：</SettingsManagerNote>
                  <SyntaxBlock
                    className={command}
                    aria-label={
                      shownTab === "claude_code" ? "Claude Code 移除命令" : "Codex 移除命令"
                    }
                    tokens={removeCommand(shownTab)}
                  />
                  {copyButton(`${shownTab}-remove`, "复制移除命令", removeCommand(shownTab))}
                </>
              )}
              {shownTab === "terminal" && (
                <>
                  <SettingsManagerNote>
                    不注册 MCP 也可以：能在终端里运行命令的助手（Claude Code、Codex
                    等）直接调用同一组工具，权限开关相同，不用重启助手。把下面这段话告诉助手，或放进项目的
                    AGENTS.md / CLAUDE.md：
                  </SettingsManagerNote>
                  <SyntaxBlock
                    className={command}
                    aria-label="命令行用法"
                    tokens={terminalInstructions(server, flags)}
                  />
                  {copyButton("terminal", "复制说明", terminalInstructions(server, flags))}
                </>
              )}
              {client && (
                <>
                  <SettingsManagerNote>
                    写入 <code>{client.path}</code>
                    {client.configured ? "（已连接）" : ""}，重新启动 {clientNames[client.id]}{" "}
                    后生效。
                  </SettingsManagerNote>
                  {outdated && (
                    <p className="notice">
                      下面的权限和 {clientNames[client.id]} 现在的配置不同，点「更新{" "}
                      {clientNames[client.id]}」写入。
                    </p>
                  )}
                  <SettingsManagerActions>
                    <ActionButton
                      action={() => void write(client.id, flags)}
                      disabled={busy !== undefined || (client.configured && !outdated)}
                      ariaBusy={busy === client.id}
                      label={
                        busy === client.id
                          ? "正在写入…"
                          : `${outdated ? "更新" : "写入"} ${clientNames[client.id]}`
                      }
                    />
                  </SettingsManagerActions>
                </>
              )}
              {shownTab === "json" && (
                <>
                  <SettingsManagerNote>粘贴到任意支持 MCP 的助手的配置中：</SettingsManagerNote>
                  <SyntaxBlock
                    className={code}
                    aria-label="MCP 配置"
                    tokens={configWithFlags(server.config, flags)}
                  />
                  {copyButton("json", "复制配置", configWithFlags(server.config, flags))}
                </>
              )}
              {result && <p role="status">{result}</p>}
              {permissionFlags.map((permission) => (
                <div className={settings.serviceRow} key={permission.flag}>
                  <span>
                    {permission.title}
                    <small>
                      <code>{permission.flag}</code> {permission.detail}
                    </small>
                  </span>
                  <Switch
                    aria-label={permission.title}
                    checked={flags.includes(permission.flag)}
                    onChange={(on) => setFlag(permission.flag, on)}
                  />
                </div>
              ))}
              <SettingsManagerNote>
                只在你信任该助手时保留这两项权限，用不上就关掉。
              </SettingsManagerNote>
              <SettingsManagerNote>
                服务器程序 <code>{server.command}</code>
                {server.installed ? "" : "（未找到，请重新安装输入法）"}
              </SettingsManagerNote>
            </>
          ) : (
            <p className="notice">输入法尚未完成初始化，完成设置向导后即可连接。</p>
          ))}
        {confirmation}
      </SettingsManagerBlock>
    </GroupList>
  );
}
