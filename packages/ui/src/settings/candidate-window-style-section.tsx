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
  candidateCardPadding,
  candidateCardPaddingPatch,
  candidateCardPaddingSlider,
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
import { ActionButton } from "../core/action-button";

export type CandidateWindowStyleSectionPreferences = Pick<
  Preferences,
  | "candidate_font_family"
  | "candidate_fallback_fonts"
  | "candidate_scale_percent"
  | "candidate_opacity_percent"
  | "candidate_corner_radius"
  | "candidate_padding"
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
  preferences: Pick<
    Preferences,
    "candidate_opacity_percent" | "candidate_corner_radius" | "candidate_padding"
  >;
  /** 宿主平台，决定「跟随皮肤」时滑块停在哪个圆角和内边距上。 */
  platform?: HostPlatform;
  showOpacity: boolean;
  showCornerRadius: boolean;
  /** 宿主声明了 `candidate_padding` 时显示「内边距」行（目前只有 macOS）。 */
  showPadding?: boolean;
  onChange: (patch: Partial<Preferences>) => void;
  /** 排在窗口自身样式之后的行，例如链接到颜色和明暗设置处的那一行。 */
  children?: ReactNode;
}

/** 卡片跟随皮肤和宿主时滑块显示的圆角，即那时预览画的卡片圆角：macOS 是原生候选窗的 10pt（`skin-card-preview` 里 macOS 默认值块的 `--msime-host-card-radius`），其他平台的预览还没有自己的默认值块，仍是 6。 */
function followedCornerRadius(platform: HostPlatform | undefined): number {
  return platform === "macos" ? 10 : 6;
}

/** 卡片跟随皮肤和宿主时滑块显示的内边距：macOS 原生候选窗的 6pt（`skin-card-preview` 里 macOS 默认值块的 `--msime-host-pad`）。只有 macOS 声明这一行，其他平台的值只是为了滑块有个落点。 */
function followedCardPadding(platform: HostPlatform | undefined): number {
  return platform === "macos" ? 6 : 2;
}

/** 「候选窗口」页的「窗口样式」组：窗口的不透明度、圆角和内边距。颜色和明暗只在「主题」页编辑，本页在这一组末尾链接过去。 */
export function CandidateWindowStyleSection({
  preferences,
  platform,
  showOpacity,
  showCornerRadius,
  showPadding = false,
  onChange,
  children,
}: CandidateWindowStyleSectionProps) {
  const opacity = candidateOpacityPercent(preferences.candidate_opacity_percent);
  const radius = candidateCornerRadius(preferences.candidate_corner_radius);
  const padding = candidateCardPadding(preferences.candidate_padding);
  return (
    <GroupList title="窗口样式">
      {showOpacity && (
        <SliderRow
          title="不透明度"
          description={`${opacity}%，文字和焦点高亮保持不透明`}
          {...candidateOpacitySlider}
          value={opacity}
          valueText={`${opacity}%`}
          onChange={(value) => onChange(candidateOpacityPatch(value))}
        />
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
                value={radius ?? followedCornerRadius(platform)}
                valueText={radius === null ? "跟随皮肤" : `${radius}pt`}
                onChange={(value) => onChange(candidateCornerRadiusPatch(value))}
              />
            </span>
            <ActionButton
              action={() => {
                if (radius !== null) onChange(candidateCornerRadiusPatch(null));
              }}
              ariaPressed={radius === null}
              className={`candidate-color-reset${radius === null ? " is-active" : ""}`}
              label="跟随皮肤"
            />
          </span>
        </Row>
      )}
      {showPadding && (
        <Row
          title="内边距"
          description={
            padding === null
              ? "跟随皮肤；调小后候选离窗口边缘更近，窗口更紧凑"
              : `${padding}pt，覆盖皮肤自带的内边距`
          }
        >
          <span className="candidate-color-control">
            <span className={settings.sliderControl}>
              <Slider
                {...candidateCardPaddingSlider}
                value={padding ?? followedCardPadding(platform)}
                valueText={padding === null ? "跟随皮肤" : `${padding}pt`}
                onChange={(value) => onChange(candidateCardPaddingPatch(value))}
              />
            </span>
            <ActionButton
              action={() => {
                if (padding !== null) onChange(candidateCardPaddingPatch(null));
              }}
              ariaPressed={padding === null}
              className={`candidate-color-reset${padding === null ? " is-active" : ""}`}
              label="跟随皮肤"
            />
          </span>
        </Row>
      )}
      {children}
    </GroupList>
  );
}
