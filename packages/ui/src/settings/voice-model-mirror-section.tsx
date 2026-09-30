import { validModelMirror } from "../voice/local-models";
import { GroupList, Row } from "../core/platform-controls";

export interface VoiceModelMirrorSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Optional mirror prefix used when downloading local voice models: the 模型下载 group. */
export function VoiceModelMirrorSection({ value, onChange }: VoiceModelMirrorSectionProps) {
  return (
    <GroupList title="模型下载">
      <Row
        title="模型下载镜像"
        description="可选。以 https:// 开头的加速前缀，下载地址为“镜像/原始地址”；留空直接从 GitHub 下载"
      >
        <input
          aria-label="模型下载镜像"
          maxLength={2048}
          value={value}
          placeholder="https://mirror.example.com"
          aria-invalid={!validModelMirror(value.trim())}
          onChange={(event) => onChange(event.target.value)}
        />
      </Row>
    </GroupList>
  );
}
