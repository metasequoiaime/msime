import type { CSSProperties } from "react";
import type { CandidateWindowStylePreferences } from "./candidate-preferences";

export type { CandidateWindowStylePreferences } from "./candidate-preferences";

export const defaultCandidateScalePercent = 100;
export const defaultCandidateOpacityPercent = 100;
/** The core accepts 50-200; the page's slider offers the part that still fits on a screen. */
export const candidateScaleSlider = { min: 75, max: 150, step: 5 } as const;
export const candidateOpacitySlider = { min: 50, max: 100, step: 5 } as const;
/** The core accepts up to 32 like a skin package's `corner_radius_dip`; the slider stops at 16, past which a card reads as a pill. */
export const candidateCornerRadiusSlider = { min: 0, max: 16, step: 1 } as const;
/** 覆盖核心接受的整个 0–16；macOS 的零内边距直接贴边，不受边框宽度限制。 */
export const candidateCardPaddingSlider = { min: 0, max: 16, step: 1 } as const;

export const candidateGeometryFields = [
  { key: "candidate_padding_horizontal", title: "窗口水平内边距", max: 16, followed: 4 },
  { key: "candidate_padding_vertical", title: "窗口垂直内边距", max: 16, followed: 4 },
  { key: "candidate_item_padding_horizontal", title: "候选项水平内边距", max: 16, followed: 8 },
  { key: "candidate_item_padding_vertical", title: "候选项垂直内边距", max: 16, followed: 6 },
  { key: "candidate_item_corner_radius", title: "候选项圆角", max: 32, followed: 6 },
  { key: "candidate_item_spacing", title: "候选项间距", max: 16, followed: 0 },
] as const;

export type CandidateGeometryKey = (typeof candidateGeometryFields)[number]["key"];

export function candidateGeometryValue(
  preferences: CandidateWindowStylePreferences,
  key: CandidateGeometryKey,
): number | null {
  const value =
    key === "candidate_padding_horizontal" || key === "candidate_padding_vertical"
      ? (preferences[key] ?? preferences.candidate_padding)
      : preferences[key];
  return boundedInteger(value, 0, key === "candidate_item_corner_radius" ? 32 : 16);
}

export function candidateGeometryPatch(
  preferences: CandidateWindowStylePreferences,
  key: CandidateGeometryKey,
  value: number | null,
): CandidateWindowStylePreferences {
  if (key === "candidate_padding_horizontal" || key === "candidate_padding_vertical") {
    const other =
      key === "candidate_padding_horizontal"
        ? "candidate_padding_vertical"
        : "candidate_padding_horizontal";
    // 第一次编辑分轴值时拆开旧值；恢复某一轴不能重新落回旧的统一覆盖。
    return {
      candidate_padding: undefined,
      [other]: candidateGeometryValue(preferences, other) ?? undefined,
      [key]: value ?? undefined,
    };
  }
  return { [key]: value ?? undefined };
}

function boundedInteger(value: unknown, min: number, max: number): number | null {
  return typeof value === "number" && Number.isInteger(value) && value >= min && value <= max
    ? value
    : null;
}

export function candidateScalePercent(value: unknown): number {
  return boundedInteger(value, 50, 200) ?? defaultCandidateScalePercent;
}

export function candidateOpacityPercent(value: unknown): number {
  return boundedInteger(value, 50, 100) ?? defaultCandidateOpacityPercent;
}

/** The user's card radius in points, or `null` when the card follows the skin package and then the host. */
export function candidateCornerRadius(value: unknown): number | null {
  return boundedInteger(value, 0, 32);
}

/** The user's card padding in points, or `null` when the card follows the skin package and then the host. */
export function candidateCardPadding(value: unknown): number | null {
  return boundedInteger(value, 0, 16);
}

/** The preference patch for a scale: the default is written as absent, as the core leaves it out of the document, so moving the slider back to 100% does not leave the draft looking edited. */
export function candidateScalePatch(value: number): CandidateWindowStylePreferences {
  return { candidate_scale_percent: value === defaultCandidateScalePercent ? undefined : value };
}

export function candidateOpacityPatch(value: number): CandidateWindowStylePreferences {
  return {
    candidate_opacity_percent: value === defaultCandidateOpacityPercent ? undefined : value,
  };
}

/** `null` hands the radius back to the skin package and the host; it is written as absent for the same reason as the defaults above. */
export function candidateCornerRadiusPatch(value: number | null): CandidateWindowStylePreferences {
  return { candidate_corner_radius: value ?? undefined };
}

/** `null` hands the padding back to the skin package and the host, written as absent like the radius. */
export function candidateCardPaddingPatch(value: number | null): CandidateWindowStylePreferences {
  return { candidate_padding: value ?? undefined };
}

/** The preview's custom properties for the window style: the scale zooms the card, the opacity mixes the surface and border towards transparent, and a set radius replaces the skin package's `--msime-skin-radius`. Spread it after the skin geometry so the user's radius wins, as it does on the hosts. The set radius is also `--msime-candidate-user-radius`, the only radius the rows are clamped to: a package's radius leaves them as the hosts leave theirs. A set padding is `--msime-candidate-user-pad`, which the card's padding takes ahead of the host's `--msime-host-pad`. */
export function candidateWindowStyle(preferences: CandidateWindowStylePreferences): CSSProperties {
  const radius = candidateCornerRadius(preferences.candidate_corner_radius);
  const padding = candidateCardPadding(preferences.candidate_padding);
  const geometry: Record<string, string> = {};
  for (const field of candidateGeometryFields) {
    const value = boundedInteger(preferences[field.key], 0, field.max);
    if (value !== null) geometry[`--msime-${field.key.replaceAll("_", "-")}`] = `${value}px`;
  }
  if (candidateCardPadding(preferences.candidate_item_padding_horizontal) !== null) {
    geometry["--msime-candidate-preview-text-pad-left"] = "0px";
    geometry["--msime-candidate-preview-text-pad-right"] = "0px";
  }
  if (candidateCornerRadius(preferences.candidate_item_corner_radius) !== null) {
    geometry["--msime-candidate-content-overflow"] = "hidden";
  }
  return {
    "--msime-candidate-scale": String(
      candidateScalePercent(preferences.candidate_scale_percent) / 100,
    ),
    "--msime-candidate-opacity": `${candidateOpacityPercent(preferences.candidate_opacity_percent)}%`,
    ...(radius === null
      ? {}
      : { "--msime-skin-radius": `${radius}px`, "--msime-candidate-user-radius": `${radius}px` }),
    ...(padding === null ? {} : { "--msime-candidate-user-pad": `${padding}px` }),
    ...geometry,
  } as CSSProperties;
}
