import type { ReactNode } from "react";
import { candidateTextColor } from "../candidate/candidate-text-color";
import { Row } from "../core/platform-controls";
import type { CustomCandidateColors } from "../theme/global-theme";
import * as settings from "./settings-style";

/** A colour slot of the custom theme's candidate palette (`custom_theme.candidate_colors`). */
export type CandidateColorKey = keyof CustomCandidateColors;

/** The custom theme's candidate colours; an unset or null slot follows the theme underneath. */
export type CandidateColorPreferences = CustomCandidateColors;

export interface CandidateColorsSectionProps {
  preferences: CandidateColorPreferences;
  previewTheme: "light" | "dark";
  showRowColors: boolean;
  showSelectionAppearance: boolean;
  showBorderColor: boolean;
  linux: boolean;
  /** Sets or clears one slot. The settings model's `setCandidateColor` also selects the custom theme when a colour is chosen (see THEME_CONTRACT). */
  onChange: (key: CandidateColorKey, value: string | null) => void;
}

// The Fcitx5 classic UI theme format has no label or accent colour (platforms/linux/src/candidates/CandidateFcitxTheme.h), so on Linux the number and accent pickers reach only the IBus panel.
const linuxFcitxClassicColorNote =
  "Fcitx5 经典界面中编号跟随正文颜色、固定候选不单独着色，此项仅对 IBus 生效";

/** One colour picker of the custom theme: the swatch, and a button that hands the slot back to the theme underneath. */
function CandidateColorRow({
  title,
  slot,
  value,
  fallback,
  onChange,
  description,
  resetLabel,
  pressed = true,
}: {
  title: string;
  slot: CandidateColorKey;
  value: string | null | undefined;
  fallback: string;
  onChange: (slot: CandidateColorKey, value: string | null) => void;
  description?: ReactNode;
  /** The reset button's own name; the text picker's is the bare 跟随主题, the name it has always had. */
  resetLabel?: string;
  /** The surface and border resets have never reported a pressed state, and a test of each pins that. */
  pressed?: boolean;
}) {
  const set = candidateTextColor(value);
  return (
    <Row title={title} description={description}>
      <span className="candidate-color-control">
        <input
          aria-label={title}
          type="color"
          value={set ?? fallback}
          onChange={(event) => onChange(slot, event.target.value)}
        />
        <button
          type="button"
          aria-label={resetLabel}
          className={`candidate-color-reset${set ? "" : " is-active"}`}
          aria-pressed={pressed ? !set : undefined}
          onClick={() => {
            if (set || !pressed) onChange(slot, null);
          }}
        >
          跟随主题
        </button>
      </span>
    </Row>
  );
}

/** The custom theme's candidate colour pickers on the 主题 page, shared by every settings host; rows of its 自定义主题 group. */
export function CandidateColorsSection({
  preferences,
  previewTheme,
  showRowColors,
  showSelectionAppearance,
  showBorderColor,
  linux,
  onChange,
}: CandidateColorsSectionProps) {
  const light = previewTheme === "light";
  return (
    <>
      <CandidateColorRow
        title="候选文字颜色"
        slot="text"
        value={preferences.text}
        fallback={light ? "#1a1a1a" : "#e9e8e8"}
        onChange={onChange}
      />
      {showRowColors ? (
        <>
          <CandidateColorRow
            title="候选强调色"
            slot="accent"
            value={preferences.accent}
            fallback={light ? "#1a73e8" : "#8ab4f8"}
            onChange={onChange}
            resetLabel="候选强调色跟随主题"
            description={linux ? linuxFcitxClassicColorNote : undefined}
          />
          <CandidateColorRow
            title="候选选中色"
            slot="selected"
            value={preferences.selected}
            fallback={light ? "#e8e8e8" : "#3e3e3e"}
            onChange={onChange}
            resetLabel="候选选中色跟随主题"
          />
        </>
      ) : (
        <p className={settings.groupNote}>当前宿主的候选面板不支持强调或选中行颜色。</p>
      )}
      {showSelectionAppearance ? (
        <CandidateColorRow
          title="候选悬停色"
          slot="hover"
          value={preferences.hover}
          fallback={light ? "#ececec" : "#414141"}
          onChange={onChange}
          resetLabel="候选悬停色跟随主题"
        />
      ) : (
        <p className={settings.groupNote}>
          {linux
            ? "悬停颜色不支持；边框仅在 Fcitx5 经典界面绘制，IBus 候选窗无边框。"
            : "当前宿主的候选面板不支持悬停或边框颜色。"}
        </p>
      )}
      <CandidateColorRow
        title="候选表面色"
        slot="surface"
        value={preferences.surface}
        fallback={light ? "#ffffff" : "#202020"}
        onChange={onChange}
        resetLabel="候选表面色跟随主题"
        pressed={false}
      />
      {showBorderColor && (
        <CandidateColorRow
          title="候选边框色"
          slot="border"
          value={preferences.border}
          fallback={light ? "#dedede" : "#303030"}
          onChange={onChange}
          resetLabel="候选边框色跟随主题"
          pressed={false}
        />
      )}
      <CandidateColorRow
        title="候选编号颜色"
        slot="number"
        value={preferences.number}
        fallback={light ? "#5f6368" : "#bdc1c6"}
        onChange={onChange}
        resetLabel="候选编号颜色跟随主题"
        description={linux ? linuxFcitxClassicColorNote : undefined}
      />
    </>
  );
}
