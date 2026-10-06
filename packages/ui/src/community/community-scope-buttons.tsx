import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

export interface CommunityScopeButtonsProps {
  ariaLabel: string;
  mineOnly: boolean;
  allLabel: string;
  mineLabel: string;
  onMineOnlyChange: (mineOnly: boolean) => void;
}

/** Two-option scope switch shared by the community skin galleries. */
export function CommunityScopeButtons({
  ariaLabel,
  mineOnly,
  allLabel,
  mineLabel,
  onMineOnlyChange,
}: CommunityScopeButtonsProps) {
  return (
    <div className={style.scopeButtons} role="group" aria-label={ariaLabel}>
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
