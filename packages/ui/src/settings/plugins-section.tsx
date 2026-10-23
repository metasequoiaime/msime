import type { InputScheme } from "../core/input-scheme";
import type { ConfirmRequest } from "../core/confirm";
import * as settings from "./settings-style";
import type { PluginPreferences } from "./plugin-preferences";
import type { PluginClient, PluginSettingsPage } from "./plugin-types";
import { PluginListView } from "./plugin-list-view";
import { scriptResidualRisk } from "./plugin-script-model";
import { ScriptPluginDetailView } from "./plugin-script-view";
import { MissingPluginView, PluginDetailView } from "./plugin-detail-view";
import { PluginSoundEffectsView } from "./plugin-sound-effects-view";
import { PluginMentionsView } from "./plugin-mentions-view";
import { usePluginsSection } from "./use-plugins-section";

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
  capabilityLabels,
  SCRIPT_BUILTIN_NOTE,
  SCRIPT_COMMAND_MODE_NOTE,
  scriptCommandStatus,
  scriptGrantState,
  scriptResidualRisk,
  toggleCapability,
} from "./plugin-script-model";
export { ScriptGrantDialog, type ScriptGrantDialogProps } from "./script-grant-dialog";
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

/** 「我的插件」组合共享状态和导航，分发列表、详情、声音与效果及名单视图。 */
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
  const {
    root,
    view,
    openView,
    backToList,
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
    mentionsDescription,
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
    soundEffectsDescription,
    importPack,
    removePack,
    grantScript,
    revokeScript,
    moveScript,
    setCommandTable,
    setPhraseTable,
    saveMentions,
  } = usePluginsSection({
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
  });

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
        soundEffectsDescription={soundEffectsDescription}
        mentionsEditable={mentionsEditable}
        mentionsDescription={mentionsDescription}
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
