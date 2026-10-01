import { TextInputRow } from "./text-input-row";

export interface DoubaoResourceIdSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Doubao resource identifier used by the selected voice provider. */
export function DoubaoResourceIdSection({ value, onChange }: DoubaoResourceIdSectionProps) {
  return (
    <TextInputRow
      title="Doubao 资源 ID"
      description="仅由 Doubao provider 使用"
      label="Doubao 资源 ID"
      value={value}
      onChange={onChange}
    />
  );
}
