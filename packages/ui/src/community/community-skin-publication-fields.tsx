import * as style from "./community-style";

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
      <label className={style.field}>
        皮肤名称
        <input
          className={style.fieldControl}
          aria-label="发布皮肤名称"
          maxLength={32}
          value={name}
          disabled={busy}
          onChange={(event) => onNameChange(event.target.value)}
        />
      </label>
      <label className={style.field}>
        设计说明
        <textarea
          className={style.textArea}
          aria-label="发布设计说明"
          maxLength={280}
          rows={4}
          value={description}
          disabled={busy}
          onChange={(event) => onDescriptionChange(event.target.value)}
        />
      </label>
      <label className={style.agreement}>
        <input
          className={style.agreementBox}
          type="checkbox"
          aria-label="确认拥有发布素材权利"
          checked={agreed}
          disabled={busy}
          onChange={(event) => onAgreedChange(event.target.checked)}
        />
        {agreementText}
      </label>
    </>
  );
}
