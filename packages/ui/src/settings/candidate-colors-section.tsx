import { candidateTextColor } from "../candidate/candidate-text-color";

export type CandidateColorKey =
  | "candidate_text_color"
  | "candidate_number_color"
  | "candidate_accent_color"
  | "candidate_selected_color"
  | "candidate_hover_color"
  | "candidate_surface_color"
  | "candidate_border_color";

export type CandidateColorPreferences = Partial<Record<CandidateColorKey, string | null>>;

export interface CandidateColorsSectionProps {
  preferences: CandidateColorPreferences;
  previewTheme: "light" | "dark";
  showRowColors: boolean;
  showSelectionAppearance: boolean;
  showBorderColor: boolean;
  linux: boolean;
  onChange: (key: CandidateColorKey, value: string | null) => void;
}

type ColorControlProps = {
  preferences: CandidateColorPreferences;
  previewTheme: "light" | "dark";
  preferenceKey: CandidateColorKey;
  label: string;
  fallback: { light: string; dark: string };
  resetAriaLabel?: string;
  resetPressed?: boolean;
  onlyResetOverrides?: boolean;
  note?: string;
  onChange: (key: CandidateColorKey, value: string | null) => void;
};

function ColorControl({
  preferences,
  previewTheme,
  preferenceKey,
  label,
  fallback,
  resetAriaLabel,
  resetPressed,
  onlyResetOverrides = false,
  note,
  onChange,
}: ColorControlProps) {
  const override = candidateTextColor(preferences[preferenceKey]);
  const reset = () => {
    if (!onlyResetOverrides || override) onChange(preferenceKey, null);
  };

  return (
    <div className="section">
      <div className="section-header">
        <span className="section-title">{label}</span>
        <div className="candidate-color-control">
          <input
            aria-label={label}
            type="color"
            value={override ?? fallback[previewTheme]}
            onChange={(event) => onChange(preferenceKey, event.target.value)}
          />
          <button
            type="button"
            {...(resetAriaLabel ? { "aria-label": resetAriaLabel } : {})}
            className={`candidate-color-reset${override ? "" : " is-active"}`}
            {...(resetPressed === undefined ? {} : { "aria-pressed": resetPressed })}
            onClick={reset}
          >
            跟随主题
          </button>
        </div>
      </div>
      {note && <small>{note}</small>}
    </div>
  );
}

const linuxFcitxClassicColorNote =
  "Fcitx5 经典界面中编号跟随正文颜色、固定候选不单独着色，此项仅对 IBus 生效";

/** Shared candidate color overrides for the appearance settings page. */
export function CandidateColorsSection({
  preferences,
  previewTheme,
  showRowColors,
  showSelectionAppearance,
  showBorderColor,
  linux,
  onChange,
}: CandidateColorsSectionProps) {
  return (
    <>
      <ColorControl
        preferences={preferences}
        previewTheme={previewTheme}
        preferenceKey="candidate_text_color"
        label="候选文字颜色"
        fallback={{ light: "#1a1a1a", dark: "#e9e8e8" }}
        onlyResetOverrides
        resetPressed={!candidateTextColor(preferences.candidate_text_color)}
        onChange={onChange}
      />
      {!showRowColors && (
        <div className="section">
          <small>当前宿主的候选面板不支持强调或选中行颜色。</small>
        </div>
      )}
      {!showSelectionAppearance && (
        <div className="section">
          <small>
            {linux
              ? "悬停颜色不支持；边框仅在 Fcitx5 经典界面绘制，IBus 候选窗无边框。"
              : "当前宿主的候选面板不支持悬停或边框颜色。"}
          </small>
        </div>
      )}
      {showRowColors && (
        <ColorControl
          preferences={preferences}
          previewTheme={previewTheme}
          preferenceKey="candidate_accent_color"
          label="候选强调色"
          fallback={{ light: "#1a73e8", dark: "#8ab4f8" }}
          resetAriaLabel="候选强调色跟随主题"
          resetPressed={!candidateTextColor(preferences.candidate_accent_color)}
          note={linux ? linuxFcitxClassicColorNote : undefined}
          onChange={onChange}
        />
      )}
      {showRowColors && (
        <ColorControl
          preferences={preferences}
          previewTheme={previewTheme}
          preferenceKey="candidate_selected_color"
          label="候选选中色"
          fallback={{ light: "#e8e8e8", dark: "#3e3e3e" }}
          resetAriaLabel="候选选中色跟随主题"
          resetPressed={!candidateTextColor(preferences.candidate_selected_color)}
          onChange={onChange}
        />
      )}
      {showSelectionAppearance && (
        <ColorControl
          preferences={preferences}
          previewTheme={previewTheme}
          preferenceKey="candidate_hover_color"
          label="候选悬停色"
          fallback={{ light: "#ececec", dark: "#414141" }}
          resetAriaLabel="候选悬停色跟随主题"
          resetPressed={!candidateTextColor(preferences.candidate_hover_color)}
          onChange={onChange}
        />
      )}
      <ColorControl
        preferences={preferences}
        previewTheme={previewTheme}
        preferenceKey="candidate_surface_color"
        label="候选表面色"
        fallback={{ light: "#ffffff", dark: "#202020" }}
        resetAriaLabel="候选表面色跟随主题"
        onChange={onChange}
      />
      {showBorderColor && (
        <ColorControl
          preferences={preferences}
          previewTheme={previewTheme}
          preferenceKey="candidate_border_color"
          label="候选边框色"
          fallback={{ light: "#dedede", dark: "#303030" }}
          resetAriaLabel="候选边框色跟随主题"
          onChange={onChange}
        />
      )}
      <ColorControl
        preferences={preferences}
        previewTheme={previewTheme}
        preferenceKey="candidate_number_color"
        label="候选编号颜色"
        fallback={{ light: "#5f6368", dark: "#bdc1c6" }}
        resetAriaLabel="候选编号颜色跟随主题"
        resetPressed={!candidateTextColor(preferences.candidate_number_color)}
        note={linux ? linuxFcitxClassicColorNote : undefined}
        onChange={onChange}
      />
    </>
  );
}
