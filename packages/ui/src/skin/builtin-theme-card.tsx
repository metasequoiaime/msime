import type { Preferences } from "../core/preferences-model";
import { ActionButton } from "../core/action-button";
import {
  candidateWindowStyle,
  type CandidateWindowStylePreferences,
} from "../candidate/candidate-window-style";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { globalThemeDescription } from "../settings/settings-options";
import * as settings from "../settings/settings-style";
import {
  customCandidateStyle,
  keyboardThemeId,
  themeCandidateStyle,
  themeEntry,
  type CustomTheme,
  type GlobalTheme,
  type ThemeAppearance,
  type ThemeCatalogEntry,
} from "../theme/global-theme";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { SkinCardHeader } from "./skin-card-header";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";
import { SkinToolbarPreview } from "./skin-toolbar-preview";

export interface BuiltinThemeCardProps {
  entry: ThemeCatalogEntry;
  globalTheme: GlobalTheme;
  packageSelected: boolean;
  preferences: CandidateWindowStylePreferences & Pick<Preferences, "custom_theme">;
  previewCustomTheme?: CustomTheme;
  candidatePreviewTheme: ThemeAppearance;
  previewTheme?: ThemeAppearance;
  keyboardDesign: TouchKeyboardSkinDesign;
  touchRows?: PreviewKey[][];
  mobile: boolean;
  linux: boolean;
  onPreferencesChange: (patch: Pick<Preferences, "global_theme" | "custom_theme">) => void;
  onTogglePreview: () => void;
}

/** 内置主题的完整卡片；逐主题预览覆盖和设置保存由调用方持有。 */
export function BuiltinThemeCard({
  entry,
  globalTheme,
  packageSelected,
  preferences,
  previewCustomTheme,
  candidatePreviewTheme,
  previewTheme: previewOverride,
  keyboardDesign,
  touchRows,
  mobile,
  linux,
  onPreferencesChange,
  onTogglePreview,
}: BuiltinThemeCardProps) {
  const id = entry.id;
  // 「自定义」卡片只画基础主题与配色；目录里找不到的皮肤包按这张卡片回退。
  const selected = globalTheme === id && (id !== "custom" || !packageSelected);
  const fixedAppearance =
    id === "custom"
      ? themeEntry(preferences.custom_theme?.base ?? "system").appearance
      : entry.appearance;
  // 只有系统主题及其上的自定义主题跟随明暗，其余内置主题固定配色。
  const previewTheme = fixedAppearance ?? previewOverride ?? candidatePreviewTheme;
  return (
    <article aria-label={entry.title} className={settings.skinCard(selected)}>
      <SkinCardHeader
        title={entry.title}
        theme={previewTheme}
        selected={selected}
        description={globalThemeDescription(entry, linux)}
        actions={
          <>
            <ActionButton
              action={() =>
                onPreferencesChange(
                  id === "custom"
                    ? // 选「自定义」卡片清除两个皮肤包槽位，保留其余配置；预览投影不写回原始设置。
                      {
                        global_theme: "custom",
                        custom_theme: {
                          ...preferences.custom_theme,
                          candidate_skin: null,
                          candidate_skin_dark: null,
                        },
                      }
                    : { global_theme: id },
                )
              }
              ariaChecked={selected}
              ariaLabel={entry.title}
              className={settings.skinSwitch(selected)}
              label={<span className={settings.skinSwitchKnob(selected)} />}
              role="switch"
            />
            {fixedAppearance === null && (
              <ActionButton
                action={onTogglePreview}
                className={settings.skinPreviewSwitch}
                label={previewTheme === "dark" ? "预览浅色" : "预览深色"}
              />
            )}
          </>
        }
      />
      <SkinPreviewSurface
        data-global-theme={id}
        data-preview-theme={previewTheme}
        style={{
          ...(id === "custom"
            ? customCandidateStyle(previewCustomTheme?.base, previewCustomTheme?.candidate_colors)
            : themeCandidateStyle(id)),
          // 用户的窗口圆角和不透明度对每个主题都生效，与原生设置预览一致。
          ...candidateWindowStyle(preferences),
        }}
        aria-hidden="true"
      >
        <div className={settings.skinCandidateStages}>
          <SkinPreviewStage>
            <SkinCandidatePreview orientation="horizontal" />
          </SkinPreviewStage>
          <SkinPreviewStage>
            <SkinCandidatePreview orientation="vertical" />
          </SkinPreviewStage>
        </div>
        {/* 触屏宿主没有悬浮工具栏，卡片改为展示键盘。 */}
        {mobile ? (
          <SkinPreviewStage>
            <ScreenKeyboardPreview
              theme={previewTheme}
              skin={id === "custom" ? keyboardThemeId("custom", preferences.custom_theme) : id}
              customDesign={id === "custom" ? keyboardDesign : undefined}
              compact={!mobile}
              phone={mobile}
              touchRows={touchRows}
            />
          </SkinPreviewStage>
        ) : (
          !linux && (
            <SkinPreviewStage>
              <SkinToolbarPreview />
            </SkinPreviewStage>
          )
        )}
      </SkinPreviewSurface>
    </article>
  );
}
