import {
  chineseInputSchemeOptions,
  type ChineseInputScheme,
} from "./input-scheme-options";

export type InputSchemeSelectorValue = ChineseInputScheme;

export interface InputSchemeSelectorSectionProps {
  value: InputSchemeSelectorValue;
  onChange: (value: InputSchemeSelectorValue) => void;
}

/** Radio selector for the desktop Chinese input schemes. */
export function InputSchemeSelectorSection({ value, onChange }: InputSchemeSelectorSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="input-scheme-title">
      <div className="section-title" id="input-scheme-title">
        输入方案
      </div>
      <div className="input-option-content">
        {chineseInputSchemeOptions.map(({ value: scheme, label }, index) => (
          <div className="input-option-item" key={scheme}>
            {index > 0 && <div className="input-option-divider" />}
            <label className="radio-option">
              <input
                type="radio"
                name="input-scheme"
                value={scheme}
                checked={value === scheme}
                onChange={() => onChange(scheme)}
              />
              <span>{label}</span>
            </label>
          </div>
        ))}
      </div>
    </div>
  );
}
