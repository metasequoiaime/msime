import type { EffectStyle, PluginPreferences } from "./plugin-preferences";
import type { PluginKind, PluginPackage } from "./plugin-types";

/** 各类插件统一的显示名。设置页、社区页和管理后台、官网都用这一套；旋律音效包另写作 `melodyKindLabel`。 */
export const kindLabels: Record<PluginKind, string> = {
  sound: "音效包",
  music: "音乐包",
  command_table: "指令表",
  effect: "特效包",
  phrase_table: "短语表",
  helpcode: "辅助码表",
  wordbook: "单词本",
  symbol_set: "符号集",
};

/** 按键旋律（`mode = "sequence"` 的音效包）的类型显示名。 */
export const melodyKindLabel = "音效包·旋律";

/** 一个包的类型显示名：旋律音效包显示为「音效包·旋律」，其余用 `kindLabels`。 */
export function packKindLabel(pack: Pick<PluginPackage, "kind" | "mode">): string {
  return pack.kind === "sound" && pack.mode === "sequence"
    ? melodyKindLabel
    : kindLabels[pack.kind];
}

/** The order the 我的插件 list groups the installed packs in. */
export const kindOrder: readonly PluginKind[] = [
  "sound",
  "effect",
  "music",
  "command_table",
  "phrase_table",
  "helpcode",
  "wordbook",
  "symbol_set",
];

export const effectStyleOptions: readonly { value: EffectStyle; label: string }[] = [
  { value: "off", label: "关闭" },
  { value: "flash", label: "闪光" },
  { value: "sparks", label: "火花" },
  { value: "power_mode", label: "Power Mode" },
];

export function effectStyleLabel(style: EffectStyle): string {
  return effectStyleOptions.find((option) => option.value === style)?.label ?? style;
}

/** The sentence for a failed host call. The desktop shell's commands and the HarmonyOS bridge both reject with `{code, detail}`, the codes being client-core's `PluginFailure`; `detail`, when present, is the rule client-core reports. */
export function pluginErrorMessage(error: unknown, fallback: string): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : "";
  const detail =
    typeof error === "object" && error !== null && "detail" in error
      ? (error as { detail: unknown }).detail
      : null;
  const reason = typeof detail === "string" && detail ? `（${detail}）` : "";
  switch (code) {
    case "plugin_invalid":
      return `插件不符合要求${reason}。`;
    case "plugin_archive":
      return `压缩包无法读取${reason}。`;
    case "plugin_unsupported_source":
      return "只能导入插件文件夹或 .zip 文件。";
    case "plugin_reserved":
      return "这个 id 属于内置插件，不能覆盖或删除。";
    case "plugin_storage":
      return "插件目录不可用，请检查数据目录的权限。";
    case "mention_invalid":
      return `名单有误${reason}。`;
    case "mention_format":
      return "名单文件无法读取；保存会用新的名单替换它。";
    case "mention_storage":
      return "名单文件不可用，请检查数据目录的权限。";
    default:
      return fallback;
  }
}

export function packLabel(pack: PluginPackage): string {
  return pack.builtin ? `${pack.name}（内置）` : pack.name;
}

/** 短语表详情最多预览的行数。 */
export const PHRASE_PREVIEW_ROWS = 20;

export function commandSummary(pack: PluginPackage): string {
  const commands = pack.commands ?? [];
  const shown = commands
    .slice(0, 3)
    .map((command) => `/${command.trigger} ${command.title}`)
    .join("、");
  return `${commands.length} 条指令${shown ? `：${shown}${commands.length > 3 ? " 等" : ""}` : ""}`;
}

/** Which sound-pack mode a sound selection can play: `key_sound.pack` names a pack of key samples, `melody.pack` a sequence. A pack without a mode is a key pack, as the host reads it. */
export type SoundUse = "keys" | "sequence";

export function soundPackUse(pack: PluginPackage): SoundUse {
  return pack.mode === "sequence" ? "sequence" : "keys";
}

/** The installed pack a selection names: the same kind and id, and for a sound selection the mode it is played in, so a melody named as the key-sound pack (or the reverse) is not taken for a usable one. */
export function selectedPack(
  packages: readonly PluginPackage[],
  kind: PluginKind,
  id: string,
  soundUse?: SoundUse,
): PluginPackage | undefined {
  return packages.find(
    (pack) =>
      pack.kind === kind &&
      pack.id === id &&
      (kind !== "sound" || soundUse === undefined || soundPackUse(pack) === soundUse),
  );
}

/** What the preferences currently do with a pack, as the list marks it: 使用中 for the selected sound, effect or music pack, 当前旋律 for the selected melody, 已启用 with its priority for an enabled command table; null when the pack is not in use or the host does not act on its kind (`kinds`, the same set the missing selections are limited to). */
export function packMarker(
  pack: PluginPackage,
  preferences: PluginPreferences,
  kinds: ReadonlySet<PluginKind>,
): string | null {
  if (!kinds.has(pack.kind)) return null;
  switch (pack.kind) {
    case "sound":
      if (soundPackUse(pack) === "sequence")
        return preferences.melody.pack === pack.id ? "当前旋律" : null;
      return preferences.key_sound.pack === pack.id ? "使用中" : null;
    case "effect":
      return preferences.effect_pack === pack.id ? "使用中" : null;
    case "music":
      return preferences.music.pack === pack.id ? "使用中" : null;
    case "command_table": {
      const position = preferences.command_tables.indexOf(pack.id);
      return position >= 0 ? `已启用 · 第 ${position + 1} 位` : null;
    }
    case "phrase_table": {
      const position = preferences.phrase_tables.indexOf(pack.id);
      return position >= 0 ? `已启用 · 第 ${position + 1} 位` : null;
    }
    case "helpcode":
      return helpcodePackUses(preferences, pack.id);
    // 单词本没有偏好：选中哪本书在背单词里；符号集装上就显示。
    case "wordbook":
    case "symbol_set":
      return null;
  }
}

/** 单词本插件在背单词里的词书 id：`client-core::plugins::wordbook_pack::book_id`。 */
export function wordbookPackBookId(id: string): string {
  return `pack-${id}`;
}

/** 单词本详情预览的单词数。 */
export const WORDBOOK_PREVIEW_WORDS = 5;

/** 符号集详情每组预览的项数。 */
export const SYMBOL_PREVIEW_ITEMS = 16;

/** 辅助码表包用在哪个方案上：「用于全拼」「用于双拼」「用于全拼和双拼」，都没用时为 null。 */
export function helpcodePackUses(preferences: PluginPreferences, id: string): string | null {
  const schemes = [
    preferences.helpcode_pack_quanpin === id ? "全拼" : null,
    preferences.helpcode_pack_shuangpin === id ? "双拼" : null,
  ].filter(Boolean);
  return schemes.length > 0 ? `用于${schemes.join("和")}` : null;
}

/** A selection naming a pack the catalog does not have, and what it is selected as. `mismatched` is set when a sound pack by that id is installed but in the other mode (a melody named as the key-sound pack, or the reverse), so it cannot be played the way it is selected. */
export type MissingSelection = {
  kind: PluginKind;
  id: string;
  uses: string[];
  mismatched?: boolean;
};

/** The selections that name a pack no longer installed, or a sound pack of the wrong mode, one entry per pack, in list order. Only meaningful once a catalog has been read from a host with a pack store; `kinds` limits it to the selections the host acts on. */
export function missingSelections(
  preferences: PluginPreferences,
  packages: readonly PluginPackage[],
  kinds: ReadonlySet<PluginKind>,
): MissingSelection[] {
  const missing: MissingSelection[] = [];
  const add = (kind: PluginKind, id: string, use: string, soundUse?: SoundUse) => {
    if (!id || !kinds.has(kind)) return;
    if (selectedPack(packages, kind, id, soundUse)) return;
    const mismatched = packages.some((pack) => pack.kind === kind && pack.id === id);
    const existing = missing.find((entry) => entry.kind === kind && entry.id === id);
    if (existing) existing.uses.push(use);
    else missing.push({ kind, id, uses: [use], ...(mismatched ? { mismatched } : {}) });
  };
  add("sound", preferences.key_sound.pack, "当前音效包", "keys");
  add("sound", preferences.melody.pack, "当前旋律", "sequence");
  add("effect", preferences.effect_pack, "当前特效包");
  add("music", preferences.music.pack, "当前音乐包");
  for (const id of preferences.command_tables) add("command_table", id, "已启用的指令表");
  for (const id of preferences.phrase_tables) add("phrase_table", id, "已启用的短语表");
  add("helpcode", preferences.helpcode_pack_quanpin, "全拼辅助码");
  add("helpcode", preferences.helpcode_pack_shuangpin, "双拼辅助码");
  return missing;
}

/** The name a missing selection is listed and headed under: 未找到 for a pack that is gone, 不可用 for a sound pack that is installed in the other mode. */
export function missingTitle(entry: MissingSelection): string {
  return `${entry.id}${entry.mismatched ? "（不可用）" : "（未找到）"}`;
}

/** Why a missing selection cannot be used, after what it is selected as. */
export function missingReason(entry: MissingSelection): string {
  if (!entry.mismatched) return "已不在本机";
  return entry.uses.includes("当前音效包")
    ? "但它是按键旋律，不能用作按键音效"
    : "但它不是按键旋律";
}
