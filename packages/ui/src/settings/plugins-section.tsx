import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import type { ConfirmRequest } from "../core/confirm";
import * as settings from "./settings-style";
import { withoutRemovedPack, type PluginPreferences } from "./plugin-preferences";
import { kindLabels, missingSelections, pluginErrorMessage } from "./plugin-catalog-helpers";
import type {
  MentionEntry,
  PluginCatalogResult,
  PluginClient,
  PluginKind,
  PluginPackage,
  PluginSettingsPage,
} from "./plugin-types";
import { PluginListView } from "./plugin-list-view";
import { MissingPluginView, PluginDetailView } from "./plugin-detail-view";
import { PluginSoundEffectsView } from "./plugin-sound-effects-view";
import { mentionListIssue, PluginMentionsView } from "./plugin-mentions-view";

export type {
  MentionEntry,
  PluginCatalogResult,
  PluginClient,
  PluginCommand,
  PluginIssue,
  PluginKind,
  PluginHelpcodeEntry,
  PluginPackage,
  PluginPhrase,
  PluginSettingsPage,
  PluginSymbolGroup,
} from "./plugin-types";
export { kindLabels, pluginErrorMessage } from "./plugin-catalog-helpers";
export { MAX_MENTIONS, mentionListIssue } from "./plugin-mentions-view";

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
  /** 快捷短语（K 模式）是否打开：`local_modes.quick_phrase`；关闭时短语表详情提示去打开。 */
  quickPhraseMode?: boolean;
  /** 宿主使用辅助码（设置里显示辅助码这一组）；辅助码表包只在这时标记和报告缺失。 */
  helpcode?: boolean;
  /** 宿主的背单词书目列出单词本插件（`HostCapabilities.wordbook_packs`）。 */
  wordbookPacks?: boolean;
  /** 宿主的符号面板显示符号集插件（`HostCapabilities.symbol_set_packs`）。 */
  symbolSetPacks?: boolean;
  /** 在背单词里选中一本书并打开背单词；没有背单词的宿主为空。 */
  onOpenWordbook?: (book: string) => void;
  /** 打开设置里的另一页（输入、背单词）；没有时详情里不显示这些链接。 */
  onOpenPage?: (page: PluginSettingsPage) => void;
  /** Loads the catalog and the name list each time this turns true, so a pack copied in by hand shows up on the next visit. Turning false also closes any open view, so the next visit starts at the list. */
  active: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onError: (message: string) => void;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
}

/** What the 我的插件 tab shows: the list, or one view opened from it. A pack is kept by key and looked up in the catalog on each render, so a pack that is gone after a reread falls back to the list. */
type PluginView =
  | { kind: "list" }
  | { kind: "pack"; packKind: PluginKind; id: string }
  | { kind: "missing"; packKind: PluginKind; id: string }
  | { kind: "sound-effects" }
  | { kind: "mentions" };

const listView: PluginView = { kind: "list" };

/** The pack kind a list row key (`pack/<kind>/<id>` or `missing/<kind>/<id>`) names, or null for the entries above the packs. */
function rowKind(rowKey: string): string | null {
  return /^(?:pack|missing)\/([^/]+)\//.exec(rowKey)?.[1] ?? null;
}

/** The 插件 page's 我的插件: the installed packs, each opening its own detail, with the settings that belong to no one pack (声音与效果) and the @ name list as entries above them. */
export function PluginsSection({
  client,
  preferences,
  keySound,
  music,
  triggers,
  typingEffects = false,
  effectStyles = false,
  effectPacks = false,
  quickPhraseMode = true,
  helpcode = false,
  wordbookPacks = false,
  symbolSetPacks = false,
  onOpenWordbook,
  onOpenPage,
  active,
  onChange,
  onError,
  confirm,
}: PluginsSectionProps) {
  const [catalog, setCatalog] = useState<PluginCatalogResult>({ packages: [], issues: [] });
  // Until a catalog has been read, an empty one says nothing about what is installed: no pack is reported missing.
  const [catalogState, setCatalogState] = useState<"loading" | "loaded" | "failed">("loading");
  const [working, setWorking] = useState(false);
  const [notice, setNotice] = useState("");
  const [mentions, setMentions] = useState<MentionEntry[]>([]);
  const [savedMentions, setSavedMentions] = useState<MentionEntry[]>([]);
  const [view, setView] = useState<PluginView>(listView);
  const mentionsEditable = triggers && Boolean(client);
  const actionRunning = useRef(false);
  const clientGeneration = useRef(0);
  // The page stays mounted while hidden, so a reload on the next visit must not overwrite edits that were never saved.
  const mentionsDirtyRef = useRef(false);
  const root = useRef<HTMLDivElement>(null);
  // Set when the user opens or closes a view: the back button, or the row the view was opened from, takes focus after the render, so a keyboard user is not left on a button that is gone.
  const pendingFocus = useRef<string | null>(null);
  const openedFrom = useRef<string | null>(null);

  useEffect(() => {
    if (!active || !client) return;
    const generation = ++clientGeneration.current;
    let current = true;
    // A read after a failed one shows as loading again; once a catalog is listed it stays on screen while it is reread.
    setCatalogState((state) => (state === "loaded" ? state : "loading"));
    void client
      .catalog()
      .then((next) => {
        if (!current) return;
        setCatalog(next);
        setCatalogState("loaded");
      })
      .catch((error: unknown) => {
        if (!current) return;
        setCatalogState((state) => (state === "loaded" ? state : "failed"));
        onError(pluginErrorMessage(error, "无法读取插件列表，请重试。"));
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

  // Leaving the page or switching to 社区插件 closes the open view: the next visit starts at the list.
  useEffect(() => {
    if (active) return;
    pendingFocus.current = null;
    setView(listView);
  }, [active]);

  useEffect(() => {
    const target = pendingFocus.current;
    if (target === null) return;
    pendingFocus.current = null;
    if (target === "back") {
      root.current?.querySelector<HTMLElement>("[data-plugin-back]")?.focus();
      return;
    }
    const rows = Array.from(root.current?.querySelectorAll<HTMLElement>("[data-plugin-row]") ?? []);
    // The row a view was opened from can be gone (its pack deleted, its missing selection dropped): focus then goes to the first row of the same kind, else the first row of the list, never to the page body.
    const kind = rowKind(target);
    const element =
      rows.find((row) => row.dataset.pluginRow === target) ??
      (kind ? rows.find((row) => rowKind(row.dataset.pluginRow ?? "") === kind) : undefined) ??
      rows[0];
    element?.focus();
  }, [view]);

  const openView = (next: PluginView, rowKey: string) => {
    openedFrom.current = rowKey;
    pendingFocus.current = "back";
    setView(next);
  };
  const backToList = () => {
    pendingFocus.current = openedFrom.current;
    setView(listView);
  };

  const { command_tables, phrase_tables } = preferences;
  const effectPacksDrawn = typingEffects && effectStyles && effectPacks;
  const packsListed = Boolean(client) && catalogState === "loaded";
  // The pack kinds the host acts on: only their selections are reported missing or marked in use.
  const actedKinds = new Set<PluginKind>([
    ...(keySound ? (["sound"] as const) : []),
    ...(music ? (["music"] as const) : []),
    ...(triggers ? (["command_table", "phrase_table"] as const) : []),
    ...(effectPacksDrawn ? (["effect"] as const) : []),
    ...(helpcode ? (["helpcode"] as const) : []),
  ]);
  // Selections naming a pack that is gone, for each kind the host acts on, listed so they can be dropped.
  const missing = packsListed ? missingSelections(preferences, catalog.packages, actedKinds) : [];

  // After an import or removal that already succeeded: a failed reread is reported as such, not as a failed import or removal.
  const refresh = async () => {
    if (!client) return;
    const generation = clientGeneration.current;
    try {
      const next = await client.catalog();
      if (generation !== clientGeneration.current) return;
      setCatalog(next);
      setCatalogState("loaded");
    } catch (error) {
      if (generation === clientGeneration.current)
        onError(pluginErrorMessage(error, "无法读取插件列表，请重试。"));
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
      // `runAsyncAction` leaves the busy flag alone once the action went stale (the page was left while a picker was open), and nothing else would clear it: the buttons would stay disabled until the page was mounted again.
      setWorking(false);
    }
  }

  const importPack = async (source: "folder" | "archive") => {
    await runPluginAction(async (isCurrent) => {
      const imported = await client!.importPack(source);
      if (!isCurrent()) return;
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
    await runPluginAction(async (isCurrent) => {
      await client!.remove(pack.kind, pack.id);
      if (!isCurrent()) return;
      onChange(withoutRemovedPack(preferences, pack.kind, pack.id));
      await refresh();
      if (!isCurrent()) return;
      // The row the detail was opened from is gone with the pack (once the reread lists it no more), so focus falls back to its kind's first row.
      pendingFocus.current = openedFrom.current;
      setView(listView);
    }, "删除失败，请重试。");
  };

  const setCommandTable = (id: string, enabled: boolean) =>
    onChange({
      ...preferences,
      command_tables: enabled
        ? [...command_tables.filter((table) => table !== id), id]
        : command_tables.filter((table) => table !== id),
    });

  const setPhraseTable = (id: string, enabled: boolean) =>
    onChange({
      ...preferences,
      phrase_tables: enabled
        ? [...phrase_tables.filter((table) => table !== id), id]
        : phrase_tables.filter((table) => table !== id),
    });

  const mentionIssue = mentionListIssue(mentions);
  const mentionsDirty = JSON.stringify(mentions) !== JSON.stringify(savedMentions);
  mentionsDirtyRef.current = mentionsDirty;
  const saveMentions = async () => {
    if (mentionIssue) return;
    await runPluginAction(async (isCurrent) => {
      const trimmed = mentions.map((entry) => ({ text: entry.text.trim(), key: entry.key }));
      await client!.saveMentions(trimmed);
      if (!isCurrent()) return;
      setMentions(trimmed);
      setSavedMentions(trimmed);
    }, "名单未能保存，请重试。");
  };

  const soundEffectParts = [
    keySound ? "按键音效" : null,
    typingEffects ? "打字效果" : null,
    music ? "背景音乐" : null,
  ].filter(Boolean);
  const soundEffects = soundEffectParts.length > 0;

  const content = (() => {
    if (view.kind === "pack") {
      const pack = catalog.packages.find(
        (item) => item.kind === view.packKind && item.id === view.id,
      );
      if (pack) {
        return (
          <PluginDetailView
            pack={pack}
            preferences={preferences}
            keySound={keySound}
            music={music}
            triggers={triggers}
            effectPacks={effectPacksDrawn}
            quickPhraseMode={quickPhraseMode}
            helpcode={helpcode}
            wordbookPacks={wordbookPacks}
            symbolSetPacks={symbolSetPacks}
            onOpenWordbook={onOpenWordbook}
            working={working}
            onChange={onChange}
            onCommandTable={setCommandTable}
            onPhraseTable={setPhraseTable}
            onOpenPage={onOpenPage}
            onRemove={(target) => void removePack(target)}
            onBack={backToList}
          />
        );
      }
    }
    if (view.kind === "missing") {
      const entry = missing.find((item) => item.kind === view.packKind && item.id === view.id);
      if (entry) {
        return (
          <MissingPluginView
            entry={entry}
            preferences={preferences}
            onChange={onChange}
            onBack={backToList}
          />
        );
      }
    }
    if (view.kind === "sound-effects" && soundEffects) {
      return (
        <PluginSoundEffectsView
          packages={catalog.packages}
          listed={packsListed}
          hasClient={Boolean(client)}
          preferences={preferences}
          keySound={keySound}
          music={music}
          typingEffects={typingEffects}
          effectStyles={effectStyles}
          effectPacks={effectPacks}
          onChange={onChange}
          onBack={backToList}
        />
      );
    }
    if (view.kind === "mentions" && mentionsEditable) {
      return (
        <PluginMentionsView
          mentions={mentions}
          setMentions={setMentions}
          issue={mentionIssue}
          dirty={mentionsDirty}
          working={working}
          onSave={() => void saveMentions()}
          onBack={backToList}
        />
      );
    }
    return (
      <PluginListView
        hasClient={Boolean(client)}
        catalog={catalog}
        catalogState={catalogState}
        missing={missing}
        kinds={actedKinds}
        preferences={preferences}
        triggers={triggers}
        soundEffects={soundEffects}
        soundEffectsDescription={`${soundEffectParts.join("、")}的开关与音量`}
        mentionsEditable={mentionsEditable}
        mentionsDescription={
          mentionsDirty ? "有未保存的修改" : `${savedMentions.length} 条，只保存在本机`
        }
        working={working}
        notice={notice}
        onImport={(source) => void importPack(source)}
        onOpen={(target) => openView({ kind: target }, target)}
        onOpenPack={(packKind, id) =>
          openView({ kind: "pack", packKind, id }, `pack/${packKind}/${id}`)
        }
        onOpenMissing={(packKind, id) =>
          openView({ kind: "missing", packKind, id }, `missing/${packKind}/${id}`)
        }
      />
    );
  })();

  return (
    <div ref={root} className={settings.subViewStack}>
      {content}
    </div>
  );
}
