import { useEffect, useRef, useState } from "react";
import type { InputScheme } from "../core/input-scheme";
import type { ConfirmRequest } from "../core/confirm";
import { useAsyncActionRunner } from "../core/use-async-action";
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
  ScriptCapability,
  ScriptGrantsResult,
} from "./plugin-types";
import type { PluginLibraryFilter } from "./plugin-library-view";
import { runningScriptIds } from "./plugin-script-model";
import { mentionListIssue } from "./plugin-mentions-view";
import { useAsyncGeneration } from "./use-async-generation";
import { usePluginNavigation } from "./use-plugin-navigation";

interface PluginsSectionOptions {
  client?: PluginClient;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  triggers: boolean;
  typingEffects: boolean;
  effectStyles: boolean;
  effectPacks: boolean;
  mentionMode: boolean;
  scheme?: InputScheme;
  helpcode: boolean;
  scriptPlugins: boolean;
  active: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onError: (message: string) => void;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
}

/** 管理插件目录、筛选、授权和名单操作，所有异步结果受当前宿主代次约束。 */
export function usePluginsSection({
  client,
  preferences,
  keySound,
  music,
  triggers,
  typingEffects,
  effectStyles,
  effectPacks,
  mentionMode,
  scheme,
  helpcode,
  scriptPlugins,
  active,
  onChange,
  onError,
  confirm,
}: PluginsSectionOptions) {
  const [catalog, setCatalog] = useState<PluginCatalogResult>({ packages: [], issues: [] });
  // 首次读取完成前，空目录不能说明插件缺失。
  const [catalogState, setCatalogState] = useState<"loading" | "loaded" | "failed">("loading");
  const [notice, setNotice] = useState("");
  const [libraryFilter, setLibraryFilter] = useState<PluginLibraryFilter>({
    kind: "all",
    query: "",
  });
  const [mentions, setMentions] = useState<MentionEntry[]>([]);
  const [savedMentions, setSavedMentions] = useState<MentionEntry[]>([]);
  const [scriptGrants, setScriptGrants] = useState<ScriptGrantsResult | null>(null);
  const mentionsEditable = triggers && Boolean(client);
  // 只有运行脚本插件的桌面宿主有授权方法。
  const scriptsShown = scriptPlugins && Boolean(client?.scriptGrants);
  // 方案不支持 K、/ 和 @ 时，列表优先说明方案限制。
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
  // 页面隐藏时仍挂载，重访读取不能覆盖未保存的名单编辑。
  const mentionsDirtyRef = useRef(false);

  useEffect(() => {
    if (!client) {
      setCatalog({ packages: [], issues: [] });
      setCatalogState("loading");
      setScriptGrants(null);
      return;
    }
    if (!active) return;
    const generation = clientGeneration.current;
    // 首次失败后重读显示加载中；已经成功列出的目录在重读期间保留。
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
    // 只在重访或宿主能力变化时读取，普通重渲染及 `onError` 身份变化不触发重读。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, client, mentionsEditable, scriptsShown]);

  const navigation = usePluginNavigation(active);

  const { command_tables, phrase_tables } = preferences;
  const effectPacksDrawn = typingEffects && effectStyles && effectPacks;
  const packsListed = Boolean(client) && catalogState === "loaded";
  // 仅对宿主实际使用的包类型报告缺失和标记使用中。
  const actedKinds = new Set<PluginKind>([
    ...(keySound ? (["sound"] as const) : []),
    ...(music ? (["music"] as const) : []),
    ...(triggers ? (["command_table", "phrase_table"] as const) : []),
    ...(effectPacksDrawn ? (["effect"] as const) : []),
    ...(helpcode ? (["helpcode"] as const) : []),
  ]);
  // 列出宿主使用但目录中已不存在的选择，供用户恢复默认。
  const missing = packsListed ? missingSelections(preferences, catalog.packages, actedKinds) : [];

  // 导入或删除已经成功后，重读失败单独报告读取错误。
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
      // 原包已被删除，由导航按原入口类型恢复列表焦点。
      navigation.backToList();
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

  return {
    ...navigation,
    catalog,
    catalogState,
    notice,
    libraryFilter,
    setLibraryFilter,
    mentions,
    setMentions,
    mentionsEditable,
    mentionsDirty,
    mentionIssue,
    mentionsDescription: mentionsDirty
      ? "有未保存的修改"
      : `${savedMentions.length} 条，只保存在本机${mentionModeGap}`,
    working,
    scriptsShown,
    scriptPacks,
    scriptGrants,
    runningScripts,
    schemeOpens,
    effectPacksDrawn,
    packsListed,
    actedKinds,
    missing,
    soundEffects,
    soundEffectsDescription: `${soundEffectParts.join("、")}的开关与音量`,
    importPack,
    removePack,
    grantScript,
    revokeScript,
    moveScript,
    setCommandTable,
    setPhraseTable,
    saveMentions,
  };
}
