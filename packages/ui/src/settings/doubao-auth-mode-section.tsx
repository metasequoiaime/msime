import { Row, Select } from "../core/platform-controls";

export type DoubaoAuthMode = "api_key" | "legacy";

export interface DoubaoAuthModeSectionProps {
  value: DoubaoAuthMode;
  linux: boolean;
  onChange: (value: DoubaoAuthMode) => void;
}

/** Doubao authentication mode selector shared by desktop and provider hosts. */
export function DoubaoAuthModeSection({ value, linux, onChange }: DoubaoAuthModeSectionProps) {
  return (
    <Row
      title="豆包鉴权方式"
      description={
        linux
          ? "provider 服务必须与此模式匹配"
          : "新版控制台使用单 API Key；旧版使用 App ID + Access Token"
      }
    >
      <Select
        aria-label="豆包鉴权方式"
        value={value}
        onChange={(event) => onChange(event.target.value === "legacy" ? "legacy" : "api_key")}
      >
        <option value="api_key">新版 API Key</option>
        <option value="legacy">旧版 App ID + Access Token</option>
      </Select>
    </Row>
  );
}
