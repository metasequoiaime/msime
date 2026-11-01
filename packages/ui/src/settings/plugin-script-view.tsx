import { useRef, useState } from "react";
import { GroupList, Row } from "../core/platform-controls";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerNote } from "./settings-manager-note";
import { PluginViewHeader } from "./plugin-view-header";
import { PluginInfoGroup } from "./plugin-info-group";
import { SummaryRow } from "./summary-row";
import { SwitchRow } from "./switch-row";
import { ActionButton } from "./action-button";
import { ScriptGrantDialog } from "./script-grant-dialog";
import { pluginErrorMessage } from "./plugin-catalog-helpers";
import { ScriptSettingsForm } from "./plugin-script-settings";
import { useScriptPluginState, type ScriptPluginStateClient } from "./use-script-plugin-state";
import type { ConfirmRequest } from "../core/confirm";
import {
  SCRIPT_BUILTIN_NOTE,
  SCRIPT_COMMAND_MODE_NOTE,
  capabilityDescription,
  capabilityLabels,
  formatKilobytes,
  healthLabels,
  hookLabels,
  scriptCommands,
  scriptCommandStatus,
  scriptGrantState,
  scriptHealth,
  scriptResidualRisk,
  scriptStaleLabel,
  shortHash,
  sortCapabilities,
  storedDataDeletionNote,
  type ScriptGrantState,
} from "./plugin-script-model";
import type {
  PluginPackage,
  PluginSettingsPage,
  ScriptCapability,
  ScriptGrantsResult,
} from "./plugin-types";

// 保留原模块的导出，内部调用方按模型、对话框和详情各自的职责导入。
export { ScriptGrantDialog, type ScriptGrantDialogProps } from "./script-grant-dialog";
export {
  SCRIPT_CAPABILITIES,
  CAPABILITY_REQUIRES,
  EXCLUSIVE_CAPABILITIES,
  CAPABILITY_GROUPS,
  capabilityLabels,
  hookLabels,
  healthLabels,
  shortHash,
  exclusivePartners,
  toggleCapability,
  type ScriptGrantState,
  scriptGrantState,
  runningScriptIds,
  scriptHealth,
  scriptMarker,
  scriptCommands,
  SCRIPT_BUILTIN_NOTE,
  scriptStaleLabel,
  SCRIPT_COMMAND_MODE_NOTE,
  scriptCommandStatus,
  scriptResidualRisk,
} from "./plugin-script-model";

export interface ScriptPluginDetailViewProps {
  pack: PluginPackage;
  grants: ScriptGrantsResult | null;
  /** 宿主会运行的插件 id，按执行顺序（`runningScriptIds`）。 */
  running: readonly string[];
  platform?: string;
  working: boolean;
  /** / 指令是否打开（`local_modes.command`）；提供指令的插件在它关着时说明并给出「打开 / 指令」按钮。 */
  commandMode?: boolean;
  /** 打开「输入 → 快捷模式」里的 / 指令，留在当前页面；没有时改为「前往输入设置」（`onOpenPage`）。 */
  onLocalMode?: (mode: "command" | "mention") => void;
  onOpenPage?: (page: PluginSettingsPage) => void;
  /** 冲突说明里把另一个插件的 id 换成名字。 */
  nameOf?: (id: string) => string;
  /** 读写设置值和持久存储；宿主没有这几项时不显示设置表单和已存数据。 */
  client?: ScriptPluginStateClient;
  /** 停用会删数据、清除数据之前的确认。 */
  confirm: (request: ConfirmRequest) => Promise<boolean>;
  /** 写入授权并启用；失败时抛出，由授权对话框说明原因。 */
  onGrant: (granted: ScriptCapability[]) => Promise<void>;
  /** 停用并撤销全部授权。 */
  onRevoke: () => void;
  /** 在会运行的插件里往前（-1）或往后（1）挪一位。 */
  onMove: (offset: -1 | 1) => void;
  onRemove: (pack: PluginPackage) => void;
  onBack: () => void;
}

/** 一个脚本插件：它在什么时候运行、申请了什么、授予了什么、输入法有没有停用它、排第几个执行、清单声明的设置项、自带和存下的数据，以及启用（经授权对话框）、调整授权、清除数据和删除。 */
export function ScriptPluginDetailView({
  pack,
  grants,
  running,
  platform,
  working,
  commandMode = false,
  onLocalMode,
  onOpenPage,
  nameOf,
  client,
  confirm,
  onGrant,
  onRevoke,
  onMove,
  onRemove,
  onBack,
}: ScriptPluginDetailViewProps) {
  const commands = scriptCommands(pack);
  const commandStatus = (trigger: string) => scriptCommandStatus(pack, trigger, grants, nameOf);
  const [dialog, setDialog] = useState(false);
  // 打开对话框的那个控件（启用开关或「调整授权」），对话框关掉后焦点回到它。
  const opener = useRef<HTMLElement | null>(null);
  const grant = grants?.plugins.find((entry) => entry.id === pack.id);
  const state = scriptGrantState(pack, grant);
  const active = state === "enabled";
  const health = scriptHealth(grants, pack);
  const requested = sortCapabilities(pack.permissions ?? []);
  const granted = active ? sortCapabilities(grant?.granted ?? []) : [];
  const position = running.indexOf(pack.id);
  const risk = scriptResidualRisk(platform);
  const {
    state: pluginState,
    loadError,
    canSave,
    canClear,
    saveSettings,
    clearData,
  } = useScriptPluginState(client, pack.id, pack.content_hash, grants?.revision);
  const settings = pack.settings ?? [];
  const dataBytes = pack.data_bytes ?? 0;
  const storageBytes = pluginState?.storage_bytes ?? 0;
  // 授权文件里记着 `persist_data`（不论内容哈希对不对得上：更新之后数据保留到重新授权），或者存储里有数据。
  const persisting = Boolean(grant?.granted.includes("persist_data")) || storageBytes > 0;
  const [clearing, setClearing] = useState(false);
  const [clearError, setClearError] = useState("");
  const revoke = async () => {
    if (persisting) {
      const confirmed = await confirm({
        title: "停用脚本插件",
        message: `停用会撤销「${pack.name}」的全部授权，${storedDataDeletionNote(storageBytes)}`,
        confirmLabel: "停用并删除数据",
        danger: true,
      });
      if (!confirmed) return;
    }
    onRevoke();
  };
  const clear = async () => {
    const confirmed = await confirm({
      title: "清除数据",
      message: `清除「${pack.name}」存下的数据？${storedDataDeletionNote(storageBytes)}`,
      confirmLabel: "清除",
      danger: true,
    });
    if (!confirmed) return;
    setClearing(true);
    setClearError("");
    try {
      await clearData();
    } catch (error) {
      setClearError(pluginErrorMessage(error, "数据没有清除，请重试。"));
    } finally {
      setClearing(false);
    }
  };
  const stateText: Record<ScriptGrantState, string> = {
    ungranted: "还没有授权，不会运行。",
    stale: `${scriptStaleLabel(pack)}：插件的内容和授权时不同，之前的授权已经作废，不会运行。`,
    enabled: "已启用。",
    disabled: "已停用，不会运行。",
  };
  const openDialog = () => {
    const focused = document.activeElement;
    opener.current = focused instanceof HTMLElement ? focused : null;
    setDialog(true);
  };
  const closeDialog = () => {
    setDialog(false);
    opener.current?.focus();
    opener.current = null;
  };
  return (
    <>
      <PluginViewHeader title={pack.name} onBack={onBack} />
      <PluginInfoGroup pack={pack} kindLabel="脚本插件（实验）">
        {pack.builtin && (
          <SummaryRow title="来源" breakAnywhere>
            {SCRIPT_BUILTIN_NOTE}
          </SummaryRow>
        )}
        <SummaryRow
          title="内容指纹"
          description="插件内容的哈希前 12 位；内容一变，指纹就变，之前的授权随之作废。"
        >
          <code>{shortHash(pack.content_hash)}</code>
        </SummaryRow>
      </PluginInfoGroup>
      <GroupList title="在这些时候运行">
        {(pack.hooks ?? []).map((hook) => (
          <Row key={hook} title={hookLabels[hook] ?? hook} />
        ))}
      </GroupList>
      <GroupList title="申请的权限">
        {requested.length === 0 ? (
          <SettingsGroupNote inline>
            没有申请任何权限：只能知道发生了什么事件，读不到也改不了文字。
          </SettingsGroupNote>
        ) : (
          requested.map((capability) => (
            <SummaryRow
              key={capability}
              title={capabilityLabels[capability].title}
              description={capabilityDescription(capability, pack)}
            >
              {granted.includes(capability) ? "已授予" : "未授予"}
            </SummaryRow>
          ))
        )}
      </GroupList>
      {commands.length > 0 && (
        <GroupList title="提供的 / 指令">
          {commands.map((command) => (
            <SummaryRow
              key={command.trigger}
              title={`/${command.trigger}`}
              description={command.title}
            >
              {commandStatus(command.trigger)}
            </SummaryRow>
          ))}
          {!commandMode && (
            <SettingsManagerBlock>
              <SettingsManagerNote>{SCRIPT_COMMAND_MODE_NOTE}</SettingsManagerNote>
              {onLocalMode ? (
                <SettingsManagerActions>
                  <ActionButton action={() => onLocalMode("command")} label="打开 / 指令" />
                </SettingsManagerActions>
              ) : (
                onOpenPage && (
                  <SettingsManagerActions>
                    <ActionButton action={() => onOpenPage("input")} label="前往输入设置" />
                  </SettingsManagerActions>
                )
              )}
            </SettingsManagerBlock>
          )}
        </GroupList>
      )}
      <GroupList title="状态">
        <SettingsGroupNote role="status">
          {stateText[state]}
          {active && health ? `${healthLabels[health.disabled]}，重新授权或重启输入法后恢复。` : ""}
        </SettingsGroupNote>
        <SwitchRow
          title="启用"
          description="打开时先确认要授予的权限。关闭会撤销全部授权，输入法在下一次切换输入框时停止运行它。"
          checked={active}
          disabled={working}
          onChange={(checked) => {
            if (checked) openDialog();
            else void revoke();
          }}
        />
        {state === "enabled" && requested.length > 0 && (
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton action={openDialog} disabled={working} label="调整授权" />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        )}
      </GroupList>
      {/* 只有一个脚本在运行时两个按钮都不能按，顺序也无从谈起，所以至少两个在运行时才显示；排在第 8 个之后不会运行时仍要说明。 */}
      {state === "enabled" && (running.length >= 2 || position < 0) && (
        <GroupList title="执行顺序">
          <SettingsManagerBlock>
            <SettingsManagerNote>
              {position < 0
                ? "同时启用的脚本插件超过 8 个，这个插件排在第 8 个之后，不会运行。"
                : `第 ${position + 1} 位，共 ${running.length} 个。多个插件按这个顺序依次处理同一件事。`}
            </SettingsManagerNote>
            <SettingsManagerActions>
              <ActionButton
                action={() => onMove(-1)}
                disabled={working || position <= 0}
                label="提前"
              />
              <ActionButton
                action={() => onMove(1)}
                disabled={working || position < 0 || position >= running.length - 1}
                label="推后"
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        </GroupList>
      )}
      {settings.length > 0 &&
        canSave &&
        (pluginState ? (
          <ScriptSettingsForm
            key={`${pack.id}:${pack.content_hash ?? ""}`}
            settings={settings}
            values={pluginState.values}
            disabled={working}
            onSave={saveSettings}
          />
        ) : (
          loadError && (
            <GroupList title="设置">
              <SettingsGroupNote inline className="text-danger">
                {loadError}
              </SettingsGroupNote>
            </GroupList>
          )
        ))}
      {(dataBytes > 0 || persisting) && (
        <GroupList title="数据">
          {dataBytes > 0 && (
            <SummaryRow title="自带数据" description="插件包里随附的只读数据，插件读它不需要授权。">
              {formatKilobytes(dataBytes)}
            </SummaryRow>
          )}
          {persisting && (
            <>
              <SummaryRow
                title="已存数据"
                description="插件长期保存在这台电脑上的数据，最多 64 KB。撤销「长期保存数据」或删除插件时一起删除。"
              >
                {formatKilobytes(storageBytes)}
              </SummaryRow>
              {canClear && (
                <SettingsManagerBlock>
                  {clearError && (
                    <SettingsManagerNote className="text-danger" role="alert">
                      {clearError}
                    </SettingsManagerNote>
                  )}
                  <SettingsManagerActions>
                    <ActionButton
                      action={clear}
                      disabled={working || clearing || storageBytes === 0}
                      ariaBusy={clearing}
                      label="清除数据"
                    />
                  </SettingsManagerActions>
                </SettingsManagerBlock>
              )}
            </>
          )}
        </GroupList>
      )}
      <GroupList title="安全">
        <SettingsGroupNote>
          脚本插件在输入法里的沙箱中运行，只能做你授予的事，读不到文件和网络，也不会在密码框里运行。
        </SettingsGroupNote>
        {risk && <SettingsGroupNote inline>{risk}</SettingsGroupNote>}
      </GroupList>
      {!pack.builtin && (
        <GroupList>
          <SettingsManagerBlock>
            <SettingsManagerNote>
              删除后插件的文件会从本机移除，它的授权也一起去掉。
            </SettingsManagerNote>
            <SettingsManagerActions>
              <ActionButton
                action={() => onRemove(pack)}
                ariaBusy={working}
                ariaLabel={`删除${pack.name}`}
                disabled={working}
                label="删除"
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        </GroupList>
      )}
      {dialog && (
        <ScriptGrantDialog
          pack={pack}
          initial={granted}
          saved={grant}
          storageBytes={storageBytes}
          stale={state === "stale"}
          platform={platform}
          commandStatus={state === "enabled" ? commandStatus : undefined}
          onCancel={closeDialog}
          onConfirm={async (next) => {
            await onGrant(next);
            closeDialog();
          }}
        />
      )}
    </>
  );
}
