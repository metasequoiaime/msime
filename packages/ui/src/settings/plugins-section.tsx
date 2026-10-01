import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import * as settings from "./settings-style";
import { GroupList, Row, Segmented, Select, Slider, Switch } from "../core/platform-controls";
import type { ConfirmRequest } from "../core/confirm";
import {
  MAX_COMMAND_TABLES,
  withoutRemovedPack,
  type EffectStyle,
  type KeySoundMode,
  type PluginPreferences,
} from "./plugin-preferences";

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
 * The host side of the 扩展 page. The host resolves every directory itself: the page reads the catalog, asks the host to show its own picker for an import, and names a pack to remove by kind and id, never by path.
 */
export interface PluginClient {
  catalog(): Promise<PluginCatalogResult>;
  /** Shows the platform's picker for a pack folder or a `.zip` file and installs what was picked; null when the picker was closed. */
  importPack(source: "folder" | "archive"): Promise<PluginPackage | null>;
  remove(kind: PluginKind, id: string): Promise<void>;
  loadMentions(): Promise<MentionEntry[]>;
  saveMentions(entries: MentionEntry[]): Promise<void>;
}

/** `client-core::plugins::mentions::MAX_ENTRIES`. */
export const MAX_MENTIONS = 1000;
/** `mentions::MAX_TEXT_UTF16`: the Windows candidate pipe's text field. */
const MAX_MENTION_TEXT_UTF16 = 199;
/** `mentions::MAX_KEY_BYTES`. */
const MAX_MENTION_KEY_BYTES = 64;
const MENTION_KEY = /^[a-z]+(?:'[a-z]+)*$/;

export const kindLabels: Record<PluginKind, string> = {
  sound: "音效包",
  music: "音乐包",
  command_table: "指令表",
  effect: "特效包",
};

const keySoundModes: readonly { value: KeySoundMode; label: string }[] = [
  { value: "keys", label: "按键音效" },
  { value: "melody", label: "按键旋律" },
];

const effectStyleOptions: readonly { value: EffectStyle; label: string }[] = [
  { value: "off", label: "关闭" },
  { value: "flash", label: "闪光" },
  { value: "sparks", label: "火花" },
  { value: "power_mode", label: "Power Mode" },
];

/**
 * Why the list cannot be saved as it stands, in the words the page shows, or null when it can. The rules are `mentions::validate_entry`'s, checked here so the user sees which row to fix before anything is sent; the host checks again.
 */
export function mentionListIssue(entries: readonly MentionEntry[]): string | null {
  if (entries.length > MAX_MENTIONS) return `名单最多 ${MAX_MENTIONS} 条。`;
  const seen = new Set<string>();
  for (const [index, entry] of entries.entries()) {
    const row = `第 ${index + 1} 行`;
    if (entry.text.trim().length === 0) return `${row}还没有填写名字或地点。`;
    // UTF-16 code units, which is what String.length counts.
    if (entry.text.length > MAX_MENTION_TEXT_UTF16) return `${row}太长了。`;
    if (/\p{Cc}/u.test(entry.text)) return `${row}含有控制字符。`;
    if (
      entry.key.length > 0 &&
      (entry.key.length > MAX_MENTION_KEY_BYTES || !MENTION_KEY.test(entry.key))
    ) {
      return `${row}的拼音只能是小写字母，音节之间用 ' 分隔，例如 zhang'san。`;
    }
    // Compared as saved: `saveMentions` trims each name before the host checks for duplicates.
    const text = entry.text.trim();
    if (seen.has(text)) return `「${text}」重复了。`;
    seen.add(text);
  }
  return null;
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
      return `扩展包不符合要求${reason}。`;
    case "plugin_archive":
      return `压缩包无法读取${reason}。`;
    case "plugin_unsupported_source":
      return "只能导入扩展包文件夹或 .zip 文件。";
    case "plugin_reserved":
      return "这个 id 属于内置扩展包，不能覆盖或删除。";
    case "plugin_storage":
      return "扩展目录不可用，请检查数据目录的权限。";
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

function packLabel(pack: PluginPackage): string {
  return pack.builtin ? `${pack.name}（内置）` : pack.name;
}

/** A select's options: the packs that fit, plus the selected id when no listed pack has it, so a missing pack reads as missing rather than as another pack. `listed` is false on a host with no pack store, which cannot tell a missing pack from one it simply did not list. */
function packOptions(packs: readonly PluginPackage[], selected: string, listed: boolean) {
  const options = packs.map((pack) => ({ value: pack.id, label: packLabel(pack) }));
  if (selected && !packs.some((pack) => pack.id === selected)) {
    options.push({ value: selected, label: listed ? `${selected}（未找到）` : selected });
  }
  return options;
}

function commandSummary(pack: PluginPackage): string {
  const commands = pack.commands ?? [];
  const shown = commands
    .slice(0, 3)
    .map((command) => `/${command.trigger} ${command.title}`)
    .join("、");
  return `${commands.length} 条指令${shown ? `：${shown}${commands.length > 3 ? " 等" : ""}` : ""}`;
}

export interface PluginsSectionProps {
  /** Absent on a host with no pack store; the sound switches still work with the built-in packs. */
  client?: PluginClient;
  preferences: PluginPreferences;
  /** The host plays sound packs (`HostCapabilities.key_sound`). */
  keySound: boolean;
  /** The host streams music packs (`HostCapabilities.music`). */
  music: boolean;
  /** The host routes the / and @ modes (`HostCapabilities.plugin_triggers`). */
  triggers: boolean;
  /** The host draws the typing effects and the combo count (`HostCapabilities.typing_effects`). */
  typingEffects?: boolean;
  /** The host draws the effect styles, not only the combo count; false on Linux, which shows the count as text. */
  effectStyles?: boolean;
  /** The host draws an installed effect pack's style and parameters (`msime_client_typing_effect_settings`); only read where `effectStyles` is true. */
  effectPacks?: boolean;
  /** Loads the catalog and the name list each time this turns true, so a pack copied in by hand shows up on the next visit. */
  active: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onError: (message: string) => void;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
}

/** The 扩展 page: key sounds, the typing melody, typing effects, background music, command tables, the installed packs and the @ name list. */
export function PluginsSection({
  client,
  preferences,
  keySound,
  music,
  triggers,
  typingEffects = false,
  effectStyles = false,
  effectPacks = false,
  active,
  onChange,
  onError,
  confirm,
}: PluginsSectionProps) {
  const [catalog, setCatalog] = useState<PluginCatalogResult>({ packages: [], issues: [] });
  // Until a catalog has been read, an empty one says nothing about what is installed: no pack is reported missing.
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [working, setWorking] = useState(false);
  const [notice, setNotice] = useState("");
  const [mentions, setMentions] = useState<MentionEntry[]>([]);
  const [savedMentions, setSavedMentions] = useState<MentionEntry[]>([]);
  const mentionsEditable = triggers && Boolean(client);
  const actionRunning = useRef(false);
  const clientGeneration = useRef(0);
  // The page stays mounted while hidden, so a reload on the next visit must not overwrite edits that were never saved.
  const mentionsDirtyRef = useRef(false);

  useEffect(() => {
    if (!active || !client) return;
    const generation = ++clientGeneration.current;
    let current = true;
    void client
      .catalog()
      .then((next) => {
        if (!current) return;
        setCatalog(next);
        setCatalogLoaded(true);
      })
      .catch((error: unknown) => {
        if (current) onError(pluginErrorMessage(error, "无法读取扩展包列表，请重试。"));
      });
    if (mentionsEditable) {
      void client
        .loadMentions()
        .then((next) => {
          if (!current) return;
          if (!mentionsDirtyRef.current) setMentions(next);
          setSavedMentions(next);
        })
        .catch((error: unknown) => {
          if (current) onError(pluginErrorMessage(error, "无法读取 @ 名单，请重试。"));
        });
    }
    return () => {
      current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
    // `onError` is the page's setter and stable; the effect reloads on a visit, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, client, mentionsEditable]);

  const packs = (kind: PluginKind) => catalog.packages.filter((pack) => pack.kind === kind);
  const keyPacks = packs("sound").filter((pack) => pack.mode !== "sequence");
  const melodyPacks = packs("sound").filter((pack) => pack.mode === "sequence");
  const musicPacks = packs("music");
  const commandPacks = packs("command_table");
  const effectPackList = packs("effect");
  // A selected pack replaces the style and intensity, so those two controls only describe what is drawn while no pack is selected.
  const effectPackSelected = effectPacks && preferences.effect_pack !== "";
  const { key_sound, commit_sound, melody, achievements, command_tables } = preferences;

  // After an import or removal that already succeeded: a failed reread is reported as such, not as a failed import or removal.
  const refresh = async () => {
    if (!client) return;
    try {
      setCatalog(await client.catalog());
      setCatalogLoaded(true);
    } catch (error) {
      onError(pluginErrorMessage(error, "无法读取扩展包列表，请重试。"));
    }
  };

  async function runPluginAction(
    operation: (isCurrent: () => boolean) => Promise<void>,
    fallback: string,
  ) {
    if (!client || actionRunning.current) return;
    actionRunning.current = true;
    const generation = clientGeneration.current;
    try {
      await runAsyncAction(
        {
          busy: false,
          isCurrent: () => generation === clientGeneration.current,
          setBusy: setWorking,
          setError: onError,
          setNotice,
        },
        operation,
        { formatError: (error) => pluginErrorMessage(error, fallback) },
      );
    } finally {
      actionRunning.current = false;
    }
  }

  const importPack = async (source: "folder" | "archive") => {
    await runPluginAction(async () => {
      const imported = await client!.importPack(source);
      if (imported) {
        setNotice(`已导入${kindLabels[imported.kind]}「${imported.name}」${imported.version}。`);
        await refresh();
      }
    }, "导入失败，请重试。");
  };

  const removePack = async (pack: PluginPackage) => {
    const confirmed = await confirm({
      title: `删除${kindLabels[pack.kind]}`,
      message: `删除「${pack.name}」？包里的文件会从本机移除。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed) return;
    await runPluginAction(async () => {
      await client!.remove(pack.kind, pack.id);
      onChange(withoutRemovedPack(preferences, pack.kind, pack.id));
      await refresh();
    }, "删除失败，请重试。");
  };

  const setCommandTable = (id: string, enabled: boolean) =>
    onChange({
      ...preferences,
      command_tables: enabled
        ? [...command_tables.filter((table) => table !== id), id]
        : command_tables.filter((table) => table !== id),
    });

  const mentionIssue = mentionListIssue(mentions);
  const mentionsDirty = JSON.stringify(mentions) !== JSON.stringify(savedMentions);
  mentionsDirtyRef.current = mentionsDirty;
  const updateMention = (index: number, patch: Partial<MentionEntry>) =>
    setMentions((current) =>
      current.map((entry, position) => (position === index ? { ...entry, ...patch } : entry)),
    );
  const saveMentions = async () => {
    if (mentionIssue) return;
    await runPluginAction(async () => {
      const trimmed = mentions.map((entry) => ({ text: entry.text.trim(), key: entry.key }));
      await client!.saveMentions(trimmed);
      setMentions(trimmed);
      setSavedMentions(trimmed);
    }, "名单未能保存，请重试。");
  };

  // Enabled tables that are no longer installed, listed so they can be switched off.
  const missingTables = catalogLoaded
    ? command_tables.filter((id) => !commandPacks.some((pack) => pack.id === id))
    : [];
  const packsListed = Boolean(client) && catalogLoaded;

  return (
    <>
      {keySound && (
        <GroupList title="按键音效">
          <Row title="按键音" description="打字时按键发声。密码等安全输入框中不发声。">
            <Switch
              checked={key_sound.enabled}
              onChange={(enabled) =>
                onChange({ ...preferences, key_sound: { ...key_sound, enabled } })
              }
            />
          </Row>
          <Row
            title="发声方式"
            description="按键音效按普通键、空格、回车和退格各自发声；按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。"
          >
            <Segmented
              options={keySoundModes}
              value={key_sound.mode}
              disabled={!key_sound.enabled}
              onChange={(mode) => onChange({ ...preferences, key_sound: { ...key_sound, mode } })}
            />
          </Row>
          <Row title="音效包" description="按键、上屏和成就音效都取自这个音效包。">
            <Select
              value={key_sound.pack}
              onChange={(event) =>
                onChange({ ...preferences, key_sound: { ...key_sound, pack: event.target.value } })
              }
            >
              {packOptions(keyPacks, key_sound.pack, packsListed).map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </Select>
          </Row>
          <Row title="旋律" hidden={key_sound.mode !== "melody"}>
            <Select
              value={melody.pack}
              onChange={(event) =>
                onChange({ ...preferences, melody: { pack: event.target.value } })
              }
            >
              {packOptions(melodyPacks, melody.pack, packsListed).map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </Select>
          </Row>
          <Row title="音效音量" description="按键音、旋律、上屏音和成就音效共用。">
            <span className={settings.sliderControl}>
              <Slider
                value={key_sound.volume}
                valueText={`${key_sound.volume}%`}
                onChange={(volume) =>
                  onChange({ ...preferences, key_sound: { ...key_sound, volume } })
                }
              />
            </span>
          </Row>
          <Row title="上屏音" description="文字上屏时播放音效包的上屏音，与按键音各自开关。">
            <Switch
              checked={commit_sound.enabled}
              onChange={(enabled) => onChange({ ...preferences, commit_sound: { enabled } })}
            />
          </Row>
          <Row
            title="成就音效"
            description="上屏字数累计达到 100、1000、1 万等里程碑时播放一段短音。按打字统计计数，需要打开打字统计。"
          >
            <Switch
              checked={achievements.enabled}
              onChange={(enabled) => onChange({ ...preferences, achievements: { enabled } })}
            />
          </Row>
        </GroupList>
      )}
      {typingEffects && (
        <GroupList title="打字效果">
          {effectStyles && (
            <>
              {effectPacks && (
                <Row
                  title="特效包"
                  description={
                    effectPackSelected
                      ? "特效包决定样式、强度、颜色和时长，下面的样式和强度不再生效。"
                      : effectPackList.length === 0
                        ? "还没有导入特效包。"
                        : undefined
                  }
                >
                  <Select
                    value={preferences.effect_pack}
                    onChange={(event) =>
                      onChange({ ...preferences, effect_pack: event.target.value })
                    }
                  >
                    <option value="">不使用</option>
                    {packOptions(effectPackList, preferences.effect_pack, packsListed).map(
                      (option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ),
                    )}
                  </Select>
                </Row>
              )}
              <Row
                title="效果样式"
                description="闪光：按键时候选栏闪一下；火花：按键和上屏时迸出火花；Power Mode：火花随连击变大。"
              >
                <Segmented
                  options={effectStyleOptions}
                  value={preferences.effect_style}
                  disabled={effectPackSelected}
                  onChange={(effect_style) => onChange({ ...preferences, effect_style })}
                />
              </Row>
              <Row title="效果强度" description="效果的大小和持续时间。">
                <span className={settings.sliderControl}>
                  <Slider
                    value={preferences.effect_intensity}
                    valueText={`${preferences.effect_intensity}%`}
                    disabled={effectPackSelected || preferences.effect_style === "off"}
                    onChange={(effect_intensity) => onChange({ ...preferences, effect_intensity })}
                  />
                </span>
              </Row>
            </>
          )}
          <Row
            title="连击计数"
            description="连续打字时在候选栏显示连击数，停顿 3 秒或按退格后重新计数。自动重复的按键不计数。"
          >
            <Switch
              checked={preferences.combo_counter}
              onChange={(combo_counter) => onChange({ ...preferences, combo_counter })}
            />
          </Row>
          {effectStyles && (
            <Row
              title="升档音"
              description="连击达到 10、25、50、100 时播放音效包的上屏音，每升一档音调更高，音量随音效音量。"
            >
              <Switch
                checked={preferences.combo_tier_sound}
                disabled={!preferences.combo_counter}
                onChange={(combo_tier_sound) => onChange({ ...preferences, combo_tier_sound })}
              />
            </Row>
          )}
        </GroupList>
      )}
      {music && (
        <GroupList title="背景音乐">
          <Row
            title="背景音乐"
            description="默认关闭。只在输入法处于活动状态时播放，切换到其他输入法时暂停。"
          >
            <Switch
              checked={preferences.music.enabled}
              onChange={(enabled) =>
                onChange({ ...preferences, music: { ...preferences.music, enabled } })
              }
            />
          </Row>
          <Row
            title="音乐包"
            description={musicPacks.length === 0 ? "还没有导入音乐包。" : undefined}
          >
            <Select
              value={preferences.music.pack}
              onChange={(event) =>
                onChange({
                  ...preferences,
                  music: { ...preferences.music, pack: event.target.value },
                })
              }
            >
              <option value="">未选择</option>
              {packOptions(musicPacks, preferences.music.pack, packsListed).map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </Select>
          </Row>
          <Row title="音乐音量">
            <span className={settings.sliderControl}>
              <Slider
                value={preferences.music.volume}
                valueText={`${preferences.music.volume}%`}
                onChange={(volume) =>
                  onChange({ ...preferences, music: { ...preferences.music, volume } })
                }
              />
            </span>
          </Row>
        </GroupList>
      )}
      {triggers && (
        <GroupList title="指令表">
          <p className={settings.groupNote}>
            在「输入 → 实用功能」打开 / 指令后，按 /
            再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用{" "}
            {MAX_COMMAND_TABLES} 个。
          </p>
          {commandPacks.length === 0 && missingTables.length === 0 && (
            <p className={settings.groupNote}>
              {client
                ? "还没有导入指令表。"
                : "这台设备还不能导入指令表，内置的 rq、sj、xq 指令照常可用。"}
            </p>
          )}
          {commandPacks.map((pack) => {
            const position = command_tables.indexOf(pack.id);
            return (
              <Row
                key={pack.id}
                title={pack.name}
                description={`${position >= 0 ? `第 ${position + 1} 位 · ` : ""}${commandSummary(pack)}`}
              >
                <Switch
                  checked={position >= 0}
                  disabled={position < 0 && command_tables.length >= MAX_COMMAND_TABLES}
                  onChange={(enabled) => setCommandTable(pack.id, enabled)}
                />
              </Row>
            );
          })}
          {missingTables.map((id) => (
            <Row
              key={id}
              title={`${id}（未找到）`}
              description="这个指令表已不在本机，关闭即可移除。"
            >
              <Switch checked onChange={(enabled) => setCommandTable(id, enabled)} />
            </Row>
          ))}
        </GroupList>
      )}
      {client && (
        <GroupList title="扩展包">
          <div className={settings.managerBlock}>
            <p className={settings.managerNote}>
              扩展包只含音频、指令模板等数据，不能带任何可执行内容，每个包都必须声明许可证。可以导入一个包的文件夹，或者打包好的
              .zip 文件。
            </p>
            <div className={settings.managerActions}>
              <button
                type="button"
                className="secondary"
                disabled={working}
                onClick={() => void importPack("folder")}
              >
                导入文件夹
              </button>
              <button
                type="button"
                className="secondary"
                disabled={working}
                onClick={() => void importPack("archive")}
              >
                导入 .zip
              </button>
            </div>
            {notice && (
              <p className={settings.managerNote} role="status">
                {notice}
              </p>
            )}
          </div>
          <div className={settings.clipboardList} aria-label="已安装的扩展包">
            {catalog.packages.length === 0 ? (
              <p className={settings.clipboardEmpty}>没有扩展包</p>
            ) : (
              catalog.packages.map((pack) => (
                <div className={settings.clipboardRow} key={`${pack.kind}/${pack.id}`}>
                  <span className={settings.clipboardEntry}>
                    <span title={pack.description ?? undefined}>
                      {pack.name} {pack.version}
                    </span>
                    <small>
                      {[
                        kindLabels[pack.kind],
                        pack.builtin ? "内置" : null,
                        pack.author ? `作者 ${pack.author}` : null,
                        `许可证 ${pack.license}`,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </small>
                  </span>
                  {!pack.builtin && (
                    <span className={settings.clipboardActions}>
                      <button
                        type="button"
                        className="secondary"
                        aria-label={`删除${pack.name}`}
                        disabled={working}
                        onClick={() => void removePack(pack)}
                      >
                        删除
                      </button>
                    </span>
                  )}
                </div>
              ))
            )}
          </div>
          {catalog.issues.length > 0 && (
            <div className={settings.managerBlock} role="list" aria-label="无法载入的扩展包">
              {catalog.issues.map((issue) => (
                <p
                  className={settings.managerNote}
                  role="listitem"
                  key={`${issue.kind}/${issue.folder}`}
                >
                  {kindLabels[issue.kind]} {issue.folder || "目录"} 无法载入：{issue.reason}
                </p>
              ))}
            </div>
          )}
        </GroupList>
      )}
      {mentionsEditable && (
        <GroupList title="@ 名单">
          <div className={settings.managerBlock}>
            <p className={settings.managerNote}>
              在「输入 → 实用功能」打开 @ 名字与地点后，按 @
              再输入拼音或首字母，就会从这份名单里出候选。名单只保存在本机，不随账号同步，也不会读取通讯录或位置。拼音可以留空，中文名字会自动取读音。
            </p>
            {mentions.map((entry, index) => (
              <div className={settings.phraseForm} key={index}>
                <label className={settings.field}>
                  名字或地点
                  <input
                    className={settings.fieldInput}
                    value={entry.text}
                    maxLength={MAX_MENTION_TEXT_UTF16}
                    onChange={(event) => updateMention(index, { text: event.target.value })}
                  />
                </label>
                <label className={settings.field}>
                  拼音
                  <input
                    className={settings.fieldInput}
                    value={entry.key}
                    placeholder="zhang'san"
                    maxLength={MAX_MENTION_KEY_BYTES}
                    autoCapitalize="off"
                    spellCheck={false}
                    onChange={(event) => updateMention(index, { key: event.target.value })}
                  />
                </label>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`删除第 ${index + 1} 行`}
                  onClick={() =>
                    setMentions((current) => current.filter((_, position) => position !== index))
                  }
                >
                  删除
                </button>
              </div>
            ))}
            {mentionIssue && mentionsDirty && (
              <p className={settings.settingsWarning} role="alert">
                {mentionIssue}
              </p>
            )}
            <div className={settings.managerActions}>
              <button
                type="button"
                className="secondary"
                disabled={mentions.length >= MAX_MENTIONS}
                onClick={() => setMentions((current) => [...current, { text: "", key: "" }])}
              >
                添加
              </button>
              <button
                type="button"
                className="primary"
                disabled={working || !mentionsDirty || mentionIssue !== null}
                onClick={() => void saveMentions()}
              >
                保存名单
              </button>
            </div>
          </div>
        </GroupList>
      )}
    </>
  );
}
