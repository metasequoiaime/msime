export interface VoiceEndpointSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Endpoint override for a remote voice recognition provider. */
export function VoiceEndpointSection({ value, onChange }: VoiceEndpointSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          识别接口地址<small>留空使用当前 provider 默认地址</small>
        </span>
        <input
          aria-label="识别接口地址"
          type="url"
          value={value}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
    </div>
  );
}
