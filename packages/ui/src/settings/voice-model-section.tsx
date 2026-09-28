export interface VoiceModelSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Provider-selected speech recognition model field. */
export function VoiceModelSection({ value, onChange }: VoiceModelSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          识别模型<small>由 provider 服务选择对应模型</small>
        </span>
        <input
          aria-label="识别模型"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
    </div>
  );
}
