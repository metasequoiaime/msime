import * as style from "./community-style";
import { CommunityPublicationMetadataFields } from "./community-publication-metadata-fields";

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
    <CommunityPublicationMetadataFields
      name={name}
      description={description}
      agreed={agreed}
      busy={busy}
      nameLabel="皮肤名称"
      nameAriaLabel="发布皮肤名称"
      descriptionLabel="设计说明"
      descriptionAriaLabel="发布设计说明"
      agreementText={agreementText}
      agreementAriaLabel="确认拥有发布素材权利"
      agreementClassName={style.agreementBox}
      onNameChange={onNameChange}
      onDescriptionChange={onDescriptionChange}
      onAgreedChange={onAgreedChange}
    />
  );
}
