import { SwitchRow } from "./switch-row";

export interface SingleCharacterOnlySectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「只出单字」开关：输入页「候选与联想」组里的一行，缺省为关，与共享偏好的默认值一致。 */
export function SingleCharacterOnlySection({ value, onChange }: SingleCharacterOnlySectionProps) {
  return (
    <SwitchRow
      title="只出单字"
      description="候选只列单个汉字，不出词组和整句；选一个字后接着拼下一个字"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
