import { validModelMirror } from "../voice/local-models";
import { TextInputRow } from "./text-input-row";

export interface VoiceModelMirrorSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** 本地模型下载镜像这一行；语音页把它放进「识别服务配置」组的「更多选项」。 */
export function VoiceModelMirrorRow({ value, onChange }: VoiceModelMirrorSectionProps) {
  return (
    <TextInputRow
      title="模型下载镜像"
      description="可选。以 https:// 开头的加速前缀，下载地址为“镜像/原始地址”；留空直接从 GitHub 下载"
      label="模型下载镜像"
      maxLength={2048}
      value={value}
      placeholder="https://mirror.example.com"
      aria-invalid={!validModelMirror(value.trim())}
      onChange={onChange}
    />
  );
}
