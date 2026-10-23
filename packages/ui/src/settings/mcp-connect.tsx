import { useTemporaryFlag } from "../core/use-temporary-flag";
import { useCallback, useEffect, useState } from "react";
import { useConfirm } from "../core/confirm";
import { readOptionalJson, writeOptionalJson } from "../core/optional-storage";
import { errorCode } from "../core/error-code";
import { SettingsServiceRow } from "./settings-service-row";
import { GroupList, Segmented, Switch } from "../core/platform-controls";
import { mcpFailureMessage } from "./mcp-errors";
import { useAsyncActionRunner } from "../core/use-async-action";
import { SyntaxBlock, type SyntaxToken, tokensText } from "./mcp-syntax";
import { ActionButton } from "./action-button";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsNotice } from "./settings-notice";
import { StatusMessage } from "../core/status-message";
import { ErrorAlert } from "../core/error-alert";
import { useAsyncGeneration } from "./use-async-generation";
import {
  mcpConfigWithFlags,
  mcpGrantMessage,
  mcpInstallCommand,
  mcpRemoveCommand,
  mcpTerminalInstructions,
  type McpClientId,
  type McpFlag,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./mcp-command-model";

export type {
  McpClientId,
  McpClientStatus,
  McpFlag,
  McpInstallOutcome,
  McpServerStatus,
} from "./mcp-command-model";

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
    detail: "修改快捷短语和设置、制作、切换和删除候选窗口皮肤，用设置里登录的账号把皮肤发布到社区",
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
  const value = readOptionalJson(flagsStorageKey);
  if (Array.isArray(value)) return allFlags.filter((flag) => value.includes(flag));
  return allFlags;
}

function saveFlags(flags: readonly McpFlag[]) {
  writeOptionalJson(flagsStorageKey, flags);
}

function sameFlags(a: readonly McpFlag[], b: readonly McpFlag[]): boolean {
  return a.length === b.length && a.every((flag) => b.includes(flag));
}

/** The tabs: two command-line assistants, running the tools straight from a terminal, the clients the host writes into, and the JSON for anything else. */
type McpTab = "claude_code" | "codex" | "terminal" | McpClientId | "json";

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
  const [busyClient, setBusyClient] = useState<McpClientId>();
  const [result, setResult] = useState<string>();
  const [copied, setCopied] = useState<string>();
  const [copyFailed, setCopyFailed] = useState(false);
  const [copyFeedbackVisible, showCopyFeedback] = useTemporaryFlag(1600);
  const [tab, setTab] = useState<McpTab>("claude_code");
  // 用户想要的权限，复制的命令和配置、未连接的助手都按它来。
  const [preferred, setPreferred] = useState<McpFlag[]>(savedFlags);
  // 已连接的助手页上，开关先显示它现有条目的权限；用户改过之后记在这里，直到写入。
  const [drafts, setDrafts] = useState<Partial<Record<McpClientId, McpFlag[]>>>({});
  const refreshGeneration = useAsyncGeneration(status, install, copyText);
  const copyGeneration = useAsyncGeneration(status, install, copyText);
  const {
    busy,
    mounted,
    generation: clientGeneration,
    run,
  } = useAsyncActionRunner((message) => setResult(message), undefined, status, install, copyText);

  useEffect(() => {
    setBusyClient(undefined);
    setCopied(undefined);
    setCopyFailed(false);
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
  }, [refresh, install, copyText]);

  function write(id: McpClientId, flags: readonly McpFlag[]) {
    if (!install || busy) return;
    const generation = clientGeneration.current;
    const name = clientNames[id];
    const pending = run(
      async (isCurrent) => {
        if (flags.length > 0) {
          const granted = await confirm({
            title: `允许 ${name} 中的助手使用这些权限？`,
            message: mcpGrantMessage(name, flags),
            confirmLabel: "允许并写入",
          });
          if (!granted) return;
          if (!isCurrent()) return;
        }
        let outcome: McpInstallOutcome;
        try {
          outcome = await install(id, false, flags);
        } catch (error) {
          if (!isCurrent()) return;
          if (errorCode(error) !== "mcp_entry_exists") throw error;
          const replace = await confirm({
            title: `替换 ${name} 中的 msime？`,
            message: `${name} 的配置里已有另一个名为 msime 的服务器。替换后，它原来的命令和参数（包括手动加上的 --allow-write）会被这里的设置覆盖。`,
            confirmLabel: "替换",
          });
          if (!replace) return;
          if (!isCurrent()) return;
          outcome = await install(id, true, flags);
        }
        if (isCurrent()) {
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
        if (!isCurrent()) return;
        await refresh();
      },
      { formatError: (error) => mcpFailureMessage(error, name) },
    );
    if (!pending) return;
    setBusyClient(id);
    void pending.finally(() => {
      if (mounted.current && generation === clientGeneration.current) setBusyClient(undefined);
    });
  }

  async function copy(key: string, text: string) {
    if (!copyText) return;
    const generation = ++copyGeneration.current;
    setCopied(undefined);
    setCopyFailed(false);
    try {
      await copyText(text);
      if (!mounted.current || generation !== copyGeneration.current) return;
      setCopied(key);
      showCopyFeedback();
    } catch {
      if (!mounted.current || generation !== copyGeneration.current) return;
      setCopyFailed(true);
    }
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
  const outdated = client?.configured === true && (client.stale || !sameFlags(flags, client.flags));

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
        <ActionButton
          action={() => copy(key, text)}
          label={copyFeedbackVisible && copied === key ? "已复制" : label}
        />
      </SettingsManagerActions>
    );
  }

  return (
    <GroupList title="连接 AI 助手">
      <SettingsManagerBlock role="group" aria-label="连接 AI 助手">
        <SettingsManagerNote>
          MCP（Model Context Protocol）让 AI
          助手通过本机服务调用水杉输入法的功能，除了把皮肤发布到社区之外不联网。连接后，助手可以管理快捷短语和设置、制作、切换和删除候选窗口皮肤、读取打字统计和已安装皮肤，并用你在设置里登录的账号把皮肤发布到社区（助手自己不登录）；遇到卡顿或候选窗口问题时，也能读取诊断日志帮助排查。助手能否修改内容由下面两个开关决定，默认都开；复制的命令、配置和一键写入都带上开着的权限。两个都关时只读，除了开关诊断日志不改动任何设置。
        </SettingsManagerNote>
        {loadFailed && <ErrorAlert>无法读取 MCP 服务器的状态。</ErrorAlert>}
        {copyFailed && <ErrorAlert>复制失败，请重试。</ErrorAlert>}
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
                    tokens={mcpInstallCommand(shownTab, server, flags)}
                  />
                  {copyButton(shownTab, "复制命令", mcpInstallCommand(shownTab, server, flags))}
                  <SettingsManagerNote>之前添加过的，先运行这条：</SettingsManagerNote>
                  <SyntaxBlock
                    className={command}
                    aria-label={
                      shownTab === "claude_code" ? "Claude Code 移除命令" : "Codex 移除命令"
                    }
                    tokens={mcpRemoveCommand(shownTab)}
                  />
                  {copyButton(`${shownTab}-remove`, "复制移除命令", mcpRemoveCommand(shownTab))}
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
                    tokens={mcpTerminalInstructions(server, flags)}
                  />
                  {copyButton("terminal", "复制说明", mcpTerminalInstructions(server, flags))}
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
                    <SettingsNotice>
                      {client.stale
                        ? `${clientNames[client.id]} 现在的配置指向 Nix store 里某一版的服务器程序，系统升级或垃圾回收后会失效。`
                        : `下面的权限和 ${clientNames[client.id]} 现在的配置不同。`}
                      点「更新 {clientNames[client.id]}」
                      {client.stale && (
                        <>
                          改成 <code>{server.command}</code>，按下面的权限
                        </>
                      )}
                      写入。
                    </SettingsNotice>
                  )}
                  <SettingsManagerActions>
                    <ActionButton
                      action={() => void write(client.id, flags)}
                      disabled={
                        busy || busyClient !== undefined || (client.configured && !outdated)
                      }
                      ariaBusy={busyClient === client.id}
                      label={
                        busyClient === client.id
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
                    tokens={mcpConfigWithFlags(server.config, flags)}
                  />
                  {copyButton("json", "复制配置", mcpConfigWithFlags(server.config, flags))}
                </>
              )}
              {result && <StatusMessage role="status">{result}</StatusMessage>}
              {permissionFlags.map((permission) => (
                <SettingsServiceRow key={permission.flag}>
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
                </SettingsServiceRow>
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
            <SettingsNotice>输入法尚未完成初始化，完成设置向导后即可连接。</SettingsNotice>
          ))}
        {confirmation}
      </SettingsManagerBlock>
    </GroupList>
  );
}
