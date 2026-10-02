import type { ReactNode } from "react";
import { CommunityRightsAgreement } from "./community-rights-agreement";
import { CommunityInputField } from "./community-input-field";
import { CommunityTextareaField } from "./community-textarea-field";

export interface CommunityPublicationMetadataFieldsProps {
  name: string;
  description: string;
  agreed: boolean;
  busy?: boolean;
  nameLabel: ReactNode;
  nameAriaLabel: string;
  descriptionLabel: ReactNode;
  descriptionAriaLabel: string;
  descriptionRows?: number;
  showAgreement?: boolean;
  agreementText: ReactNode;
  agreementAriaLabel?: string;
  agreementClassName?: string;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onAgreedChange: (agreed: boolean) => void;
}

/** Shared name, description, and publication-rights fields used by community dialogs. */
export function CommunityPublicationMetadataFields({
  name,
  description,
  agreed,
  busy = false,
  nameLabel,
  nameAriaLabel,
  descriptionLabel,
  descriptionAriaLabel,
  descriptionRows = 4,
  showAgreement = true,
  agreementText,
  agreementAriaLabel = "确认拥有发布内容权利",
  agreementClassName,
  onNameChange,
  onDescriptionChange,
  onAgreedChange,
}: CommunityPublicationMetadataFieldsProps) {
  return (
    <>
      <CommunityInputField
        label={nameLabel}
        ariaLabel={nameAriaLabel}
        maxLength={32}
        value={name}
        disabled={busy}
        onChange={onNameChange}
      />
      <CommunityTextareaField
        label={descriptionLabel}
        ariaLabel={descriptionAriaLabel}
        maxLength={280}
        rows={descriptionRows}
        value={description}
        disabled={busy}
        onChange={onDescriptionChange}
      />
      {showAgreement && (
        <CommunityRightsAgreement
          agreementText={agreementText}
          ariaLabel={agreementAriaLabel}
          checked={agreed}
          disabled={busy}
          checkboxClassName={agreementClassName}
          onChange={onAgreedChange}
        />
      )}
    </>
  );
}
