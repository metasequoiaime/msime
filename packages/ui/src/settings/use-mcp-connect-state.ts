import { useCallback, useEffect, useState } from "react";
import type { ConfirmRequest } from "../core/confirm";
import { errorCode } from "../core/error-code";
import { readOptionalJson, writeOptionalJson } from "../core/optional-storage";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useTemporaryFlag } from "../core/use-temporary-flag";
import { useAsyncGeneration } from "./use-async-generation";
import { mcpFailureMessage } from "./mcp-errors";
import {
  mcpGrantMessage,
  type McpClientId,
  type McpFlag,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./mcp-command-model";

export const mcpClientNames: Record<McpClientId, string> = {
  claude_desktop: "Claude Desktop",
  cursor: "Cursor",
};

/** 放宽助手权限的两个开关，按写进 `args` 的固定顺序排列。`msime-mcp` 不带参数时两项都是关的；设置页默认替用户打开。 */
export const mcpPermissionFlags = [
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

const allFlags: McpFlag[] = mcpPermissionFlags.map((permission) => permission.flag);

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

/** 两种命令行助手、终端、宿主可写入的客户端，以及其他客户端的 JSON 配置。 */
type McpTab = "claude_code" | "codex" | "terminal" | McpClientId | "json";

export interface McpConnectOptions {
  status: () => Promise<McpServerStatus>;
  install?: (
    client: McpClientId,
    replace: boolean,
    flags: readonly McpFlag[],
  ) => Promise<McpInstallOutcome>;
  copyText?: (text: string) => Promise<void>;
}

/** MCP 连接的状态与操作；宿主能力和确认均由展示入口注入。 */
export function useMcpConnectState({
  status,
  install,
  copyText,
  confirm,
}: McpConnectOptions & {
  confirm: (request: ConfirmRequest) => Promise<boolean>;
}) {
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
    const name = mcpClientNames[id];
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
    ...writableClients.map((client) => ({ value: client.id, label: mcpClientNames[client.id] })),
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

  function selectTab(next: McpTab) {
    setTab(next);
    setResult(undefined);
  }

  return {
    server,
    loadFailed,
    busy,
    busyClient,
    result,
    copied,
    copyFailed,
    copyFeedbackVisible,
    tabs,
    shownTab,
    client,
    flags,
    outdated,
    selectTab,
    setFlag,
    write,
    copy,
  };
}
