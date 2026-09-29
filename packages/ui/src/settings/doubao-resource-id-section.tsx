import { Row } from "../core/platform-controls";

export interface DoubaoResourceIdSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Doubao resource identifier used by the selected voice provider. */
export function DoubaoResourceIdSection({ value, onChange }: DoubaoResourceIdSectionProps) {
  return (
    <Row title="Doubao 资源 ID" description="仅由 Doubao provider 使用">
      <input
        aria-label="Doubao 资源 ID"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </Row>
  );
}
