import { useSettingsForm } from "../settings-form-context";
import { PluginsSection } from "../plugins-section";
import { pluginPreferences } from "../plugin-preferences";
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
    showHelpcode,
    showWordbookPacks,
    showSymbolSetPacks,
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
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
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
        quickPhraseMode={draft.local_modes?.quick_phrase ?? true}
        helpcode={showHelpcode}
        wordbookPacks={showWordbookPacks}
        symbolSetPacks={showSymbolSetPacks}
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
