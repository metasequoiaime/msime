export type ImeModeScope = "app" | "global";

export interface ImeModeScopeSectionProps {
  value?: ImeModeScope;
  onChange: (value: ImeModeScope) => void;
}

/** Input-mode state scope selector for hosts that remember the mode per context. */
export function ImeModeScopeSection({ value, onChange }: ImeModeScopeSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          中英文状态<small>按应用分别记忆输入状态，或让所有输入上下文保持同一状态</small>
        </span>
        <select
          aria-label="中英文状态"
          value={value ?? "app"}
          onChange={(event) => onChange(event.target.value as ImeModeScope)}
        >
          <option value="app">按应用记忆</option>
          <option value="global">全局统一</option>
        </select>
      </label>
    </div>
  );
}
