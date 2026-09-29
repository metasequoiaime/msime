import { Row, Switch } from "../core/platform-controls";

export interface VoiceStreamPreeditSectionProps {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}

/** Live recognition fragment display toggle for providers that support streaming. */
export function VoiceStreamPreeditSection({ enabled, onChange }: VoiceStreamPreeditSectionProps) {
  return (
    <Row title="流式预编辑" description="provider 支持时显示实时识别片段">
      <Switch aria-label="流式预编辑" checked={enabled} onChange={onChange} />
    </Row>
  );
}
