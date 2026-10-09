import type { Preferences } from "../index";
import type { PluginKind } from "./plugin-types";

/** Mirrors `client-core::preferences::KeySoundMode`. */
export type KeySoundMode = "keys" | "melody";

/** Mirrors `client-core::preferences::KeySoundPreferences`. */
export type KeySoundPreferences = {
  enabled: boolean;
  mode: KeySoundMode;
  /** The sound pack keys, commits and achievements are played from. */
  pack: string;
  /** 0-100, for every effect sound: keys, the melody, commits and achievements. */
  volume: number;
};

/** Mirrors `client-core::preferences::CommitSoundPreferences`. */
export type CommitSoundPreferences = { enabled: boolean };

/** Mirrors `client-core::preferences::MelodyPreferences`. */
export type MelodyPreferences = { pack: string };

/** Mirrors `client-core::preferences::MusicPreferences`. */
export type MusicPreferences = { enabled: boolean; pack: string; volume: number };

/** Mirrors `client-core::preferences::AchievementPreferences`. */
export type AchievementPreferences = { enabled: boolean };

/** Mirrors `client-core::plugins::EffectStyle`: what a host draws on keys and commits. */
export type EffectStyle = "off" | "flash" | "sparks" | "power_mode";

/** Mirrors `client-core::preferences::PluginPreferences`. The document leaves the whole section out while it is at these defaults, so a page reading one merges it over `defaultPluginPreferences`. */
export type PluginPreferences = {
  key_sound: KeySoundPreferences;
  commit_sound: CommitSoundPreferences;
  melody: MelodyPreferences;
  music: MusicPreferences;
  achievements: AchievementPreferences;
  /** Installed command-table packs the `/` mode reads, in priority order: the first pack that defines a trigger wins. */
  command_tables: string[];
  /** The typing effect the host draws; `off` draws nothing. */
  effect_style: EffectStyle;
  /** 0-100: how large and how long the effect is drawn. */
  effect_intensity: number;
  /** An installed effect pack whose style and parameters replace `effect_style` and `effect_intensity`; empty for none. */
  effect_pack: string;
  /** Count consecutive keys and show the count; a pause of 3 seconds or a backspace starts it again. */
  combo_counter: boolean;
  /** Play the key sound pack's commit sample, pitched up, when the count reaches 10, 25, 50 and 100. */
  combo_tier_sound: boolean;
  /** K 模式读取的已安装短语表包，按优先级排列；为空时文档里不写这个键。 */
  phrase_tables: string[];
  /** 全拼方案选用的已安装辅助码表包；为空表示沿用 `quanpin_helpcode.schema`。 */
  helpcode_pack_quanpin: string;
  /** 双拼方案选用的已安装辅助码表包；为空表示沿用 `shuangpin_helpcode.schema`。 */
  helpcode_pack_shuangpin: string;
};

/** `client-core::plugins::DEFAULT_SOUND_PACK`. */
export const DEFAULT_SOUND_PACK = "default";
/** `client-core::plugins::DEFAULT_MELODY_PACK`. */
export const DEFAULT_MELODY_PACK = "twinkle";
/** `PluginPreferences::MAX_COMMAND_TABLES`. */
export const MAX_COMMAND_TABLES = 16;
/** `PluginPreferences::MAX_PHRASE_TABLES`。 */
export const MAX_PHRASE_TABLES = 16;

export const defaultPluginPreferences: PluginPreferences = {
  key_sound: { enabled: false, mode: "keys", pack: DEFAULT_SOUND_PACK, volume: 50 },
  commit_sound: { enabled: false },
  melody: { pack: DEFAULT_MELODY_PACK },
  music: { enabled: false, pack: "", volume: 30 },
  achievements: { enabled: false },
  command_tables: [],
  effect_style: "off",
  effect_intensity: 50,
  effect_pack: "",
  combo_counter: false,
  combo_tier_sound: false,
  phrase_tables: [],
  helpcode_pack_quanpin: "",
  helpcode_pack_shuangpin: "",
};

/** The plugin section a draft holds, each part filled in from the defaults where the document left it out. */
export function pluginPreferences(draft?: Pick<Preferences, "plugins">): PluginPreferences {
  const value = draft?.plugins;
  return {
    key_sound: { ...defaultPluginPreferences.key_sound, ...value?.key_sound },
    commit_sound: { ...defaultPluginPreferences.commit_sound, ...value?.commit_sound },
    melody: { ...defaultPluginPreferences.melody, ...value?.melody },
    music: { ...defaultPluginPreferences.music, ...value?.music },
    achievements: { ...defaultPluginPreferences.achievements, ...value?.achievements },
    command_tables: value?.command_tables ?? defaultPluginPreferences.command_tables,
    effect_style: value?.effect_style ?? defaultPluginPreferences.effect_style,
    effect_intensity: value?.effect_intensity ?? defaultPluginPreferences.effect_intensity,
    effect_pack: value?.effect_pack ?? defaultPluginPreferences.effect_pack,
    combo_counter: value?.combo_counter ?? defaultPluginPreferences.combo_counter,
    combo_tier_sound: value?.combo_tier_sound ?? defaultPluginPreferences.combo_tier_sound,
    phrase_tables: value?.phrase_tables ?? defaultPluginPreferences.phrase_tables,
    helpcode_pack_quanpin:
      value?.helpcode_pack_quanpin ?? defaultPluginPreferences.helpcode_pack_quanpin,
    helpcode_pack_shuangpin:
      value?.helpcode_pack_shuangpin ?? defaultPluginPreferences.helpcode_pack_shuangpin,
  };
}

/**
 * 包从磁盘删除之后的设置：选中它的选择回到新档案的默认值（特效包回到无），启用它的指令表或短语表被移除，文档不会指向一个已经不在的包。
 */
export function withoutRemovedPack(
  preferences: PluginPreferences,
  kind: PluginKind,
  id: string,
): PluginPreferences {
  if (kind === "phrase_table") {
    return {
      ...preferences,
      phrase_tables: preferences.phrase_tables.filter((table) => table !== id),
    };
  }
  // 单词本和符号集没有偏好键；单词本的复习进度留在背单词里。
  if (kind === "wordbook" || kind === "symbol_set") return preferences;
  if (kind === "helpcode") {
    return {
      ...preferences,
      helpcode_pack_quanpin:
        preferences.helpcode_pack_quanpin === id ? "" : preferences.helpcode_pack_quanpin,
      helpcode_pack_shuangpin:
        preferences.helpcode_pack_shuangpin === id ? "" : preferences.helpcode_pack_shuangpin,
    };
  }
  if (kind === "effect") {
    return preferences.effect_pack === id ? { ...preferences, effect_pack: "" } : preferences;
  }
  if (kind === "command_table") {
    return {
      ...preferences,
      command_tables: preferences.command_tables.filter((table) => table !== id),
    };
  }
  if (kind === "music") {
    return preferences.music.pack === id
      ? { ...preferences, music: { ...preferences.music, enabled: false, pack: "" } }
      : preferences;
  }
  return {
    ...preferences,
    key_sound:
      preferences.key_sound.pack === id
        ? { ...preferences.key_sound, pack: DEFAULT_SOUND_PACK }
        : preferences.key_sound,
    melody: preferences.melody.pack === id ? { pack: DEFAULT_MELODY_PACK } : preferences.melody,
  };
}

/** 把一个包设为它所属类型的当前选择：选用声音或音乐包就同时打开对应的播放开关，声音包还切到能播放所选包的发声方式；特效包成为特效，指令表和短语表启用在已启用的之后。 */
export function withPackSelected(
  preferences: PluginPreferences,
  pack: {
    kind: PluginKind;
    id: string;
    mode?: "keys" | "sequence";
  },
): PluginPreferences {
  switch (pack.kind) {
    case "sound":
      return pack.mode === "sequence"
        ? {
            ...preferences,
            key_sound: { ...preferences.key_sound, enabled: true, mode: "melody" },
            melody: { pack: pack.id },
          }
        : {
            ...preferences,
            key_sound: { ...preferences.key_sound, enabled: true, mode: "keys", pack: pack.id },
          };
    case "effect":
      return { ...preferences, effect_pack: pack.id };
    case "music":
      return { ...preferences, music: { ...preferences.music, enabled: true, pack: pack.id } };
    case "command_table":
      return preferences.command_tables.includes(pack.id)
        ? preferences
        : { ...preferences, command_tables: [...preferences.command_tables, pack.id] };
    case "phrase_table":
      return preferences.phrase_tables.includes(pack.id)
        ? preferences
        : { ...preferences, phrase_tables: [...preferences.phrase_tables, pack.id] };
    // 单词本在背单词里选，符号集装上就显示，都不在偏好里。
    case "wordbook":
    case "symbol_set":
      return preferences;
    // 辅助码表包可以分别用于全拼和双拼；整体选中时两个方案都用它。
    case "helpcode":
      return {
        ...preferences,
        helpcode_pack_quanpin: pack.id,
        helpcode_pack_shuangpin: pack.id,
      };
  }
}
