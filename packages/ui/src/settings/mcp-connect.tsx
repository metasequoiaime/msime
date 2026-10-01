import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import * as settings from "./settings-style";
import { GroupList, Segmented, Switch } from "../core/platform-controls";
import { mcpFailureMessage } from "./mcp-errors";

/** The assistants the host can write the entry for. */
export type McpClientId = "claude_desktop" | "cursor";

export type McpClientStatus = {
  id: McpClientId;
  /** The assistant's configuration file. */
  path: string;
  /** The file already holds exactly this entry. */
  configured: boolean;
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

export type McpInstallOutcome = "added" | "replaced" | "unchanged";

const clientNames: Record<McpClientId, string> = {
  claude_desktop: "Claude Desktop",
  cursor: "Cursor",
};

const code =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed";
const command =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed whitespace-pre-wrap break-all";

/** The flags that widen what the assistant may do; both are off in the host's own entry. */
const permissionFlags = [
  {
    flag: "--allow-write",
    title: "允许修改设置",
    detail: "修改快捷短语和设置、制作候选框皮肤",
  },
  {
    flag: "--allow-dictionary-read",
    title: "允许读取词库",
    detail: "读取用户词库、查看编码的候选；与上一项同时开启才能增删、调整和导入词",
  },
] as const;

type McpFlag = (typeof permissionFlags)[number]["flag"];

/** The tabs: two command-line assistants, running the tools straight from a terminal, the clients the host writes into, and the JSON for anything else. */
type McpTab = "claude_code" | "codex" | "terminal" | McpClientId | "json";

/** One argument quoted for the shell the command is pasted into: double quotes for a Windows path, single quotes for a POSIX shell, and nothing when the argument needs none. */
function shellQuote(value: string, windows: boolean): string {
  if (/^[\w@%+=:,./-]+$/.test(value)) return value;
  if (windows) return `"${value.replace(/"/g, '\\"')}"`;
  return `'${value.replace(/'/g, "'\\''")}'`;
}

/** The server's argument list: the runtime options, then the chosen flags. */
function serverArgs(server: McpServerStatus, flags: readonly McpFlag[]): string[] {
  return [...(server.options ? ["--options", server.options] : []), ...flags];
}

/** `msime-mcp` with the runtime options and the chosen flags, quoted for the shell. */
function serverProgram(server: McpServerStatus, flags: readonly McpFlag[]): string {
  const windows = /^[A-Za-z]:\\/.test(server.command);
  return [server.command, ...serverArgs(server, flags)]
    .map((part) => shellQuote(part, windows))
    .join(" ");
}

function installCommand(
  assistant: "claude_code" | "codex",
  server: McpServerStatus,
  flags: readonly McpFlag[],
): string {
  const program = serverProgram(server, flags);
  return assistant === "claude_code"
    ? `claude mcp add --scope user msime -- ${program}`
    : `codex mcp add msime -- ${program}`;
}

/** What to tell an assistant that works in a terminal, such as in its AGENTS.md or CLAUDE.md: the same tools without registering a server, one command per tool. */
function terminalInstructions(server: McpServerStatus, flags: readonly McpFlag[]): string {
  const program = serverProgram(server, flags);
  return [
    "水杉输入法（MSIME）可以在终端里直接管理：",
    `- 查看可用的工具和参数：${program} tools`,
    `- 调用一个工具，参数是 JSON 对象，输出 JSON：${program} call <工具名> '<JSON 参数>'`,
    `- 排查输入法问题（卡顿、候选框不见了）的步骤：${program} prompt diagnose`,
  ].join("\n");
}

/** Removes an earlier registration: both assistants refuse to add a name that is already there, so changing the permissions means removing it first. */
function removeCommand(assistant: "claude_code" | "codex"): string {
  return assistant === "claude_code"
    ? "claude mcp remove --scope user msime"
    : "codex mcp remove msime";
}

/** The host's JSON entry with the chosen flags added to `args`; the entry as the host wrote it when no flag is chosen or it is not the expected shape. */
function configWithFlags(config: string, flags: readonly McpFlag[]): string {
  if (flags.length === 0) return config;
  try {
    const parsed = JSON.parse(config) as { mcpServers?: { msime?: { args?: unknown } } };
    const entry = parsed.mcpServers?.msime;
    if (!entry || !Array.isArray(entry.args)) return config;
    entry.args = [...entry.args, ...flags];
    return JSON.stringify(parsed, null, 2);
  } catch {
    return config;
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
  install?: (client: McpClientId, replace: boolean) => Promise<McpInstallOutcome>;
  copyText?: (text: string) => Promise<void>;
}) {
  const { confirm, confirmation } = useConfirm();
  const [server, setServer] = useState<McpServerStatus>();
  const [loadFailed, setLoadFailed] = useState(false);
  const [busy, setBusy] = useState<McpClientId>();
  const [result, setResult] = useState<string>();
  const [copied, setCopied] = useState<string>();
  const [tab, setTab] = useState<McpTab>("claude_code");
  const [flags, setFlags] = useState<McpFlag[]>([]);
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

  async function write(id: McpClientId) {
    if (!install || busy !== undefined || actionRunning.current) return;
    const generation = clientGeneration.current;
    const name = clientNames[id];
    actionRunning.current = true;
    setBusy(id);
    setResult(undefined);
    try {
      let outcome: McpInstallOutcome;
      try {
        outcome = await install(id, false);
      } catch (error) {
        if (errorCode(error) !== "mcp_entry_exists") throw error;
        const replace = await confirm({
          title: `替换 ${name} 中的 msime？`,
          message: `${name} 的配置里已有另一个名为 msime 的服务器。替换后，它原来的命令和参数（包括手动加上的 --allow-write）会被这里的设置覆盖。`,
          confirmLabel: "替换",
        });
        if (!replace) return;
        if (!mounted.current || generation !== clientGeneration.current) return;
        outcome = await install(id, true);
      }
      if (mounted.current && generation === clientGeneration.current) {
        setResult(
          outcome === "unchanged"
            ? `${name} 已经连接，无需改动。`
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

  function copyButton(key: string, label: string, text: string) {
    if (!copyText) return null;
    return (
      <div className={settings.managerActions}>
        <button type="button" className="secondary" onClick={() => copy(key, text)}>
          {copied === key ? "已复制" : label}
        </button>
      </div>
    );
  }

  return (
    <GroupList title="连接 AI 助手">
      <div className={settings.managerBlock} role="group" aria-label="连接 AI 助手">
        <p className={settings.managerNote}>
          连接后，把输入法的问题（卡顿、候选框不见了）直接告诉 AI
          助手：它会打开诊断日志、请你重做一遍出问题的操作，再读日志找原因；也能读取快捷短语、设置、打字统计和已安装的候选框皮肤。通过
          MCP 在本机运行，不联网，除了开关诊断日志不改动任何设置。
        </p>
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
                  <p className={settings.managerNote}>
                    在终端运行下面的命令，然后重新启动{" "}
                    {shownTab === "claude_code" ? "Claude Code" : "Codex"}：
                  </p>
                  <pre
                    className={command}
                    aria-label={
                      shownTab === "claude_code" ? "Claude Code 安装命令" : "Codex 安装命令"
                    }
                  >
                    {installCommand(shownTab, server, flags)}
                  </pre>
                  {copyButton(shownTab, "复制命令", installCommand(shownTab, server, flags))}
                  <p className={settings.managerNote}>之前添加过的，先运行这条：</p>
                  <pre
                    className={command}
                    aria-label={
                      shownTab === "claude_code" ? "Claude Code 移除命令" : "Codex 移除命令"
                    }
                  >
                    {removeCommand(shownTab)}
                  </pre>
                  {copyButton(`${shownTab}-remove`, "复制移除命令", removeCommand(shownTab))}
                </>
              )}
              {shownTab === "terminal" && (
                <>
                  <p className={settings.managerNote}>
                    不注册 MCP 也可以：能在终端里运行命令的助手（Claude Code、Codex
                    等）直接调用同一组工具，权限开关相同，不用重启助手。把下面这段话告诉助手，或放进项目的
                    AGENTS.md / CLAUDE.md：
                  </p>
                  <pre className={command} aria-label="命令行用法">
                    {terminalInstructions(server, flags)}
                  </pre>
                  {copyButton("terminal", "复制说明", terminalInstructions(server, flags))}
                </>
              )}
              {client && (
                <>
                  <p className={settings.managerNote}>
                    写入 <code>{client.path}</code>
                    {client.configured ? "（已连接）" : ""}，重新启动 {clientNames[client.id]}{" "}
                    后生效。
                  </p>
                  {flags.length > 0 && (
                    <p className="notice">
                      一键写入的配置不含下面开启的权限；需要这些权限时，请到「其他」复制配置并手动粘贴。
                    </p>
                  )}
                  <div className={settings.managerActions}>
                    <button
                      type="button"
                      className="secondary"
                      disabled={busy !== undefined || client.configured}
                      aria-busy={busy === client.id}
                      onClick={() => void write(client.id)}
                    >
                      {busy === client.id ? "正在写入…" : `写入 ${clientNames[client.id]}`}
                    </button>
                  </div>
                </>
              )}
              {shownTab === "json" && (
                <>
                  <p className={settings.managerNote}>粘贴到任意支持 MCP 的助手的配置中：</p>
                  <pre className={code} aria-label="MCP 配置">
                    {configWithFlags(server.config, flags)}
                  </pre>
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
                    onChange={(on) =>
                      setFlags((current) =>
                        permissionFlags
                          .map((candidate) => candidate.flag)
                          .filter((flag) =>
                            flag === permission.flag ? on : current.includes(flag),
                          ),
                      )
                    }
                  />
                </div>
              ))}
              <p className={settings.managerNote}>只在你信任该助手时开启这两项权限。</p>
              <p className={settings.managerNote}>
                服务器程序 <code>{server.command}</code>
                {server.installed ? "" : "（未找到，请重新安装输入法）"}
              </p>
            </>
          ) : (
            <p className="notice">输入法尚未完成初始化，完成设置向导后即可连接。</p>
          ))}
        {confirmation}
      </div>
    </GroupList>
  );
}
