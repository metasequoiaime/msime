import type {
  PluginPackage,
  ScriptCapability,
  ScriptCommand,
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
  "provide_commands",
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
  // 授权文案和 Windows 原生页（`PluginPageModel.h` 的 `capability_label`）、`docs/plugins.md` 一字不差。回车在 `/` 指令里各桌面宿主都发 `MSIME_COMMIT_RAW`，上屏的是原文；失焦时 macOS 上屏原文，Windows 和 Linux 丢弃组字，都不上屏插件行。快捷键各宿主不一致：macOS 的 Cmd、Ctrl、Option 组合键和候选窗不可见时的 Tab、Fcitx5 没被前面处理的非字符键发 `MSIME_FINISH_COMPOSITION`，上屏高亮行，Windows Server 的 `translate_key` 对组合键发 `MSIME_CANCEL`，所以写「在有的系统上」。
  provide_commands: {
    title: "提供 / 指令",
    description:
      "你在 / 指令里输入它声明的触发词（例如 /uuid）之后打的字母会交给这个插件，它据此给出候选；给出的候选只取决于这些字母和随机数，带不走你之前打过的内容。你按序号、点选、按空格选中某一行，或者打标点结束输入时，高亮的那一行会输入；在有的系统上，按快捷键或 Tab 等键、用菜单或工具栏结束输入时也会输入这一行。按回车时只输入你打的原文（例如 /uuid），点到别的窗口时插件给出的行不会输入。它看不到你平时打的字、其他指令和别的插件的内容；它给出的文字不进入词库、不计入打字统计、不会作为上文发给 AI 候选，也不会拿去查释义或翻译。",
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
  command: "你在 / 指令里输入它的触发词时（给出候选）",
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

export function sortCapabilities(capabilities: Iterable<ScriptCapability>): ScriptCapability[] {
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
      return scriptStaleLabel(pack);
    case "disabled":
      return "未启用";
    case "enabled": {
      if (health) return "已被输入法停用";
      const position = running.indexOf(pack.id);
      return position < 0 ? "已启用 · 超过 8 个，不运行" : `已启用 · 第 ${position + 1} 位`;
    }
  }
}

/** 脚本插件在 `/` 指令里声明的指令（清单 `[[script.commands]]`），按清单顺序；不提供指令的插件为空。 */
export function scriptCommands(pack: PluginPackage): ScriptCommand[] {
  return pack.kind === "script" ? ((pack.commands ?? []) as ScriptCommand[]) : [];
}

/** 随输入法分发的官方脚本插件（`builtin` 为真）详情里「来源」一行：它不能删除，和导入的插件一样默认不运行。Windows 原生设置应用（`PluginPageModel.h`）用同一句。 */
export const SCRIPT_BUILTIN_NOTE = "随输入法安装和更新，不能删除；需要你授权后才会运行。";

/** 授权之后包的内容变了时的标记。内置插件的内容只会随输入法更新而变，说法跟着改。 */
export function scriptStaleLabel(pack: Pick<PluginPackage, "builtin">): string {
  return pack.builtin ? "已随输入法更新，需要重新授权" : "已更新，需要重新授权";
}

/** / 指令关着时，提供指令的插件详情里的说明，后面跟着「打开 / 指令」按钮。授权本身从不改这个开关。 */
export const SCRIPT_COMMAND_MODE_NOTE =
  "需要先打开 / 指令，并使用中文标点；在全拼、双拼、五笔下没有输入时按 / 再输入触发词。";

/**
 * 一条插件指令现在的状态，白话：没启用或没授予「提供 / 指令」时说要怎样才生效；输入法停用了插件时照实说；否则按宿主算出的状态（`ScriptGrantsResult.commands`）说生效，或者和谁冲突、没生效。`nameOf` 把冲突的插件 id 换成它的名字。
 */
export function scriptCommandStatus(
  pack: PluginPackage,
  trigger: string,
  grants: ScriptGrantsResult | null,
  nameOf: (id: string) => string = (id) => id,
): string {
  const grant = grants?.plugins.find((entry) => entry.id === pack.id);
  if (scriptGrantState(pack, grant) !== "enabled") return "授权并启用后生效";
  if (!grant?.granted.includes("provide_commands")) return "没有授予「提供 / 指令」，不会生效";
  if (scriptHealth(grants, pack)) return "插件已被输入法停用，不会给出候选";
  const resolved = grants?.commands.find(
    (entry) => entry.id === pack.id && entry.trigger === trigger,
  );
  switch (resolved?.status) {
    case "active":
      return "生效";
    case "shadowed_by_user_table":
      return `与你的指令 /${resolved.conflict} 冲突，未生效`;
    case "shadowed_by_plugin":
      return `与插件 ${nameOf(resolved.conflict_id)} 的 /${resolved.conflict} 冲突，未生效`;
    case undefined:
      return "未生效";
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
