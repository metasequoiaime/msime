export interface VoiceStreamPreeditSectionProps {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}

/** Live recognition fragment display toggle for providers that support streaming. */
export function VoiceStreamPreeditSection({ enabled, onChange }: VoiceStreamPreeditSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          流式预编辑<small>provider 支持时显示实时识别片段</small>
        </span>
        <input
          aria-label="流式预编辑"
          className="toggle"
          type="checkbox"
          checked={enabled}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
