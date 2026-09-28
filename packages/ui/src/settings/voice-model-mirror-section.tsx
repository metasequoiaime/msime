import { validModelMirror } from "../voice/local-models";

export interface VoiceModelMirrorSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Optional mirror prefix used when downloading local voice models. */
export function VoiceModelMirrorSection({ value, onChange }: VoiceModelMirrorSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          模型下载镜像
          <small>
            可选。以 https:// 开头的加速前缀，下载地址为“镜像/原始地址”；留空直接从 GitHub
            下载。保存设置后生效
          </small>
        </span>
        <input
          aria-label="模型下载镜像"
          maxLength={2048}
          value={value}
          placeholder="https://mirror.example.com"
          aria-invalid={!validModelMirror(value.trim())}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
    </div>
  );
}
