import { VoiceModelPathSection } from "./voice-model-path-section";

export interface VoiceModelPathDisclosureProps {
  disclosure: boolean;
  path: string;
  pickPath?: () => Promise<string | null>;
  onChange: (path: string) => void;
}

/** Adds the advanced disclosure used when a host also provides a model store. */
export function VoiceModelPathDisclosure({
  disclosure,
  path,
  pickPath,
  onChange,
}: VoiceModelPathDisclosureProps) {
  const editor = <VoiceModelPathSection path={path} pickPath={pickPath} onChange={onChange} />;
  return disclosure ? (
    <details className="section">
      <summary>高级：手动指定 Whisper 模型文件</summary>
      {editor}
    </details>
  ) : (
    editor
  );
}
