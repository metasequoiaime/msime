import { useEffect, useId, useRef, useState } from "react";
import { Checks, GroupList, Row } from "../core/platform-controls";
import * as dialogStyle from "../community/community-style";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerNote } from "./settings-manager-note";
import { PluginViewHeader } from "./plugin-view-header";
import { PluginInfoGroup } from "./plugin-info-group";
import { SummaryRow } from "./summary-row";
import { SwitchRow } from "./switch-row";
import { ActionButton } from "./action-button";
import { pluginErrorMessage } from "./plugin-catalog-helpers";
import type {
  PluginPackage,
  ScriptCapability,
  ScriptGrant,
  ScriptGrantsResult,
  ScriptHealth,
} from "./plugin-types";

/** 脚本插件的能力，按 `msime_script_runtime::Capability::ALL` 的顺序。 */
export const SCRIPT_CAPABILITIES: readonly ScriptCapability[] = [
  "read_composition",
  "read_commit",
  "read_candidates",
  "rewrite_commit",
  "rerank_candidates",
  "annotate_candidates",
  "filter_candidates",
];

/** 授予一项之前必须一起授予的能力：`Capability::requires`。 */
export const CAPABILITY_REQUIRES: Partial<Record<ScriptCapability, ScriptCapability>> = {
  rewrite_commit: "read_commit",
  rerank_candidates: "read_candidates",
  annotate_candidates: "read_candidates",
  filter_candidates: "read_candidates",
};

/** 不能同时授予的一对：改写上屏的插件读不到组字过程，免得它把放弃的组字夹带进后面的上屏（`CapabilitySet::check_rules`）。 */
export const EXCLUSIVE_CAPABILITIES: readonly [ScriptCapability, ScriptCapability] = [
  "rewrite_commit",
  "read_composition",
];

/** 每项能力的白话说明：标题说能做什么，说明说这意味着插件能看到或改动什么。 */
export const capabilityLabels: Record<ScriptCapability, { title: string; description: string }> = {
  read_composition: {
    title: "读取正在输入的内容",
    description: "能看到你正在打的拼音或编码、组字时显示的文字，以及每次按下的字符。",
  },
  read_commit: {
    title: "读取输入的文字",
    description: "能看到你选中的候选和最终输入到应用里的文字。",
  },
  read_candidates: {
    title: "读取候选词",
    description: "能看到候选窗口里当前一页的候选，最多 20 个。",
  },
  rewrite_commit: {
    title: "修改输入的文字",
    description: "能在文字输入到应用之前改写它，例如在中英文之间加空格。",
  },
  rerank_candidates: {
    title: "调整候选顺序",
    description: "能把候选词往前或往后排。",
  },
  annotate_candidates: {
    title: "给候选加注释",
    description: "能在候选旁边显示一小段说明。",
  },
  filter_candidates: {
    title: "隐藏候选",
    description: "能把某些候选移到末尾并隐藏起来。",
  },
};

/** 清单里钩子的白话名字：插件在什么时候运行。不认识的名字原样显示。 */
export const hookLabels: Record<string, string> = {
  focus_in: "进入输入框时",
  focus_out: "离开输入框时",
  input: "每次打字时",
  selection_retracted: "撤回选词时",
  candidate_selected: "选中候选时",
  before_commit: "文字输入到应用之前（可以改写）",
  commit: "文字输入到应用之后",
  composition_start: "开始组字时",
  composition_update: "组字变化时",
  composition_end: "组字结束时",
  candidates_shown: "候选窗口出现时",
  candidates_hidden: "候选窗口消失时",
  candidates: "生成候选时（可以调整候选）",
};

/** 输入法停用一个插件的原因：`script-health.json` 的四个类别。 */
export const healthLabels: Record<ScriptHealth["disabled"], string> = {
  invalid: "模块不符合要求，输入法已停用它",
  init: "初始化失败，输入法已停用它",
  failed: "运行时出错太多次，输入法已停用它",
  slow: "运行太慢，输入法已停用它",
};

/** 内容哈希的前 12 位，授权页用它让人分辨两个版本。 */
export function shortHash(hash: string | undefined): string {
  return (hash ?? "").slice(0, 12);
}

function sortCapabilities(capabilities: Iterable<ScriptCapability>): ScriptCapability[] {
  const set = new Set(capabilities);
  return SCRIPT_CAPABILITIES.filter((capability) => set.has(capability));
}

/** 和这一项互斥的那一项；不互斥时为 null。 */
export function exclusivePartner(capability: ScriptCapability): ScriptCapability | null {
  const [first, second] = EXCLUSIVE_CAPABILITIES;
  if (capability === first) return second;
  if (capability === second) return first;
  return null;
}

/**
 * 勾选或取消一项之后的授权，只在清单申请过的能力（`requested`）里变动：勾选时把它依赖的那一项一起勾上，并去掉和它互斥的那一项（以及依赖那一项的）；取消时把依赖它的各项一起取消。结果按 `SCRIPT_CAPABILITIES` 排序，所以总能通过宿主的依赖和互斥检查。
 */
export function toggleCapability(
  current: readonly ScriptCapability[],
  capability: ScriptCapability,
  checked: boolean,
  requested: readonly ScriptCapability[],
): ScriptCapability[] {
  const allowed = new Set(requested);
  const next = new Set(current.filter((item) => allowed.has(item)));
  const removeWithDependents = (target: ScriptCapability) => {
    next.delete(target);
    for (const item of SCRIPT_CAPABILITIES) {
      if (CAPABILITY_REQUIRES[item] === target) next.delete(item);
    }
  };
  if (checked) {
    if (!allowed.has(capability)) return sortCapabilities(next);
    next.add(capability);
    const required = CAPABILITY_REQUIRES[capability];
    if (required && allowed.has(required)) next.add(required);
    const partner = exclusivePartner(capability);
    if (partner) removeWithDependents(partner);
    // 勾上依赖项之后，它的互斥项也要让开（例如勾选「修改输入的文字」带上「读取输入的文字」）。
    if (required) {
      const requiredPartner = exclusivePartner(required);
      if (requiredPartner) removeWithDependents(requiredPartner);
    }
  } else {
    removeWithDependents(capability);
  }
  return sortCapabilities(next);
}

/** 一个脚本插件的授权状态，按页面要说的先后：没授权过、内容变了要重新授权、启用、停用。 */
export type ScriptGrantState = "ungranted" | "stale" | "enabled" | "disabled";

export function scriptGrantState(
  pack: Pick<PluginPackage, "content_hash">,
  grant: ScriptGrant | undefined,
): ScriptGrantState {
  if (!grant) return "ungranted";
  if (grant.hash !== pack.content_hash) return "stale";
  return grant.enabled ? "enabled" : "disabled";
}

/** 宿主会运行的插件 id，按执行顺序：启用、内容哈希对得上的条目，最多 8 个（`ScriptGrants::enabled`）。 */
export function runningScriptIds(
  grants: ScriptGrantsResult | null,
  packages: readonly PluginPackage[],
): string[] {
  if (!grants) return [];
  return grants.plugins
    .filter((grant) => grant.enabled)
    .slice(0, 8)
    .filter((grant) =>
      packages.some(
        (pack) =>
          pack.kind === "script" && pack.id === grant.id && pack.content_hash === grant.hash,
      ),
    )
    .map((grant) => grant.id);
}

/** 输入法记下的停用，只算这个版本（哈希相同）的。 */
export function scriptHealth(
  grants: ScriptGrantsResult | null,
  pack: Pick<PluginPackage, "id" | "content_hash">,
): ScriptHealth | undefined {
  return grants?.health.find((entry) => entry.id === pack.id && entry.hash === pack.content_hash);
}

/** 列表里这一行的标记。 */
export function scriptMarker(
  pack: PluginPackage,
  grants: ScriptGrantsResult | null,
  running: readonly string[],
): string {
  const grant = grants?.plugins.find((entry) => entry.id === pack.id);
  const health = scriptHealth(grants, pack);
  switch (scriptGrantState(pack, grant)) {
    case "ungranted":
      return "未授权";
    case "stale":
      return "已更新，需要重新授权";
    case "disabled":
      return "未启用";
    case "enabled": {
      if (health) return "已被输入法停用";
      const position = running.indexOf(pack.id);
      return position < 0 ? "已启用 · 超过 8 个，不运行" : `已启用 · 第 ${position + 1} 位`;
    }
  }
}

/** 授权页上的平台说明。授权文件不签名，桌面上同一用户下的程序都能绕过授权页自己写授权（设计接受的残余风险），要写明。 */
export function scriptResidualRisk(platform: string | undefined): string | null {
  if (platform === "macos") {
    return "在 macOS 上，以你的身份运行的其他程序可以不经过这个页面，直接写入授权，让脚本插件读取或改写你输入的文字。只安装你信任的脚本插件。";
  }
  if (platform === "windows") {
    return "在 Windows 上，以你的身份运行的其他程序可以不经过这个页面，直接写入授权，让脚本插件读取或改写你输入的文字。只安装你信任的脚本插件。以管理员身份运行的窗口里不会运行脚本插件。";
  }
  if (platform === "linux") {
    return "在 Linux 上，以你的身份运行的其他程序可以不经过这个页面，直接写入授权，让脚本插件读取或改写你输入的文字。只安装你信任的脚本插件。";
  }
  return null;
}

export interface ScriptGrantDialogProps {
  pack: PluginPackage;
  /** 打开时勾选着的能力：调整授权时是现在授予的，新授权时为空。 */
  initial: readonly ScriptCapability[];
  /** 授权过、但插件的内容已经变了。 */
  stale: boolean;
  platform?: string;
  /** 写入授权并启用；失败时抛出宿主的 `{code, detail}`，对话框留着并说明原因。 */
  onConfirm: (granted: ScriptCapability[]) => Promise<void>;
  onCancel: () => void;
}

/** 授权对话框：清单申请的每项能力一个复选框，依赖联动，「修改输入的文字」和「读取正在输入的内容」互斥。确认后启用插件。 */
export function ScriptGrantDialog({
  pack,
  initial,
  stale,
  platform,
  onConfirm,
  onCancel,
}: ScriptGrantDialogProps) {
  const requested = sortCapabilities(pack.permissions ?? []);
  const [granted, setGranted] = useState<ScriptCapability[]>(() =>
    sortCapabilities(initial.filter((item) => requested.includes(item))),
  );
  // 现在列着、勾着的那几项。包在对话框打开之后被换过（`plugin_script_changed`，页面重读后 `pack` 换成新版本）时，`granted` 里可能还留着新版本不再申请的一项：它不显示，不能算作勾着去禁用互斥的另一项，也不能被交出去、被宿主以「清单没有申请」拒绝。
  const selected = granted.filter((item) => requested.includes(item));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const titleId = useId();
  const cancelButton = useRef<HTMLButtonElement>(null);
  const risk = scriptResidualRisk(platform);
  // 从「取消」开始：对话框是从开关打开的，按住不放的空格或回车不该直接把读写文字的权限授出去。
  useEffect(() => {
    cancelButton.current?.focus();
  }, []);
  const items = requested.map((capability) => {
    const partner = exclusivePartner(capability);
    const blockedBy = partner && selected.includes(partner) ? partner : null;
    const required = CAPABILITY_REQUIRES[capability];
    const notes = [
      required ? `需要同时授予「${capabilityLabels[required].title}」。` : null,
      blockedBy ? `不能和「${capabilityLabels[blockedBy].title}」同时授予。` : null,
    ].filter(Boolean);
    return {
      value: capability,
      checked: selected.includes(capability),
      disabled: busy || blockedBy !== null,
      label: (
        <span className="flex flex-col">
          <span>{capabilityLabels[capability].title}</span>
          <span className="text-xs text-secondary">
            {capabilityLabels[capability].description}
            {notes.join("")}
          </span>
        </span>
      ),
    };
  });
  const confirm = async () => {
    setBusy(true);
    setError("");
    try {
      await onConfirm(selected);
    } catch (failure) {
      setError(pluginErrorMessage(failure, "授权没有写入，请重试。"));
      setBusy(false);
    }
  };
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
            已更新，需要重新授权：这个插件的内容和你上次授权时不同，之前的授权已经作废。
          </p>
        )}
        <p className="m-0 leading-relaxed text-secondary">
          脚本插件是第三方代码，会在输入法里运行。只勾选你愿意让它做的事；没有勾选的，它一律做不到。在密码框和隐私输入框里它不会运行。
        </p>
        {requested.length > 0 ? (
          <Checks
            legend="允许这个插件"
            items={items}
            onChange={(capability, checked) =>
              setGranted((current) => toggleCapability(current, capability, checked, requested))
            }
          />
        ) : (
          <p className="m-0 leading-relaxed text-secondary">
            这个插件没有申请任何权限：它只能知道发生了什么事件，读不到也改不了任何文字。
          </p>
        )}
        {risk && <p className={dialogStyle.warning}>{risk}</p>}
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
            授权并启用
          </button>
        </div>
      </div>
    </div>
  );
}

export interface ScriptPluginDetailViewProps {
  pack: PluginPackage;
  grants: ScriptGrantsResult | null;
  /** 宿主会运行的插件 id，按执行顺序（`runningScriptIds`）。 */
  running: readonly string[];
  platform?: string;
  working: boolean;
  /** 写入授权并启用；失败时抛出，由授权对话框说明原因。 */
  onGrant: (granted: ScriptCapability[]) => Promise<void>;
  /** 停用并撤销全部授权。 */
  onRevoke: () => void;
  /** 在会运行的插件里往前（-1）或往后（1）挪一位。 */
  onMove: (offset: -1 | 1) => void;
  onRemove: (pack: PluginPackage) => void;
  onBack: () => void;
}

/** 一个脚本插件：它在什么时候运行、申请了什么、授予了什么、输入法有没有停用它、排第几个执行，以及启用（经授权对话框）、调整授权和删除。 */
export function ScriptPluginDetailView({
  pack,
  grants,
  running,
  platform,
  working,
  onGrant,
  onRevoke,
  onMove,
  onRemove,
  onBack,
}: ScriptPluginDetailViewProps) {
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
  const stateText: Record<ScriptGrantState, string> = {
    ungranted: "还没有授权，不会运行。",
    stale: "已更新，需要重新授权：插件的内容和授权时不同，之前的授权已经作废，不会运行。",
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
          <SettingsGroupNote>
            没有申请任何权限：只能知道发生了什么事件，读不到也改不了文字。
          </SettingsGroupNote>
        ) : (
          requested.map((capability) => (
            <SummaryRow
              key={capability}
              title={capabilityLabels[capability].title}
              description={capabilityLabels[capability].description}
            >
              {granted.includes(capability) ? "已授予" : "未授予"}
            </SummaryRow>
          ))
        )}
      </GroupList>
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
            else onRevoke();
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
      {state === "enabled" && (
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
      <GroupList title="安全">
        <SettingsGroupNote>
          脚本插件在输入法里的沙箱中运行，只能做你授予的事，读不到文件和网络，也不会在密码框里运行。
        </SettingsGroupNote>
        {risk && <SettingsGroupNote>{risk}</SettingsGroupNote>}
      </GroupList>
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
      {dialog && (
        <ScriptGrantDialog
          pack={pack}
          initial={granted}
          stale={state === "stale"}
          platform={platform}
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
