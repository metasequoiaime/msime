export interface ModelSelectProps {
  models: readonly string[];
  model: string;
  ariaLabel: string;
  emptyLabel: string;
  onSelect: (model: string) => void;
}

/** Selects a known model while leaving custom values editable in the companion input. */
export function ModelSelect({ models, model, ariaLabel, emptyLabel, onSelect }: ModelSelectProps) {
  return (
    <select
      aria-label={ariaLabel}
      value={models.includes(model) ? model : ""}
      onChange={(event) => {
        if (event.target.value) onSelect(event.target.value);
      }}
    >
      <option value="">{emptyLabel}</option>
      {models.map((entry) => (
        <option key={entry} value={entry}>
          {entry}
        </option>
      ))}
    </select>
  );
}
