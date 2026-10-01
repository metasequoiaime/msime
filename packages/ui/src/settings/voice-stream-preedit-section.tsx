import { SwitchRow } from "./switch-row";

export interface VoiceStreamPreeditSectionProps {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}

/** Live recognition fragment display toggle for providers that support streaming. */
export function VoiceStreamPreeditSection({ enabled, onChange }: VoiceStreamPreeditSectionProps) {
  return (
    <SwitchRow
      title="流式预编辑"
      description="provider 支持时显示实时识别片段"
      aria-label="流式预编辑"
      checked={enabled}
      onChange={onChange}
    />
  );
}
