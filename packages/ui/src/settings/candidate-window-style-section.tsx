import type { HostPlatform, Preferences } from "../index";
import { GroupList, Row, Segmented, Select, Slider } from "../core/platform-controls";
import {
  candidateFontPresetAvailable,
  candidateFontPresetPatch,
  candidateFontPresets,
  currentCandidateFontPreset,
  type CandidateFontPresetId,
} from "../candidate/candidate-font-presets";
import {
  candidateCornerRadius,
  candidateCornerRadiusPatch,
  candidateCornerRadiusSlider,
  candidateOpacityPatch,
  candidateOpacityPercent,
  candidateOpacitySlider,
  candidateScalePatch,
  candidateScalePercent,
  candidateScaleSlider,
} from "../candidate/candidate-window-style";
import { CandidateColorRow, type CandidateColorKey } from "./candidate-colors-section";
import { SurfaceThemeSelect, type SurfaceTheme } from "./surface-theme-select";
import type { CustomCandidateColors } from "../theme/global-theme";
import * as settings from "./settings-style";

export type CandidateWindowStyleSectionPreferences = Pick<
  Preferences,
  | "candidate_theme"
  | "candidate_font_family"
  | "candidate_fallback_fonts"
  | "candidate_scale_percent"
  | "candidate_opacity_percent"
  | "candidate_corner_radius"
>;

export interface CandidateWindowStyleSectionProps {
  preferences: CandidateWindowStyleSectionPreferences;
  /** The custom theme's candidate colours; the four pickers here write the same slots as the 主题 page's 自定义主题 group. */
  colors: CustomCandidateColors;
  previewTheme: "light" | "dark";
  platform: HostPlatform | undefined;
  showFontPresets: boolean;
  showRowColors: boolean;
  showScale: boolean;
  showOpacity: boolean;
  showCornerRadius: boolean;
  onChange: (patch: Partial<Preferences>) => void;
  /** The settings model's `onCandidateColorChange`, which also selects the custom theme when a colour is chosen. */
  onColorChange: (key: CandidateColorKey, value: string | null) => void;
}

/** What the slider shows while the card follows the skin and the host: the radius the preview draws then. */
const followedCornerRadius = 6;

/** The 候选窗 group of the 候选窗口 page: the candidate window's theme, font preset, size, opacity, colours and corner radius in one place. The colours are the custom theme's pickers, so choosing one switches the global theme to custom exactly as the 主题 page does. */
export function CandidateWindowStyleSection({
  preferences,
  colors,
  previewTheme,
  platform,
  showFontPresets,
  showRowColors,
  showScale,
  showOpacity,
  showCornerRadius,
  onChange,
  onColorChange,
}: CandidateWindowStyleSectionProps) {
  const light = previewTheme === "light";
  const preset = currentCandidateFontPreset(preferences, platform);
  const scale = candidateScalePercent(preferences.candidate_scale_percent);
  const opacity = candidateOpacityPercent(preferences.candidate_opacity_percent);
  const radius = candidateCornerRadius(preferences.candidate_corner_radius);
  // Titled 候选窗 on every host, the phone's candidate strip included: 候选栏 is already the page's own title there.
  return (
    <GroupList title="候选窗">
      <Row title="主题" description="跟随全局时使用颜色模式的明暗">
        <SurfaceThemeSelect
          label="主题"
          value={preferences.candidate_theme ?? "follow"}
          optionOrder={["follow", "light", "dark"]}
          onChange={(value) => onChange({ candidate_theme: value })}
        />
      </Row>
      {showFontPresets && (
        <Row
          title="候选字体"
          description={
            preset === null
              ? `当前为自定义字体 ${preferences.candidate_font_family ?? ""}，可在下方字体组修改`
              : platform === "windows"
                ? "Windows 没有自带圆体"
                : undefined
          }
        >
          <Segmented<CandidateFontPresetId | "custom">
            options={candidateFontPresets.map((entry) => ({
              value: entry.id,
              label: entry.label,
              disabled: !candidateFontPresetAvailable(entry.id, platform),
            }))}
            value={preset ?? "custom"}
            onChange={(id) => {
              if (id !== "custom") onChange(candidateFontPresetPatch(id, platform, preferences));
            }}
          />
        </Row>
      )}
      {showScale && (
        <Row title="整体大小" description={`${scale}%，字号与窗口尺寸一起缩放`}>
          <span className={settings.sliderControl}>
            <Slider
              {...candidateScaleSlider}
              value={scale}
              valueText={`${scale}%`}
              onChange={(value) => onChange(candidateScalePatch(value))}
            />
          </span>
        </Row>
      )}
      {showOpacity && (
        <Row title="不透明度" description={`${opacity}%，文字和焦点高亮保持不透明`}>
          <span className={settings.sliderControl}>
            <Slider
              {...candidateOpacitySlider}
              value={opacity}
              valueText={`${opacity}%`}
              onChange={(value) => onChange(candidateOpacityPatch(value))}
            />
          </span>
        </Row>
      )}
      <CandidateColorRow
        title="背景颜色"
        slot="surface"
        value={colors.surface}
        fallback={light ? "#ffffff" : "#202020"}
        onChange={onColorChange}
        description="选色后候选框使用该底色"
        resetLabel="背景颜色跟随主题"
      />
      {showRowColors && (
        <CandidateColorRow
          title="焦点高亮颜色"
          slot="selected"
          value={colors.selected}
          fallback={light ? "#e8e8e8" : "#3e3e3e"}
          onChange={onColorChange}
          description="选中候选的底色，文字明暗自动适配"
          resetLabel="焦点高亮颜色跟随主题"
        />
      )}
      <CandidateColorRow
        title="文字颜色"
        slot="text"
        value={colors.text}
        fallback={light ? "#1a1a1a" : "#e9e8e8"}
        onChange={onColorChange}
        description="普通候选词的颜色"
        resetLabel="文字颜色跟随主题"
      />
      <CandidateColorRow
        title="序号颜色"
        slot="number"
        value={colors.number}
        fallback={light ? "#5f6368" : "#bdc1c6"}
        onChange={onColorChange}
        description="候选序号 1-9 的颜色"
        resetLabel="序号颜色跟随主题"
      />
      {showCornerRadius && (
        <Row
          title="圆角大小"
          description={
            radius === null
              ? "跟随皮肤；设置后覆盖皮肤自带的圆角"
              : `${radius}pt，覆盖皮肤自带的圆角`
          }
        >
          <span className="candidate-color-control">
            <span className={settings.sliderControl}>
              <Slider
                {...candidateCornerRadiusSlider}
                value={radius ?? followedCornerRadius}
                valueText={radius === null ? "跟随皮肤" : `${radius}pt`}
                onChange={(value) => onChange(candidateCornerRadiusPatch(value))}
              />
            </span>
            <button
              type="button"
              className={`candidate-color-reset${radius === null ? " is-active" : ""}`}
              aria-pressed={radius === null}
              onClick={() => {
                if (radius !== null) onChange(candidateCornerRadiusPatch(null));
              }}
            >
              跟随皮肤
            </button>
          </span>
        </Row>
      )}
    </GroupList>
  );
}
