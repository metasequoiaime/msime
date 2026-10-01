import { clamp } from "../core/number";
import { SettingField } from "./setting-field";

export function AiCandidateLimitSection({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="section">
      <SettingField label="候选数量">
        <input
          aria-label="AI 候选数量"
          type="number"
          min="1"
          max="10"
          value={value}
          onChange={(event) => onChange(clamp(Number(event.target.value) || 3, 1, 10))}
        />
      </SettingField>
    </div>
  );
}
