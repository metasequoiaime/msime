export interface VoiceModelPathSectionProps {
  path: string;
  pickPath?: () => Promise<string | null>;
  onChange: (path: string) => void;
}

/** Manual Whisper model path input shared by voice hosts with and without a model store. */
export function VoiceModelPathSection({ path, pickPath, onChange }: VoiceModelPathSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          Whisper 模型文件
          <small>ggml 模型的绝对路径，例如 /Users/you/models/ggml-large-v3-turbo.bin</small>
        </span>
        <span className="flex items-center gap-2 [&>input]:min-w-0 [&>input]:flex-1">
          <input
            aria-label="Whisper 模型文件"
            value={path}
            placeholder="/path/to/ggml-model.bin"
            onChange={(event) => onChange(event.target.value)}
          />
          {pickPath && (
            <button
              type="button"
              className="secondary"
              onClick={() => {
                void (async () => {
                  // Cancelling resolves to null and must leave the field as it was, rather than
                  // clearing a path that already worked.
                  const chosen = await pickPath();
                  if (chosen) onChange(chosen);
                })();
              }}
            >
              选择…
            </button>
          )}
        </span>
      </label>
    </div>
  );
}
