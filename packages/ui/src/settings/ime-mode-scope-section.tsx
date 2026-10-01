import { SelectRow } from "./select-row";

export type ImeModeScope = "app" | "global";

export interface ImeModeScopeSectionProps {
  value?: ImeModeScope;
  onChange: (value: ImeModeScope) => void;
}

/** Input-mode state scope selector for hosts that remember the mode per context: one row of the 中英文 group. */
export function ImeModeScopeSection({ value, onChange }: ImeModeScopeSectionProps) {
  return (
    <SelectRow
      title="中英文状态"
      description="按应用分别记忆输入状态，或让所有输入上下文保持同一状态"
      value={value ?? "app"}
      onChange={(event) => onChange(event.target.value as ImeModeScope)}
    >
      <option value="app">按应用记忆</option>
      <option value="global">全局统一</option>
    </SelectRow>
  );
}
