import * as style from "./community-style";

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
      <button
        type="button"
        className={mineOnly ? "secondary" : "primary"}
        aria-pressed={!mineOnly}
        onClick={() => {
          if (mineOnly) onMineOnlyChange(false);
        }}
      >
        {allLabel}
      </button>
      <button
        type="button"
        className={mineOnly ? "primary" : "secondary"}
        aria-pressed={mineOnly}
        onClick={() => {
          if (!mineOnly) onMineOnlyChange(true);
        }}
      >
        {mineLabel}
      </button>
    </div>
  );
}
