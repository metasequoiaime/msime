import type { ReactNode } from "react";
import * as style from "./community-style";

export interface CommunityRightsAgreementProps {
  agreementText: ReactNode;
  ariaLabel: string;
  checked: boolean;
  disabled?: boolean;
  checkboxClassName?: string;
  onChange: (agreed: boolean) => void;
}

/** Shared rights confirmation checkbox used by community publication dialogs. */
export function CommunityRightsAgreement({
  agreementText,
  ariaLabel,
  checked,
  disabled,
  checkboxClassName,
  onChange,
}: CommunityRightsAgreementProps) {
  return (
    <label className={style.agreement}>
      <input
        className={checkboxClassName}
        type="checkbox"
        aria-label={ariaLabel}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      {agreementText}
    </label>
  );
}
