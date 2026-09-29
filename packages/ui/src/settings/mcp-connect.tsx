import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import * as settings from "./settings-style";
import { GroupList } from "../core/platform-controls";
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

/**
 * 「连接 AI 助手」: the `msime-mcp` entry an assistant runs, to copy or to write into Claude Desktop's or Cursor's configuration.
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
  const [copied, setCopied] = useState(false);
  const mounted = useRef(true);
  const refreshGeneration = useRef(0);
  const clientGeneration = useRef(0);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
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
    if (!install) return;
    const generation = clientGeneration.current;
    const name = clientNames[id];
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
      if (mounted.current && generation === clientGeneration.current) setBusy(undefined);
    }
  }

  return (
    <GroupList title="连接 AI 助手">
      <div className={settings.managerBlock} role="group" aria-label="连接 AI 助手">
        <p className={settings.managerNote}>
          连接后，直接告诉 AI
          助手输入法哪里不对劲（比如卡顿、候选框不见了），它会打开诊断日志、请你把出问题的操作再做一遍，然后读日志帮你找原因。它也能读取快捷短语、设置和打字统计。通过
          MCP（Model Context Protocol）在本机运行，不联网；除了开关诊断日志，默认不改动任何设置。
        </p>
        {loadFailed && <p role="alert">无法读取 MCP 服务器的状态。</p>}
        {server && (
          <>
            <div className={settings.serviceRow}>
              <span>
                服务器程序
                <small>
                  <code>{server.command}</code>
                  {server.installed ? "" : "（未找到，请重新安装输入法）"}
                </small>
              </span>
            </div>
            {server.config ? (
              <>
                <pre className={code} aria-label="MCP 配置">
                  {server.config}
                </pre>
                <p className="notice">
                  要让助手修改快捷短语和设置，在 args 中加入 <code>--allow-write</code>
                  ；要让它读取你的用户词库、查看编码的候选，加入{" "}
                  <code>--allow-dictionary-read</code>
                  ；两项都加才能增删、调整和导入词。这两项只应在你信任该助手时开启。
                </p>
                {copyText && (
                  <div className={settings.serviceRow}>
                    <span>复制后粘贴到任意支持 MCP 的助手的配置中</span>
                    <div>
                      <button
                        type="button"
                        className="secondary"
                        onClick={() =>
                          void copyText(server.config!).then(() => {
                            if (!mounted.current) return;
                            setCopied(true);
                            window.setTimeout(() => {
                              if (mounted.current) setCopied(false);
                            }, 1600);
                          })
                        }
                      >
                        {copied ? "已复制" : "复制配置"}
                      </button>
                    </div>
                  </div>
                )}
                {install &&
                  server.installed &&
                  server.clients.map((client) => (
                    <div className={settings.serviceRow} key={client.id}>
                      <span>
                        {clientNames[client.id]}
                        <small>
                          <code>{client.path}</code>
                          {client.configured ? "（已连接）" : ""}
                        </small>
                      </span>
                      <div>
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
                    </div>
                  ))}
              </>
            ) : (
              <p className="notice">输入法尚未完成初始化，完成设置向导后即可连接。</p>
            )}
            {result && <p role="status">{result}</p>}
          </>
        )}
        {confirmation}
      </div>
    </GroupList>
  );
}
