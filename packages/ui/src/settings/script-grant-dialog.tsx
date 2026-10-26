import { useEffect, useId, useRef } from "react";
import { Checks } from "../core/platform-controls";
import * as dialogStyle from "../community/community-style";
import {
  CAPABILITY_GROUPS,
  CAPABILITY_REQUIRES,
  capabilityDescription,
  capabilityLabels,
  exclusivePartners,
  scriptCommands,
  scriptResidualRisk,
  scriptStaleLabel,
  storedDataDeletionNote,
} from "./plugin-script-model";
import type { PluginPackage, ScriptCapability, ScriptGrant } from "./plugin-types";
import { useScriptGrantDialog } from "./use-script-grant-dialog";

export interface ScriptGrantDialogProps {
  pack: PluginPackage;
  /** 打开时勾选着的能力：调整授权时是现在授予的，新授权时（包括内容变了之后重新授权）为空，所以「长期保存数据」从不预先勾上。 */
  initial: readonly ScriptCapability[];
  /** 授权文件里这个插件现在的那一条，不论内容哈希对不对得上。 */
  saved?: Pick<ScriptGrant, "granted">;
  /** 持久存储现在的字节数；不知道时为 0。 */
  storageBytes?: number;
  /** 授权过、但插件的内容已经变了。 */
  stale: boolean;
  platform?: string;
  /** 插件指令现在的状态（`scriptCommandStatus`）；调整授权时由详情页给，新授权时没有。 */
  commandStatus?: (trigger: string) => string;
  /** 写入授权并启用；失败时抛出宿主的 `{code, detail}`，对话框留着并说明原因。 */
  onConfirm: (granted: ScriptCapability[]) => Promise<void>;
  onCancel: () => void;
}

/** 授权对话框：清单申请的每项能力一个复选框，按读文字、改文字、记住数据分组，依赖联动，互斥的两项不能同时勾选。确认后启用插件；这次授权会删掉插件存下的数据时先说明并再确认一次。 */
export function ScriptGrantDialog({
  pack,
  initial,
  saved,
  storageBytes = 0,
  stale,
  platform,
  commandStatus,
  onConfirm,
  onCancel,
}: ScriptGrantDialogProps) {
  const { requested, selected, busy, error, confirmingDeletion, setCapability, confirm } =
    useScriptGrantDialog({ pack, initial, saved, storageBytes, onConfirm });
  const commands = scriptCommands(pack);
  const titleId = useId();
  const cancelButton = useRef<HTMLButtonElement>(null);
  const risk = scriptResidualRisk(platform);
  // 从「取消」开始：对话框是从开关打开的，按住不放的空格或回车不该直接把读写文字的权限授出去。
  useEffect(() => {
    cancelButton.current?.focus();
  }, []);
  const settings = pack.settings ?? [];
  const item = (capability: ScriptCapability) => {
    const blockedBy =
      exclusivePartners(capability).find((partner) => selected.includes(partner)) ?? null;
    const required = CAPABILITY_REQUIRES[capability];
    const notes = [
      required ? `需要同时授予「${capabilityLabels[required].title}」。` : null,
      blockedBy ? `不能和「${capabilityLabels[blockedBy].title}」同时授予。` : null,
    ].filter(Boolean);
    // 「长期保存数据」的说明是红字警告。
    const warning = capability === "persist_data";
    return {
      value: capability,
      checked: selected.includes(capability),
      disabled: busy || blockedBy !== null,
      label: (
        <span className="flex flex-col">
          <span>{capabilityLabels[capability].title}</span>
          <span className={warning ? "text-xs text-danger" : "text-xs text-secondary"}>
            {capabilityDescription(capability, pack)}
          </span>
          {notes.length > 0 && <span className="text-xs text-secondary">{notes.join("")}</span>}
        </span>
      ),
    };
  };
  const groups = CAPABILITY_GROUPS.map((group) => ({
    title: group.title,
    capabilities: group.capabilities.filter((capability) => requested.includes(capability)),
  })).filter((group) => group.capabilities.length > 0);
  // 没有申请任何权限时插件还能做的事：随包数据和设置项都不需要授权。
  const unrestricted = [
    "知道发生了什么事件",
    (pack.data_bytes ?? 0) > 0 ? "读自己包里的数据" : null,
    settings.length > 0 ? "读你在设置项里填的值" : null,
  ].filter(Boolean);
  return (
    <div
      className={dialogStyle.backdrop}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) onCancel();
      }}
    >
      <div
        className={dialogStyle.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !busy) {
            event.stopPropagation();
            onCancel();
          }
        }}
      >
        <h2 id={titleId} className={dialogStyle.dialogTitle}>
          授权「{pack.name}」
        </h2>
        {stale && (
          <p className={dialogStyle.warning} role="status">
            {scriptStaleLabel(pack)}：这个插件的内容和你上次授权时不同，之前的授权已经作废。
          </p>
        )}
        <p className="m-0 leading-relaxed text-secondary">
          脚本插件是第三方代码，会在输入法里运行。只勾选你愿意让它做的事；没有勾选的，它一律做不到。在密码框和隐私输入框里它不会运行。
        </p>
        {commands.length > 0 && (
          <div className="flex flex-col gap-1">
            <p className="m-0 leading-relaxed text-secondary">它在 / 指令里提供：</p>
            <ul className="m-0 pl-5 leading-relaxed text-secondary" aria-label="它提供的 / 指令">
              {commands.map((command) => (
                <li key={command.trigger}>
                  /{command.trigger} {command.title}
                  {commandStatus ? `：${commandStatus(command.trigger)}` : ""}
                </li>
              ))}
            </ul>
          </div>
        )}
        {requested.length > 0 ? (
          <div className="flex flex-col gap-2" role="group" aria-label="允许这个插件">
            {groups.map((group) => (
              <Checks
                key={group.title}
                legend={group.title}
                items={group.capabilities.map(item)}
                onChange={setCapability}
              />
            ))}
          </div>
        ) : (
          <p className="m-0 leading-relaxed text-secondary">
            这个插件没有申请任何权限：它只能{unrestricted.join("、")}，读不到也改不了任何文字。
          </p>
        )}
        {settings.length > 0 && requested.length > 0 && (
          <p className="m-0 leading-relaxed text-secondary">
            插件能读到你在详情页设置项里填的值，这不需要授权。
          </p>
        )}
        {risk && <p className={dialogStyle.warning}>{risk}</p>}
        {confirmingDeletion && (
          <p className="m-0 leading-relaxed text-danger" role="alert">
            这次授权不包括「{capabilityLabels.persist_data.title}」，
            {storedDataDeletionNote(storageBytes)}
          </p>
        )}
        {error && (
          <p className="m-0 leading-relaxed text-danger" role="alert">
            {error}
          </p>
        )}
        <div className={dialogStyle.dialogActions}>
          <button
            type="button"
            ref={cancelButton}
            className="secondary"
            disabled={busy}
            onClick={onCancel}
          >
            取消
          </button>
          <button
            type="button"
            className="primary"
            disabled={busy}
            aria-busy={busy}
            onClick={() => void confirm()}
          >
            {confirmingDeletion ? "删除数据并授权" : "授权并启用"}
          </button>
        </div>
      </div>
    </div>
  );
}
