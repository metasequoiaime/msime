import type { ReactNode } from "react";
import type { HostPlatform, Preferences } from "../index";
import { GroupList, Row, Segmented, Slider } from "../core/platform-controls";
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

/** The 候选字体 preset row, the first row of the 候选窗口 page's 字体与大小 group. */
export function CandidateFontPresetRow({
  preferences,
  platform,
  onChange,
}: CandidateFontPresetRowProps) {
  const preset = currentCandidateFontPreset(preferences, platform);
  return (
    <Row
      title="候选字体"
      description={
        preset === null
          ? `当前为自定义字体 ${preferences.candidate_font_family ?? ""}，可在下方修改`
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
  );
}

export interface CandidateScaleRowProps {
  preferences: Pick<Preferences, "candidate_scale_percent">;
  onChange: (patch: Partial<Preferences>) => void;
}

/** The 整体大小 slider, the last size row of the 候选窗口 page's 字体与大小 group: it scales the font sizes above it together with the window. */
export function CandidateScaleRow({ preferences, onChange }: CandidateScaleRowProps) {
  const scale = candidateScalePercent(preferences.candidate_scale_percent);
  return (
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
  );
}

export interface CandidateWindowStyleSectionProps {
  preferences: Pick<Preferences, "candidate_opacity_percent" | "candidate_corner_radius">;
  showOpacity: boolean;
  showCornerRadius: boolean;
  onChange: (patch: Partial<Preferences>) => void;
  /** Rows after the window's own style, such as the link to where its colours and light/dark are chosen. */
  children?: ReactNode;
}

/** What the slider shows while the card follows the skin and the host: the radius the preview draws then. */
const followedCornerRadius = 6;

/** The 窗口样式 group of the 候选窗口 page: the window's opacity and corner radius. Its colours and light/dark are edited only on the 主题 page, which the page links to from the end of this group. */
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
