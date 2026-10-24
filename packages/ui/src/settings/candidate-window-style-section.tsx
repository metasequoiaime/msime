import { useMacOSLiquidGlass } from "../core/use-macos-liquid-glass";
import type { HostPlatform } from "../core/host-capabilities";
import type { ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import type {
  CandidateFontPreferences,
  CandidateWindowStylePreferences,
} from "../candidate/candidate-preferences";
import { GroupList, Row, Slider, Switch } from "../core/platform-controls";
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
  candidateGeometryFields,
  candidateGeometryValue,
  candidateGeometryPatch,
} from "../candidate/candidate-window-style";
import * as settings from "./settings-style";
import { SegmentedRow } from "./segmented-row";
import { SliderRow } from "./slider-row";
import { ActionButton } from "../core/action-button";

export type CandidateWindowStyleSectionPreferences = Pick<
  CandidateFontPreferences,
  "candidate_font_family" | "candidate_fallback_fonts"
> &
  CandidateWindowStylePreferences;

export interface CandidateFontPresetRowProps {
  preferences: Pick<CandidateFontPreferences, "candidate_font_family" | "candidate_fallback_fonts">;
  platform: HostPlatform | undefined;
  onChange: (patch: Partial<Preferences>) => void;
  /** 用户点了「自定义…」：字体还停在某个预设上时也选中这一项，好让页面亮出字体框。 */
  custom?: boolean;
  /** 给了它，预设后面就多一项「自定义…」：选它只回报 `true`、不改字体，选回预设时回报 `false`。没有它时（HarmonyOS 手机的「更多选项」）字体框一直在下面，不需要这一项。 */
  onCustomChange?: (custom: boolean) => void;
}

/** 「候选字体」预设行，「候选窗口」页主流程里字体那一行。 */
export function CandidateFontPresetRow({
  preferences,
  platform,
  onChange,
  custom = false,
  onCustomChange,
}: CandidateFontPresetRowProps) {
  const preset = currentCandidateFontPreset(preferences, platform);
  const presets: { value: CandidateFontPresetId | "custom"; label: string; disabled?: boolean }[] =
    candidateFontPresets.map((entry) => ({
      value: entry.id,
      label: entry.label,
      disabled: !candidateFontPresetAvailable(entry.id, platform),
    }));
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
      options={onCustomChange ? [...presets, { value: "custom", label: "自定义…" }] : presets}
      value={preset === null || (custom && onCustomChange) ? "custom" : preset}
      onChange={(id) => {
        if (id === "custom") {
          onCustomChange?.(true);
          return;
        }
        onCustomChange?.(false);
        onChange(candidateFontPresetPatch(id, platform, preferences));
      }}
    />
  );
}

export interface CandidateScaleRowProps {
  preferences: Pick<CandidateWindowStylePreferences, "candidate_scale_percent">;
  platform?: HostPlatform;
  onChange: (patch: Partial<Preferences>) => void;
}

/** 「整体大小」滑块；自绘候选窗和 Linux 的 Fcitx5 主题连同窗口一起缩放，IBus 面板只缩放候选字号（见 `candidatePanelLimitNotes.ibus_panel`）。 */
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
  preferences: CandidateWindowStylePreferences & { global_theme?: Preferences["global_theme"] };
  /** 宿主平台，决定「跟随皮肤」时滑块停在哪个圆角和内边距上。 */
  platform?: HostPlatform;
  showOpacity: boolean;
  showCornerRadius: boolean;
  /** 宿主声明了 `candidate_padding` 时显示「内边距」行（目前是 macOS 和 Linux 的 Fcitx5）。 */
  showPadding?: boolean;
  showItemGeometry?: boolean;
  /** 宿主消费 `candidate_window_fit_content` 时显示「收紧窗口宽度」行（目前是 macOS）。 */
  showFitContent?: boolean;
  onChange: (patch: Partial<Preferences>) => void;
  /** 排在窗口自身样式之后的行，例如链接到颜色和明暗设置处的那一行。 */
  children?: ReactNode;
}

/** 卡片跟随皮肤和宿主时滑块显示的圆角：macOS 是原生候选窗的 10pt，即那时预览画的卡片圆角（`skin-card-preview` 里 macOS 默认值块的 `--msime-host-card-radius`）；Linux 是 Fcitx5 经典界面主题画的 10px 卡片圆角（`CandidateFcitxTheme.h` 的 `FcitxPanelGeometry::radius`），滑块从这里往上拖卡片才会变圆；其他平台仍是 6。 */
function followedCornerRadius(platform: HostPlatform | undefined): number {
  return platform === "macos" || platform === "linux" ? 10 : 6;
}

/** 卡片跟随皮肤和宿主时滑块显示的内边距：macOS 原生候选窗的 4pt（`skin-card-preview` 里 macOS 默认值块的 `--msime-host-pad`），Linux Fcitx5 卡片照 macOS 也是 4px（`FcitxPanelGeometry::padding`）。只有这两个平台声明这一行，其他平台的值只是为了滑块有个落点。 */
function followedCardPadding(platform: HostPlatform | undefined): number {
  return platform === "macos" || platform === "linux" ? 4 : 2;
}

/** 「窗口样式」组：窗口的不透明度、圆角和内边距，HarmonyOS 手机的「候选栏」页在宿主声明支持时用它。颜色和明暗只在「主题」页编辑。 */
export function CandidateWindowStyleSection({
  children,
  ...rows
}: CandidateWindowStyleSectionProps) {
  return (
    <GroupList title="窗口样式">
      <CandidateWindowStyleRows {...rows} />
      {children}
    </GroupList>
  );
}

/** 不透明度、圆角和内边距这几行本身，不带组：桌面「候选窗口」页把它们放进「更多选项」。 */
export function CandidateWindowStyleRows({
  preferences,
  platform,
  showOpacity,
  showCornerRadius,
  showPadding = false,
  showItemGeometry = false,
  showFitContent = false,
  onChange,
}: Omit<CandidateWindowStyleSectionProps, "children">) {
  const nativeGlass = useMacOSLiquidGlass();
  const systemOpacity =
    platform === "macos" && nativeGlass && (preferences.global_theme ?? "system") === "system";
  const opacity = candidateOpacityPercent(preferences.candidate_opacity_percent);
  const radius = candidateCornerRadius(preferences.candidate_corner_radius);
  const padding = candidateCardPadding(preferences.candidate_padding);
  return (
    <>
      {showFitContent && (
        <Row
          title="收紧窗口宽度"
          description="严格按文字所需的宽度绘制候选窗，不垫出厂最小宽度；皮肤的 fit_content 打开时始终收紧"
        >
          <Switch
            checked={preferences.candidate_window_fit_content ?? false}
            onChange={(candidate_window_fit_content) => onChange({ candidate_window_fit_content })}
          />
        </Row>
      )}
      {showOpacity && (
        <SliderRow
          title="不透明度"
          description={
            systemOpacity
              ? "系统液态玻璃的不透明度由 macOS 管理；自定义主题可单独调整"
              : `${opacity}%，文字和焦点高亮保持不透明`
          }
          {...candidateOpacitySlider}
          disabled={systemOpacity}
          value={opacity}
          valueText={systemOpacity ? "跟随系统" : `${opacity}%`}
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
      {showPadding && !showItemGeometry && (
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
      {showItemGeometry &&
        candidateGeometryFields.map((field) => {
          const value = candidateGeometryValue(preferences, field.key);
          return (
            <Row
              key={field.key}
              title={field.title}
              description={value === null ? "跟随皮肤与窗口布局" : `${value}pt，覆盖皮肤与窗口布局`}
            >
              <span className="candidate-color-control">
                <span className={settings.sliderControl}>
                  <Slider
                    min={0}
                    max={field.max}
                    step={1}
                    value={value ?? field.followed}
                    valueText={value === null ? "跟随皮肤" : `${value}pt`}
                    onChange={(next) =>
                      onChange(candidateGeometryPatch(preferences, field.key, next))
                    }
                  />
                </span>
                <ActionButton
                  action={() => {
                    if (value !== null) {
                      onChange(candidateGeometryPatch(preferences, field.key, null));
                    }
                  }}
                  ariaPressed={value === null}
                  className={`candidate-color-reset${value === null ? " is-active" : ""}`}
                  label="跟随皮肤"
                />
              </span>
            </Row>
          );
        })}
    </>
  );
}
