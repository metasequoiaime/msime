import type { EffectStyle } from "./plugin-preferences";

/** Mirrors `client-core::plugins::PluginKind`. */
export type PluginKind = "sound" | "music" | "command_table" | "effect";

export type PluginCommand = { trigger: string; title: string; template: string };

/** One pack as `client-core::plugins::scan` lists it: the manifest's fields, with the content of its kind flattened in. */
export type PluginPackage = {
  id: string;
  name: string;
  version: string;
  license: string;
  author: string | null;
  description: string | null;
  /** Shipped in the bundle rather than installed; cannot be removed. */
  builtin: boolean;
  kind: PluginKind;
  /** Sound packs: a sample per key class, or one sample a melody is played on. */
  mode?: "keys" | "sequence";
  /** Music packs. */
  tracks?: string[];
  /** Command tables. */
  commands?: PluginCommand[];
  /** Effect packs: `client-core::plugins::effect_pack::EffectPack`, never `off`. */
  style?: EffectStyle;
  intensity?: number;
  colors?: string[];
  duration_ms?: number | null;
  particles?: number | null;
};

/** A folder under the plugins directory that is not a loadable pack, and why. */
export type PluginIssue = { kind: PluginKind; folder: string; reason: string };

export type PluginCatalogResult = { packages: PluginPackage[]; issues: PluginIssue[] };

/** One entry of the @ mode's name list, `client-core::plugins::mentions::MentionEntry`. */
export type MentionEntry = { text: string; key: string };

/**
 * The host side of the 插件 page. The host resolves every directory itself: the page reads the catalog, asks the host to show its own picker for an import, and names a pack to remove by kind and id, never by path.
 */
export interface PluginClient {
  catalog(): Promise<PluginCatalogResult>;
  /** Shows the platform's picker for a pack folder or a `.zip` file and installs what was picked; null when the picker was closed. */
  importPack(source: "folder" | "archive"): Promise<PluginPackage | null>;
  remove(kind: PluginKind, id: string): Promise<void>;
  loadMentions(): Promise<MentionEntry[]>;
  saveMentions(entries: MentionEntry[]): Promise<void>;
}
