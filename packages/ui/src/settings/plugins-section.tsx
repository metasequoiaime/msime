import type { InputScheme } from "../core/input-scheme";
import { useEffect, useRef, useState } from "react";

import { useAsyncActionRunner } from "../core/use-async-action";
import type { ConfirmRequest } from "../core/confirm";
import * as settings from "./settings-style";
import { withoutRemovedPack, type PluginPreferences } from "./plugin-preferences";
import {
  missingSelections,
  packKindLabel,
  pluginErrorCode,
  pluginErrorMessage,
  schemeOpensTableModes,
} from "./plugin-catalog-helpers";
import type {
  MentionEntry,
  PluginCatalogResult,
  PluginClient,
  PluginKind,
  PluginPackage,
  PluginSettingsPage,
  ScriptCapability,
  ScriptGrantsResult,
} from "./plugin-types";
import { PluginListView } from "./plugin-list-view";
import type { PluginLibraryFilter } from "./plugin-library-view";
import { runningScriptIds, scriptResidualRisk, ScriptPluginDetailView } from "./plugin-script-view";
import { MissingPluginView, PluginDetailView } from "./plugin-detail-view";
import { PluginSoundEffectsView } from "./plugin-sound-effects-view";
import { mentionListIssue, PluginMentionsView } from "./plugin-mentions-view";
import { useAsyncGeneration } from "./use-async-generation";

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
  ScriptCapability,
  ScriptCommand,
  ScriptCommandState,
  ScriptGrant,
  ScriptGrantsResult,
  ScriptHealth,
} from "./plugin-types";
export {
  SCRIPT_CAPABILITIES,
  ScriptGrantDialog,
  capabilityLabels,
  SCRIPT_BUILTIN_NOTE,
  SCRIPT_COMMAND_MODE_NOTE,
  scriptCommandStatus,
  scriptGrantState,
  scriptResidualRisk,
  toggleCapability,
  type ScriptGrantDialogProps,
} from "./plugin-script-view";
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
  /** 宿主把每种特效样式都画成候选卡片闪一下，越强的样式闪得越亮，不画火花（HarmonyOS）；只在 `effectStyles` 为真时读取。 */
  effectFlashOnly?: boolean;
  /** 宿主按 `plugins.effect_intensity` 画效果，「声音与效果」给出「效果强度」；固定强度的宿主（macOS、Windows、鸿蒙，`HostCapabilities.fixed_effect_intensity`）不给。 */
  effectIntensity?: boolean;
  /** 打字统计是否打开；关着时「成就音效」不能打开。不知道时按打开处理。 */
  statisticsEnabled?: boolean;
  /** 快捷短语（K 模式）是否打开：`local_modes.quick_phrase`；关闭时短语表详情提示去打开。 */
  quickPhraseMode?: boolean;
  /** / 指令是否打开：`local_modes.command`，缺省为关；关闭时指令表详情提示打开，列表不把已启用的指令表标成生效。 */
  commandMode?: boolean;
  /** @ 模式是否打开：`local_modes.mention`，缺省为关；关闭时 @ 名单提示打开。 */
  mentionMode?: boolean;
  /** 打开「输入 → 快捷模式」里的 / 指令或 @ 模式，留在当前页面；没有时提示改为跳到输入页（`onOpenPage`）。 */
  onLocalMode?: (mode: "command" | "mention") => void;
  /** 当前输入方案；只有全拼、双拼和五笔打开 K、/ 和 @ 模式，其他方案下列表（已启用短语表、指令表的标记和 @ 名单的说明）和详情都会说明。缺省时按能打开处理，不提示。 */
  scheme?: InputScheme;
  /** 宿主使用辅助码（设置里显示辅助码这一组）；辅助码表包只在这时标记和报告缺失。 */
  helpcode?: boolean;
  /** 宿主的背单词书目列出单词本插件（`HostCapabilities.wordbook_packs`）。 */
  wordbookPacks?: boolean;
  /** 宿主的符号面板显示符号集插件（`HostCapabilities.symbol_set_packs`）。 */
  symbolSetPacks?: boolean;
  /** 宿主运行脚本插件（`HostCapabilities.script_plugins`）：显示「脚本插件（实验）」分组和授权页。 */
  scriptPlugins?: boolean;
  /** 宿主平台（`HostCapabilities.platform`），授权页按它说明这个平台上同一用户下的程序能伪造授权。 */
  platform?: string;
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
  effectFlashOnly = false,
  effectIntensity = true,
  statisticsEnabled,
  quickPhraseMode = true,
  commandMode = false,
  mentionMode = false,
  onLocalMode,
  scheme,
  helpcode = false,
  wordbookPacks = false,
  symbolSetPacks = false,
  scriptPlugins = false,
  platform,
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
  const [notice, setNotice] = useState("");
  const [libraryFilter, setLibraryFilter] = useState<PluginLibraryFilter>({
    kind: "all",
    query: "",
  });
  const [mentions, setMentions] = useState<MentionEntry[]>([]);
  const [savedMentions, setSavedMentions] = useState<MentionEntry[]>([]);
  const [view, setView] = useState<PluginView>(listView);
  const [scriptGrants, setScriptGrants] = useState<ScriptGrantsResult | null>(null);
  const mentionsEditable = triggers && Boolean(client);
  // 只有运行脚本插件的桌面宿主有授权方法。
  const scriptsShown = scriptPlugins && Boolean(client?.scriptGrants);
  // The engine opens K, / and @ only under some schemes; when the current one cannot, no switch helps, so the list names the scheme first.
  const schemeOpens = scheme ? schemeOpensTableModes(scheme) : true;
  const mentionModeGap = !schemeOpens
    ? "；当前方案打不开 @ 模式"
    : mentionMode
      ? ""
      : "；@ 模式未开";
  const clientGeneration = useAsyncGeneration(active, client, mentionsEditable, scriptsShown);
  const { busy: working, run: runAsyncPluginAction } = useAsyncActionRunner(
    onError,
    setNotice,
    active,
    client,
    mentionsEditable,
    scriptsShown,
  );
  // The page stays mounted while hidden, so a reload on the next visit must not overwrite edits that were never saved.
  const mentionsDirtyRef = useRef(false);
  const root = useRef<HTMLDivElement>(null);
  // Set when the user opens or closes a view: the back button, or the row the view was opened from, takes focus after the render, so a keyboard user is not left on a button that is gone.
  const pendingFocus = useRef<string | null>(null);
  const openedFrom = useRef<string | null>(null);

  useEffect(() => {
    if (!client) {
      setCatalog({ packages: [], issues: [] });
      setCatalogState("loading");
      setScriptGrants(null);
      return;
    }
    if (!active) return;
    const generation = clientGeneration.current;
    // A read after a failed one shows as loading again; once a catalog is listed it stays on screen while it is reread.
    setCatalogState((state) => (state === "loaded" ? state : "loading"));
    void client
      .catalog()
      .then((next) => {
        if (generation !== clientGeneration.current) return;
        setCatalog(next);
        setCatalogState("loaded");
      })
      .catch((error: unknown) => {
        if (generation !== clientGeneration.current) return;
        setCatalogState((state) => (state === "loaded" ? state : "failed"));
        onError(pluginErrorMessage(error, "无法读取插件列表，请重试。"));
      });
    if (scriptsShown) {
      void client.scriptGrants!()
        .then((next) => {
          if (generation === clientGeneration.current) setScriptGrants(next);
        })
        .catch((error: unknown) => {
          if (generation === clientGeneration.current)
            onError(pluginErrorMessage(error, "无法读取脚本插件的授权，请重试。"));
        });
    }
    if (mentionsEditable) {
      void client
        .loadMentions()
        .then((next) => {
          if (generation !== clientGeneration.current) return;
          if (!mentionsDirtyRef.current) setMentions(next);
          setSavedMentions(next);
        })
        .catch((error: unknown) => {
          if (generation === clientGeneration.current)
            onError(pluginErrorMessage(error, "无法读取 @ 名单，请重试。"));
        });
    }
    // `onError` is the page's setter and stable; the effect reloads on a visit, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, client, mentionsEditable, scriptsShown]);

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
      // 导入或删除脚本包时宿主在同一把锁里改了授权条目，一起重读。
      if (scriptsShown) {
        const grants = await client.scriptGrants!();
        if (generation === clientGeneration.current) setScriptGrants(grants);
      }
    } catch (error) {
      if (generation === clientGeneration.current)
        onError(pluginErrorMessage(error, "无法读取插件列表，请重试。"));
    }
  };

  async function runPluginAction(
    operation: (isCurrent: () => boolean) => Promise<void>,
    fallback: string,
  ) {
    if (!client) return;
    await runAsyncPluginAction(operation, {
      formatError: (error) => pluginErrorMessage(error, fallback),
    });
  }

  const importPack = async (source: "folder" | "archive") => {
    await runPluginAction(async (isCurrent) => {
      const imported = await client!.importPack(source);
      if (!isCurrent()) return;
      if (imported) {
        setNotice(`已导入${packKindLabel(imported)}「${imported.name}」${imported.version}。`);
        await refresh();
      }
    }, "导入失败，请重试。");
  };

  const removePack = async (pack: PluginPackage) => {
    const confirmed = await confirm({
      title: `删除${packKindLabel(pack)}`,
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

  const scriptPacks = scriptsShown ? catalog.packages.filter((pack) => pack.kind === "script") : [];
  const runningScripts = runningScriptIds(scriptGrants, scriptPacks);

  // 写授权：包在页面读到之后被换过（`plugin_script_changed`）时重读目录和授权，页面换成现在的内容哈希，再把错误抛给调用方说明。
  const writeScriptGrant = async (
    pack: PluginPackage,
    enabled: boolean,
    granted: ScriptCapability[],
  ) => {
    try {
      return await client!.setScriptGrant!(pack.id, pack.content_hash ?? "", enabled, granted);
    } catch (error) {
      if (pluginErrorCode(error) === "plugin_script_changed") await refresh();
      throw error;
    }
  };

  // 授权对话框里的确认：失败时抛出，由对话框说明原因（例如超出内存总账）。
  const grantScript = async (pack: PluginPackage, granted: ScriptCapability[]) => {
    const generation = clientGeneration.current;
    const next = await writeScriptGrant(pack, true, granted);
    if (generation === clientGeneration.current) setScriptGrants(next);
  };

  const revokeScript = async (pack: PluginPackage) => {
    await runPluginAction(async (isCurrent) => {
      const next = await writeScriptGrant(pack, false, []);
      if (isCurrent()) setScriptGrants(next);
    }, "停用失败，请重试。");
  };

  const moveScript = async (id: string, offset: -1 | 1) => {
    const position = runningScripts.indexOf(id);
    const target = position + offset;
    if (position < 0 || target < 0 || target >= runningScripts.length) return;
    const order = [...runningScripts];
    [order[position], order[target]] = [order[target], order[position]];
    await runPluginAction(async (isCurrent) => {
      const next = await client!.reorderScripts!(order);
      if (isCurrent()) setScriptGrants(next);
    }, "执行顺序没有保存，请重试。");
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
      if (pack && pack.kind === "script") {
        if (scriptsShown) {
          return (
            <ScriptPluginDetailView
              pack={pack}
              grants={scriptGrants}
              running={runningScripts}
              platform={platform}
              working={working}
              commandMode={commandMode}
              onLocalMode={onLocalMode}
              onOpenPage={onOpenPage}
              nameOf={(id) => scriptPacks.find((candidate) => candidate.id === id)?.name ?? id}
              onGrant={(granted) => grantScript(pack, granted)}
              onRevoke={() => void revokeScript(pack)}
              onMove={(offset) => void moveScript(pack.id, offset)}
              onRemove={(target) => void removePack(target)}
              onBack={backToList}
            />
          );
        }
      } else if (pack) {
        return (
          <PluginDetailView
            pack={pack}
            preferences={preferences}
            keySound={keySound}
            music={music}
            triggers={triggers}
            effectPacks={effectPacksDrawn}
            effectFlashOnly={effectFlashOnly}
            quickPhraseMode={quickPhraseMode}
            commandMode={commandMode}
            scheme={scheme}
            helpcode={helpcode}
            wordbookPacks={wordbookPacks}
            symbolSetPacks={symbolSetPacks}
            onOpenWordbook={onOpenWordbook}
            working={working}
            onChange={onChange}
            onCommandTable={setCommandTable}
            onPhraseTable={setPhraseTable}
            onLocalMode={onLocalMode}
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
          effectFlashOnly={effectFlashOnly}
          effectIntensity={effectIntensity}
          statisticsEnabled={statisticsEnabled}
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
          mode={mentionMode}
          scheme={scheme}
          onEnableMode={onLocalMode && (() => onLocalMode("mention"))}
          onOpenPage={onOpenPage}
          onSave={() => void saveMentions()}
          onBack={backToList}
        />
      );
    }
    return (
      <PluginListView
        // macOS 和 Linux 都用可筛选的卡片插件库；Linux 的配色由 GNOME 平台 token 决定。
        library={
          platform === "macos" || platform === "linux"
            ? { filter: libraryFilter, onFilter: setLibraryFilter }
            : undefined
        }
        hasClient={Boolean(client)}
        catalog={catalog}
        catalogState={catalogState}
        missing={missing}
        kinds={actedKinds}
        preferences={preferences}
        modes={{ quickPhrase: quickPhraseMode, command: commandMode, schemeOpens }}
        triggers={triggers}
        soundEffects={soundEffects}
        soundEffectsDescription={`${soundEffectParts.join("、")}的开关与音量`}
        mentionsEditable={mentionsEditable}
        mentionsDescription={
          mentionsDirty
            ? "有未保存的修改"
            : `${savedMentions.length} 条，只保存在本机${mentionModeGap}`
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
        scripts={
          scriptsShown
            ? {
                packages: scriptPacks,
                grants: scriptGrants,
                running: runningScripts,
                risk: scriptResidualRisk(platform),
              }
            : null
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
