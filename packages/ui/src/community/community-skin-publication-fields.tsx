import * as style from "./community-style";
import { CommunityRightsAgreement } from "./community-rights-agreement";
import { CommunityInputField } from "./community-input-field";
import { CommunityTextareaField } from "./community-textarea-field";

export interface CommunitySkinPublicationFieldsProps {
  name: string;
  description: string;
  agreed: boolean;
  busy?: boolean;
  agreementText: string;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onAgreedChange: (agreed: boolean) => void;
}

/** Shared name, description, and rights fields used by skin publication dialogs. */
export function CommunitySkinPublicationFields({
  name,
  description,
  agreed,
  busy = false,
  agreementText,
  onNameChange,
  onDescriptionChange,
  onAgreedChange,
}: CommunitySkinPublicationFieldsProps) {
  return (
    <>
      <CommunityInputField
        label="皮肤名称"
        ariaLabel="发布皮肤名称"
        maxLength={32}
        value={name}
        disabled={busy}
        onChange={onNameChange}
      />
      <CommunityTextareaField
        label="设计说明"
        ariaLabel="发布设计说明"
        maxLength={280}
        rows={4}
        value={description}
        disabled={busy}
        onChange={onDescriptionChange}
      />
      <CommunityRightsAgreement
        agreementText={agreementText}
        ariaLabel="确认拥有发布素材权利"
        checked={agreed}
        disabled={busy}
        checkboxClassName={style.agreementBox}
        onChange={onAgreedChange}
      />
    </>
  );
}
