import { useRef } from "react";
import { resourceScopeTitle } from "./community-helpers";
import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

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
  const menuRef = useRef<HTMLDetailsElement>(null);
  return (
    <>
      <div
        className={style.scopeButtonsCollapsing}
        role="group"
        aria-label={`${resourceLabel}范围`}
      >
        {options.map((option) => (
          <ActionButton
            key={option.scope || "all"}
            action={() => onScopeChange(option.scope)}
            className={scope === option.scope ? "primary" : "secondary"}
            ariaPressed={scope === option.scope}
            label={option.label}
          />
        ))}
      </div>
      <details ref={menuRef} className={style.scopeMenu}>
        <summary className={style.scopeMenuSummary} aria-label={`${resourceLabel}筛选范围`}>
          {resourceScopeTitle(scope)}
        </summary>
        <div className={style.scopeMenuList} role="group" aria-label={`${resourceLabel}筛选范围`}>
          {options.map((option) => (
            <ActionButton
              key={option.scope || "all"}
              action={() => {
                onScopeChange(option.scope);
                menuRef.current?.removeAttribute("open");
              }}
              className={style.scopeMenuItem}
              ariaLabel={`筛选范围：${option.label}`}
              ariaPressed={scope === option.scope}
              label={option.label}
            />
          ))}
        </div>
      </details>
    </>
  );
}
