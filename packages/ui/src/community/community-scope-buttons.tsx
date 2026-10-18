import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export interface CommunityScopeButtonsProps {
  ariaLabel: string;
  mineOnly: boolean;
  allLabel: string;
  mineLabel: string;
  onMineOnlyChange: (mineOnly: boolean) => void;
  className?: string;
}

/** 社区皮肤图库共用的两项范围切换，可按所在工具栏调整外观。 */
export function CommunityScopeButtons({
  ariaLabel,
  mineOnly,
  allLabel,
  mineLabel,
  onMineOnlyChange,
  className = style.scopeButtons,
}: CommunityScopeButtonsProps) {
  return (
    <div className={className} role="group" aria-label={ariaLabel}>
      <ActionButton
        action={() => {
          if (mineOnly) onMineOnlyChange(false);
        }}
        className={mineOnly ? "secondary" : "primary"}
        ariaPressed={!mineOnly}
        label={allLabel}
      />
      <ActionButton
        action={() => {
          if (!mineOnly) onMineOnlyChange(true);
        }}
        className={mineOnly ? "primary" : "secondary"}
        ariaPressed={mineOnly}
        label={mineLabel}
      />
    </div>
  );
}
