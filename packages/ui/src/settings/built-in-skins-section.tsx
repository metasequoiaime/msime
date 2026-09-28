import { SkinCandidatePreview } from "../skin/skin-candidate-preview";
import { SkinToolbarPreview } from "../skin/skin-toolbar-preview";
import { candidateSkinPalette, type PreviewTheme } from "../skin/skin-preview-palette";
import { skinOptions } from "../skin/skin-options";
import * as settings from "./settings-style";

export interface BuiltInSkinsSectionProps {
  selected: string;
  previewThemes: Partial<Record<string, PreviewTheme>>;
  defaultTheme: PreviewTheme;
  linux: boolean;
  onSelect: (id: string) => void;
  onTogglePreview: (id: string) => void;
}

/** Renders the built-in candidate skin cards shared by desktop and touch settings. */
export function BuiltInSkinsSection({
  selected,
  previewThemes,
  defaultTheme,
  linux,
  onSelect,
  onTogglePreview,
}: BuiltInSkinsSectionProps) {
  return (
    <div className={settings.skinGrid}>
      {skinOptions.map(([id, title, description, candidateOnlyDescription]) => {
        const theme = previewThemes[id] ?? defaultTheme;
        const active = selected === id;
        return (
          <article aria-label={title} className={settings.skinCard(active)} key={id}>
            <div className={settings.skinCardHeader} data-skin-card-header="">
              <div className={settings.skinCardBody}>
                <span className={settings.skinCardTitle}>
                  {title} ({theme === "dark" ? "Dark" : "Light"})
                </span>
                <span className={settings.skinCardDescription}>
                  {linux ? candidateOnlyDescription : description}
                </span>
              </div>
              <div className={settings.skinCardActions}>
                <button
                  type="button"
                  role="switch"
                  aria-label={title}
                  aria-checked={active}
                  className={settings.skinSwitch(active)}
                  onClick={() => onSelect(id)}
                >
                  <span className={settings.skinSwitchKnob(active)} />
                </button>
                <button
                  type="button"
                  className={settings.skinPreviewSwitch}
                  onClick={() => onTogglePreview(id)}
                >
                  {theme === "dark" ? "预览浅色" : "预览深色"}
                </button>
              </div>
            </div>
            <div
              className={`${settings.skinCardPreview} skin-${id}`}
              data-skin-preview=""
              data-preview-theme={theme}
              style={candidateSkinPalette(id, theme)}
              aria-hidden="true"
            >
              <div className={settings.skinPreviewStage} data-skin-stage="">
                <SkinCandidatePreview orientation="horizontal" />
              </div>
              <div className={settings.skinPreviewStage} data-skin-stage="">
                <SkinCandidatePreview orientation="vertical" />
              </div>
              {!linux && (
                <div className={settings.skinPreviewStage} data-skin-stage="">
                  <SkinToolbarPreview />
                </div>
              )}
            </div>
          </article>
        );
      })}
    </div>
  );
}
