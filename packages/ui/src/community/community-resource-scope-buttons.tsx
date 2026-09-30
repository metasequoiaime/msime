import { resourceScopeTitle } from "./community-helpers";
import * as style from "./community-style";

export type CommunityResourceScope = "" | "mine" | "saved";

export interface CommunityResourceScopeButtonsProps {
  resourceLabel: string;
  scope: CommunityResourceScope;
  onScopeChange: (scope: CommunityResourceScope) => void;
}

/** Three-option scope controls shared by community resource galleries. */
export function CommunityResourceScopeButtons({
  resourceLabel,
  scope,
  onScopeChange,
}: CommunityResourceScopeButtonsProps) {
  const options: { scope: CommunityResourceScope; label: string }[] = [
    { scope: "", label: "全部" },
    { scope: "saved", label: "收藏" },
    { scope: "mine", label: "我的作品" },
  ];
  return (
    <>
      <div
        className={style.scopeButtonsCollapsing}
        role="group"
        aria-label={`${resourceLabel}范围`}
      >
        {options.map((option) => (
          <button
            key={option.scope || "all"}
            type="button"
            className={scope === option.scope ? "primary" : "secondary"}
            aria-pressed={scope === option.scope}
            onClick={() => onScopeChange(option.scope)}
          >
            {option.label}
          </button>
        ))}
      </div>
      <details className={style.scopeMenu}>
        <summary className={style.scopeMenuSummary} aria-label={`${resourceLabel}筛选范围`}>
          {resourceScopeTitle(scope)}
        </summary>
        <div className={style.scopeMenuList} role="group" aria-label={`${resourceLabel}筛选范围`}>
          {options.map((option) => (
            <button
              key={option.scope || "all"}
              type="button"
              className={style.scopeMenuItem}
              aria-label={`筛选范围：${option.label}`}
              aria-pressed={scope === option.scope}
              onClick={(event) => {
                onScopeChange(option.scope);
                event.currentTarget.closest("details")?.removeAttribute("open");
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
      </details>
    </>
  );
}
