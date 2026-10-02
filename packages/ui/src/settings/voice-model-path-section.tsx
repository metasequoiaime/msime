import { Row } from "../core/platform-controls";
import { SettingField } from "./setting-field";

export interface VoiceModelPathSectionProps {
  path: string;
  pickPath?: () => Promise<string | null>;
  onChange: (path: string) => void;
  /** 画成组内的一行，供语音页的「识别服务配置」组使用；不传时仍是旧面板的独立卡片。 */
  grouped?: boolean;
}

/** Manual model directory input shared by voice hosts with and without a model store. */
export function VoiceModelPathSection({
  path,
  pickPath,
  onChange,
  grouped = false,
}: VoiceModelPathSectionProps) {
  const description = "已安装模型所在文件夹的绝对路径（包含 msime-model.json）";
  const editor = (
    <span className="flex items-center gap-2 [&>input]:min-w-0 [&>input]:flex-1">
      <input
        aria-label="本地模型目录"
        value={path}
        placeholder="/path/to/voice-models/<model>"
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
  );
  return grouped ? (
    <Row title="本地模型目录" description={description}>
      {editor}
    </Row>
  ) : (
    <div className="section">
      <SettingField label="本地模型目录" description={description}>
        {editor}
      </SettingField>
    </div>
  );
}
