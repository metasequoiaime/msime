import * as skin from "../keyboard/touch-skin-style";

export interface TouchKeyboardSchemesSectionProps {
  options: readonly (readonly [string, string])[];
  enabled: readonly string[];
  selected: string;
  onSelect: (scheme: string) => void;
  onToggle: (scheme: string, enabled: boolean) => void;
}

/** Touch keyboard scheme choices and visibility switches. */
export function TouchKeyboardSchemesSection({
  options,
  enabled,
  selected,
  onSelect,
  onToggle,
}: TouchKeyboardSchemesSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="touch-keyboard-schemes-title">
      <div className="section-title" id="touch-keyboard-schemes-title">
        输入方案
      </div>
      <div className="input-setting-description">
        开启的方案会显示在键盘快捷切换中，至少保留一种。点击名称设为当前方案。
      </div>
      <div className="input-option-content">
        {options.map(([scheme, label], index) => {
          const isEnabled = enabled.includes(scheme);
          const isSelected = selected === scheme;
          return (
            <div className="input-option-item" key={scheme}>
              {index > 0 && <div className="input-option-divider" />}
              <div className={skin.schemeRow}>
                <button
                  type="button"
                  className={skin.schemeSelect(isSelected)}
                  aria-label={`设为当前输入方案 ${label}`}
                  aria-pressed={isSelected}
                  disabled={!isEnabled}
                  onClick={() => onSelect(scheme)}
                >
                  <span>{label}</span>
                  {isSelected && <span aria-hidden="true">✓</span>}
                </button>
                <input
                  className="toggle"
                  type="checkbox"
                  aria-label={`显示输入方案 ${label}`}
                  checked={isEnabled}
                  disabled={isEnabled && enabled.length === 1}
                  onChange={(event) => onToggle(scheme, event.target.checked)}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
