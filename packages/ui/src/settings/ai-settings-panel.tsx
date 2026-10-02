import { AiSettingsContent, type AiSettingsContentProps } from "./ai-settings-content";
import { McpConnectSection } from "./mcp-connect";

export type AiSettingsPanelProps = Omit<AiSettingsContentProps, "trailing">;

/** Embedded AI settings adapter; the shared content owns the controls and provider logic. */
export function AiSettingsPanel(props: AiSettingsPanelProps) {
  const { client } = props;
  return (
    <AiSettingsContent
      {...props}
      trailing={
        client.mcpServerStatus ? (
          <McpConnectSection
            status={client.mcpServerStatus}
            install={client.installMcpClient}
            copyText={client.copyText}
          />
        ) : null
      }
    />
  );
}
