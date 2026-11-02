import {
  useMcpConnectState,
  mcpClientNames,
  mcpPermissionFlags,
  type McpConnectOptions,
} from "./use-mcp-connect-state";
import { useConfirm } from "../core/confirm";
import { SettingsServiceRow } from "./settings-service-row";
import { GroupList, Segmented, Switch } from "../core/platform-controls";
import { SyntaxBlock, type SyntaxToken, tokensText } from "./mcp-syntax";
import { ActionButton } from "./action-button";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsGroupNote } from "./settings-group-note";
import { InfoTip } from "../core/info-tip";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsNotice } from "./settings-notice";
import { StatusMessage } from "../core/status-message";
import { ErrorAlert } from "../core/error-alert";
import {
  mcpConfigWithFlags,
  mcpInstallCommand,
  mcpRemoveCommand,
  mcpTerminalInstructions,
} from "./mcp-command-model";

export type {
  McpClientId,
  McpClientStatus,
  McpFlag,
  McpInstallOutcome,
  McpServerStatus,
} from "./mcp-command-model";

const code =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed";
const command =
  "m-0 overflow-x-auto rounded-lg border border-edge bg-raised p-3 text-xs leading-relaxed whitespace-pre-wrap break-all";

/** 宿主按入口注入可用能力；页面负责展示命令和确认框。 */
export function McpConnectSection({ status, install, copyText }: McpConnectOptions) {
  const { confirm, confirmation } = useConfirm();
  const {
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
  } = useMcpConnectState({ status, install, copyText, confirm });

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
      {/* MCP 是什么、连上之后能做什么：解释整组，收进组名旁的「?」。放在 `SettingsManagerBlock` 外面，那里面的说明照旧画在原处。 */}
      <SettingsGroupNote>
        MCP（Model Context Protocol）让 AI
        助手通过本机服务调用水杉输入法的功能，除了把皮肤发布到社区之外不联网。连接后，助手可以管理快捷短语和设置、制作、切换和删除候选窗口皮肤、读取打字统计和已安装皮肤，并用你在设置里登录的账号把皮肤发布到社区（助手自己不登录）；遇到卡顿或候选窗口问题时，也能读取诊断日志帮助排查。助手能否修改内容由下面两个开关决定，默认都开；复制的命令、配置和一键写入都带上开着的权限。两个都关时只读，除了开关诊断日志不改动任何设置。
      </SettingsGroupNote>
      <SettingsManagerBlock role="group" aria-label="连接 AI 助手">
        {loadFailed && <ErrorAlert>无法读取 MCP 服务器的状态。</ErrorAlert>}
        {copyFailed && <ErrorAlert>复制失败，请重试。</ErrorAlert>}
        {server &&
          (server.config ? (
            <>
              <Segmented
                aria-label="AI 助手"
                options={tabs}
                value={shownTab}
                onChange={selectTab}
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
                    {client.configured ? "（已连接）" : ""}，重新启动 {mcpClientNames[client.id]}{" "}
                    后生效。
                  </SettingsManagerNote>
                  {outdated && (
                    <SettingsNotice>
                      {client.stale
                        ? `${mcpClientNames[client.id]} 现在的配置指向 Nix store 里某一版的服务器程序，系统升级或垃圾回收后会失效。`
                        : `下面的权限和 ${mcpClientNames[client.id]} 现在的配置不同。`}
                      点「更新 {mcpClientNames[client.id]}」
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
                          : `${outdated ? "更新" : "写入"} ${mcpClientNames[client.id]}`
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
              {mcpPermissionFlags.map((permission) => (
                <SettingsServiceRow key={permission.flag}>
                  <span>
                    <span className="inline-flex items-center gap-1">
                      {permission.title}
                      <InfoTip subject={permission.title}>{permission.detail}</InfoTip>
                    </span>
                    <small>
                      <code>{permission.flag}</code>
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
