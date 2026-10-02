import * as skin from "../keyboard/touch-skin-style";
import * as settings from "./settings-style";
import { Row } from "../core/platform-controls";
import { ActionButton } from "../core/action-button";
import { SwitchRow } from "./switch-row";

export interface TouchKeyboardSchemesSectionProps {
  options: readonly (readonly [string, string])[];
  enabled: readonly string[];
  selected: string;
  onSelect: (scheme: string) => void;
  onToggle: (scheme: string, enabled: boolean) => void;
}

/** Touch keyboard scheme choices and visibility switches, as rows of the 输入 page's 方案 group. */
export function TouchKeyboardSchemesSection({
  options,
  enabled,
  selected,
  onSelect,
  onToggle,
}: TouchKeyboardSchemesSectionProps) {
  return (
    <div role="group" aria-label="输入方案" className={settings.rowStack}>
      <Row
        title="输入方案"
        description="开启的方案会显示在键盘快捷切换中，至少保留一种。点击名称设为当前方案。"
      />
      {options.map(([scheme, label]) => {
        const isEnabled = enabled.includes(scheme);
        const isSelected = selected === scheme;
        return (
          <SwitchRow
            key={scheme}
            title={
              <ActionButton
                action={() => onSelect(scheme)}
                ariaLabel={`设为当前输入方案 ${label}`}
                ariaPressed={isSelected}
                className={skin.schemeSelect(isSelected)}
                disabled={!isEnabled}
                label={
                  <>
                    <span>{label}</span>
                    {isSelected && <span aria-hidden="true">✓</span>}
                  </>
                }
              />
            }
            aria-label={`显示输入方案 ${label}`}
            checked={isEnabled}
            disabled={isEnabled && enabled.length === 1}
            onChange={(checked) => onToggle(scheme, checked)}
          />
        );
      })}
    </div>
  );
}
