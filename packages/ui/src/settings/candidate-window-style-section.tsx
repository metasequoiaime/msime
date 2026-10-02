import type { ReactNode } from "react";
import type { HostPlatform, Preferences } from "../index";
import { GroupList, Row, Slider } from "../core/platform-controls";
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
import * as settings from "./settings-style";
import { SegmentedRow } from "./segmented-row";
import { SliderRow } from "./slider-row";

export type CandidateWindowStyleSectionPreferences = Pick<
  Preferences,
  | "candidate_font_family"
  | "candidate_fallback_fonts"
  | "candidate_scale_percent"
  | "candidate_opacity_percent"
  | "candidate_corner_radius"
>;

export interface CandidateFontPresetRowProps {
  preferences: Pick<Preferences, "candidate_font_family" | "candidate_fallback_fonts">;
  platform: HostPlatform | undefined;
  onChange: (patch: Partial<Preferences>) => void;
}

/** 「候选字体」预设行，「候选窗口」页「字体与大小」组的第一行。 */
export function CandidateFontPresetRow({
  preferences,
  platform,
  onChange,
}: CandidateFontPresetRowProps) {
  const preset = currentCandidateFontPreset(preferences, platform);
  return (
    <SegmentedRow
      title="候选字体"
      description={
        preset === null
          ? `当前为自定义字体 ${preferences.candidate_font_family ?? ""}，可在下方修改`
          : platform === "windows"
            ? "Windows 没有自带圆体"
            : undefined
      }
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
  );
}

export interface CandidateScaleRowProps {
  preferences: Pick<Preferences, "candidate_scale_percent">;
  onChange: (patch: Partial<Preferences>) => void;
}

/** 「整体大小」滑块，「候选窗口」页「字体与大小」组的最后一个尺寸行：它把上面的各个字号连同窗口一起缩放。 */
export function CandidateScaleRow({ preferences, onChange }: CandidateScaleRowProps) {
  const scale = candidateScalePercent(preferences.candidate_scale_percent);
  return (
    <SliderRow
      title="整体大小"
      description={`${scale}%，字号与窗口尺寸一起缩放`}
      {...candidateScaleSlider}
      value={scale}
      valueText={`${scale}%`}
      onChange={(value) => onChange(candidateScalePatch(value))}
    />
  );
}

export interface CandidateWindowStyleSectionProps {
  preferences: Pick<Preferences, "candidate_opacity_percent" | "candidate_corner_radius">;
  showOpacity: boolean;
  showCornerRadius: boolean;
  onChange: (patch: Partial<Preferences>) => void;
  /** 排在窗口自身样式之后的行，例如链接到颜色和明暗设置处的那一行。 */
  children?: ReactNode;
}

/** What the slider shows while the card follows the skin and the host: the radius the preview draws then. */
const followedCornerRadius = 6;

/** 「候选窗口」页的「窗口样式」组：窗口的不透明度和圆角。颜色和明暗只在「主题」页编辑，本页在这一组末尾链接过去。 */
export function CandidateWindowStyleSection({
  preferences,
  showOpacity,
  showCornerRadius,
  onChange,
  children,
}: CandidateWindowStyleSectionProps) {
  const opacity = candidateOpacityPercent(preferences.candidate_opacity_percent);
  const radius = candidateCornerRadius(preferences.candidate_corner_radius);
  return (
    <GroupList title="窗口样式">
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
      {children}
    </GroupList>
  );
}
