import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { PluginsSection } from "../plugins-section";
import { pluginPreferences } from "../plugin-preferences";
import { createSettingsDraftActions } from "../settings-draft-actions";

/** The 插件 page of the settings form (route id `plugins`): sound packs, typing effects, background music, command tables and the @ name list. `hidden` is set while the page shows the community gallery in its place. */
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
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return (
    <fieldset disabled={busy} hidden={page !== "plugins" || hidden} aria-label="插件">
      <div className={settings.groups}>
        <PluginsSection
          client={client.plugins}
          preferences={pluginPreferences(draft)}
          keySound={showKeySound}
          music={showMusic}
          triggers={showPluginTriggers}
          typingEffects={showTypingEffects}
          effectStyles={showTypingEffectStyles}
          effectPacks={showTypingEffectPacks}
          active={page === "plugins" && !hidden}
          onChange={(plugins) => onPreferencesChange({ plugins })}
          onError={setError}
          confirm={confirm}
        />
      </div>
    </fieldset>
  );
}
