import type { Preferences } from "../index";
import { ReservedCandidatePreview, type PreviewReserve } from "../skin/skin-candidate-preview";
import { candidateFontSize, candidateFontStyle } from "./candidate-font-size";
import { candidateFamilyStyle } from "./candidate-font-family";
import { candidateWindowStyle } from "./candidate-window-style";
import { ExternalAppearancePreview } from "../skin/external-appearance-preview";
import type { SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { useCandidatePreviewTheme } from "./candidate-preview-theme";
import { useResolvedCandidateFonts, type FontFamilyResolver } from "./resolved-candidate-fonts";
import * as settings from "../settings/settings-style";
import { defaultHelpcode } from "../settings/pages/helpcode-page";
import {
  customCandidateStyle,
  themeCandidateStyle,
  themeEntry,
  type ResolvedTheme,
  type ResolveThemeRequest,
} from "../theme/global-theme";

export function AppearanceCandidatePreview({
  preferences: storedPreferences,
  scan,
  readImage,
  resolveTheme,
  resolveFonts,
  active = true,
  revision = 0,
  mobile = false,
  reserve,
}: {
  preferences: Preferences;
  scan?: () => Promise<SkinCatalog>;
  readImage?: SkinImageReader;
  /** `SettingsClient.resolveTheme`: when present, a custom theme's package is previewed with the host's own `resolve()` answer. */
  resolveTheme?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  resolveFonts?: FontFamilyResolver;
  active?: boolean;
  revision?: number;
  mobile?: boolean;
  /** 设置页按能切换的最高排布预留预览高度，见 `ReservedCandidatePreview`；不传就随样例伸缩。 */
  reserve?: PreviewReserve;
}) {
  const preferences = useResolvedCandidateFonts(
    storedPreferences,
    active ? resolveFonts : undefined,
  );
  const globalTheme = preferences.global_theme ?? "system";
  const custom = globalTheme === "custom";
  const colors = preferences.custom_theme?.candidate_colors;
  const mode = useCandidatePreviewTheme(preferences.theme, preferences.candidate_theme);
  // A built-in theme is one fixed palette, and so is a custom theme over one; only `system` and a custom theme over it follow the light/dark mode.
  const base = custom ? (preferences.custom_theme?.base ?? "system") : globalTheme;
  const theme = themeEntry(base).appearance ?? mode;
  // Only a custom theme with an external package needs the package preview.
  const builtin = !custom || !preferences.custom_theme?.candidate_skin;
  const helpcodeKey = preferences.scheme === "quanpin" ? "quanpin_helpcode" : "shuangpin_helpcode";
  // A missing object takes the core's per-scheme default (全拼 hides its codes, 双拼 shows them); a missing field inside a stored object is the core's serde default, which is on.
  const schemeHelpcode = preferences[helpcodeKey] ?? defaultHelpcode[helpcodeKey];
  const helpcode =
    (preferences.scheme === "quanpin" || preferences.scheme === "shuangpin") &&
    (schemeHelpcode.enabled ?? true) &&
    (schemeHelpcode.show_in_candidate_window ?? true);
  const surfaceName = mobile ? "候选栏" : "候选窗口";
  return (
    <section className={settings.groupPreview} aria-label={`${surfaceName}预览`}>
      <div className={settings.panelPreviewLabel}>
        预览：固定样例随当前设置草稿变化，不代表实际输入候选。
      </div>
      {builtin ? (
        <div
          data-skin-preview=""
          className={`${settings.skinCardPreview} appearance-candidate-preview`}
          data-global-theme={globalTheme}
          data-preview-theme={theme}
          data-font-size={candidateFontSize(preferences.candidate_font_size)}
          style={{
            // The pickers belong to the custom theme and draw nowhere else.
            ...(custom ? customCandidateStyle(base, colors) : themeCandidateStyle(globalTheme)),
            ...candidateFontStyle(preferences),
            ...candidateFamilyStyle(preferences),
            ...candidateWindowStyle(preferences),
          }}
          aria-hidden="true"
        >
          <div className={settings.skinPreviewStage} data-skin-stage="">
            <ReservedCandidatePreview
              reserve={reserve}
              orientation={preferences.candidate_layout ?? "vertical"}
              count={preferences.candidate_page_size}
              preedit={preferences.candidate_preedit_style !== "empty"}
              helpcode={helpcode}
            />
          </div>
        </div>
      ) : (
        <ExternalAppearancePreview
          preferences={preferences}
          theme={theme}
          scan={scan}
          readImage={readImage}
          resolve={resolveTheme}
          active={active}
          revision={revision}
          helpcode={helpcode}
          reserve={reserve}
        />
      )}
    </section>
  );
}
