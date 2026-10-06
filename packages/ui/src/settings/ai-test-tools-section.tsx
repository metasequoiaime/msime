import { GroupList } from "../core/platform-controls";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { ActionButton } from "./action-button";
import { StatusMessage } from "../core/status-message";

export function AiTestToolsSection({
  input,
  busy,
  status,
  output,
  onInputChange,
  onTest,
  onCopyOutput,
}: {
  input: string;
  busy: boolean;
  status: string;
  output: string;
  onInputChange: (input: string) => void;
  onTest: () => void;
  onCopyOutput?: () => void;
}) {
  const result = (
    <>
      {status && <StatusMessage role="status">{status}</StatusMessage>}
      {output && (
        <div className="ai-test-result">
          <div>{output}</div>
          {onCopyOutput && <ActionButton action={onCopyOutput} label="复制结果" />}
        </div>
      )}
    </>
  );
  return (
    <GroupList title="测试工具">
      <SettingsManagerBlock>
        <SettingsTextareaField
          label="AI 润色测试"
          description="仅在点击发送时请求当前配置；测试文字不会写入日志。"
          ariaLabel="AI 测试输入"
          placeholder="输入一段待润色文字"
          value={input}
          onChange={onInputChange}
        />
        <SettingsManagerActions>
          <ActionButton
            action={onTest}
            disabled={busy || !input.trim()}
            label={busy ? "发送中…" : "发送并润色"}
          />
        </SettingsManagerActions>
        {result}
      </SettingsManagerBlock>
    </GroupList>
  );
}
