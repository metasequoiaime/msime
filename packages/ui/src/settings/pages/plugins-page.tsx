import { useEffect, useState } from "react";
import { useSettingsForm } from "../settings-form-context";
import { PluginsSection } from "../plugins-section";
import { pluginPreferences } from "../plugin-preferences";
import { defaultLocalModes } from "../local-mode-preferences";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** The 插件 page of the settings form (route id `plugins`): the installed packs, each opening its own detail, with 声音与效果 and the @ name list as entries above them. `hidden` is set while the page shows the community gallery in its place. */
export function PluginsSettingsPage({ hidden = false }: { hidden?: boolean }) {
  const {
    client,
    draft,
    setDraft,
    busy,
    page,
    setError,
    confirm,
    showKeySound,
    showMusic,
    showPluginTriggers,
    showTypingEffects,
    showTypingEffectStyles,
    showTypingEffectPacks,
    showEffectIntensity,
    typingEffectsFlashOnly,
    showHelpcode,
    showWordbookPacks,
    showSymbolSetPacks,
    showScriptPlugins,
    host,
    selectPage,
  } = useSettingsForm();
  const vocabulary = client.vocabularyReview;
  // 「去背单词」：先在背单词里选中这本书，再打开背单词页。
  const openWordbook = vocabulary
    ? (book: string) =>
        void vocabulary
          .load()
          .then((status) => vocabulary.setSettings({ ...status.settings, wordbook: book }))
          .then(() => selectPage("vocabulary"))
          .catch(() => setError("没能打开背单词，请重试。"))
    : undefined;
  // 「成就音效」按打字统计计数：每次进入本页读一次统计开关，关着时那一行不能打开。宿主没有打字统计或读取失败时不知道，按打开处理。
  const statistics = showKeySound ? client.typingStatistics : undefined;
  const [statisticsEnabled, setStatisticsEnabled] = useState<boolean>();
  useEffect(() => {
    if (page !== "plugins" || !statistics) return;
    let current = true;
    statistics.load().then(
      (status) => {
        if (current) setStatisticsEnabled(status.statistics.enabled);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [page, statistics]);
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const localModes = draft.local_modes ?? defaultLocalModes;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "plugins" || hidden} ariaLabel="插件">
      <PluginsSection
        client={client.plugins}
        preferences={pluginPreferences(draft)}
        keySound={showKeySound}
        music={showMusic}
        triggers={showPluginTriggers}
        typingEffects={showTypingEffects}
        effectStyles={showTypingEffectStyles}
        effectPacks={showTypingEffectPacks}
        effectFlashOnly={typingEffectsFlashOnly}
        effectIntensity={showEffectIntensity}
        statisticsEnabled={statistics ? statisticsEnabled : undefined}
        quickPhraseMode={draft.local_modes?.quick_phrase ?? true}
        commandMode={draft.local_modes?.command ?? false}
        mentionMode={draft.local_modes?.mention ?? false}
        // 指令表详情和 @ 名单里的「打开」按钮就地改「输入 → 快捷模式」的开关，和在输入页拨动它写的是同一个键。
        onLocalMode={(mode) =>
          onPreferencesChange({ local_modes: { ...localModes, [mode]: true } })
        }
        scheme={draft.scheme}
        helpcode={showHelpcode}
        wordbookPacks={showWordbookPacks}
        symbolSetPacks={showSymbolSetPacks}
        scriptPlugins={showScriptPlugins}
        platform={host?.platform}
        onOpenWordbook={openWordbook}
        onOpenPage={selectPage}
        active={page === "plugins" && !hidden}
        onChange={(plugins) => onPreferencesChange({ plugins })}
        onError={setError}
        confirm={confirm}
      />
    </SettingsPageFieldset>
  );
}
