export type InputSchemeSelectorValue = "quanpin" | "shuangpin" | "wubi";

export interface InputSchemeSelectorSectionProps {
  value: InputSchemeSelectorValue;
  onChange: (value: InputSchemeSelectorValue) => void;
}

const options: [InputSchemeSelectorValue, string][] = [
  ["quanpin", "全拼"],
  ["shuangpin", "双拼"],
  ["wubi", "五笔"],
];

/** Radio selector for the desktop Chinese input schemes. */
export function InputSchemeSelectorSection({ value, onChange }: InputSchemeSelectorSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="input-scheme-title">
      <div className="section-title" id="input-scheme-title">
        输入方案
      </div>
      <div className="input-option-content">
        {options.map(([scheme, label], index) => (
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
