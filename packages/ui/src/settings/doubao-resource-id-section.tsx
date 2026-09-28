export interface DoubaoResourceIdSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Doubao resource identifier used by the selected voice provider. */
export function DoubaoResourceIdSection({ value, onChange }: DoubaoResourceIdSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          Doubao 资源 ID<small>仅由 Doubao provider 使用</small>
        </span>
        <input
          aria-label="Doubao 资源 ID"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
    </div>
  );
}
