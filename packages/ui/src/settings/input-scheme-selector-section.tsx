import { Row, Segmented } from "../core/platform-controls";
import { chineseInputSchemeOptions, type ChineseInputScheme } from "./input-scheme-options";

export type InputSchemeSelectorValue = ChineseInputScheme;

export interface InputSchemeSelectorSectionProps {
  value: InputSchemeSelectorValue;
  onChange: (value: InputSchemeSelectorValue) => void;
  /** Uses the shared settings-page row instead of the legacy panel markup. */
  grouped?: boolean;
  hidden?: boolean;
}

/** Radio selector for the desktop Chinese input schemes. */
export function InputSchemeSelectorSection({
  value,
  onChange,
  grouped = false,
  hidden,
}: InputSchemeSelectorSectionProps) {
  if (grouped) {
    return (
      <Row title="输入方案" hidden={hidden}>
        <Segmented options={chineseInputSchemeOptions} value={value} onChange={onChange} />
      </Row>
    );
  }

  return (
    <div className="section" role="group" aria-labelledby="input-scheme-title" hidden={hidden}>
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
