export type InputModeScheme = "quanpin" | "shuangpin" | "wubi" | "japanese";

export interface InputModeSectionProps {
  scheme: InputModeScheme;
  lastChineseScheme?: Exclude<InputModeScheme, "japanese"> | null;
  onChange: (
    patch: Partial<{
      scheme: InputModeScheme;
      last_chinese_scheme: Exclude<InputModeScheme, "japanese"> | null;
    }>,
  ) => void;
}

/** Chinese/Japanese input mode selector with remembered Chinese scheme. */
export function InputModeSection({ scheme, lastChineseScheme, onChange }: InputModeSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="input-mode-title">
      <div className="section-title" id="input-mode-title">
        输入模式
      </div>
      <div className="input-setting-description">
        切换中文或日文输入，并保留各模式上次选择的方案
      </div>
      <div className="input-option-content input-mode-options">
        <label className="radio-option">
          <input
            type="radio"
            name="input-mode"
            value="chinese"
            aria-label="中文"
            checked={scheme !== "japanese"}
            onChange={() => onChange({ scheme: lastChineseScheme ?? "quanpin" })}
          />
          <span>中文</span>
        </label>
        <div className="input-option-divider" />
        <label className="radio-option">
          <input
            type="radio"
            name="input-mode"
            value="japanese"
            aria-label="日文"
            checked={scheme === "japanese"}
            onChange={() =>
              onChange({
                last_chinese_scheme: scheme === "japanese" ? lastChineseScheme : scheme,
                scheme: "japanese",
              })
            }
          />
          <span>日文</span>
        </label>
      </div>
    </div>
  );
}
