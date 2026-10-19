import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 桌面和触屏宿主共用 MCP 连接内容。 */
export function McpSettingsContent() {
  const { client } = useSettingsForm();
  if (!client.mcpServerStatus) return null;
  return (
    <McpConnectSection
      status={client.mcpServerStatus}
      install={client.installMcpClient}
      copyText={client.copyText}
    />
  );
}

/** 触屏宿主保留独立连接页，桌面在 AI 页的顶部标签里复用连接内容。 */
export function McpSettingsPage() {
  const { client, busy, page, mobilePlatform } = useSettingsForm();
  if (!mobilePlatform || !client.mcpServerStatus) return null;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "mcp"} ariaLabel="AI 助手">
      <McpSettingsContent />
    </SettingsPageFieldset>
  );
}
