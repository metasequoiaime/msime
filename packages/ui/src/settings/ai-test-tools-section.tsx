import { GroupList } from "../core/platform-controls";
import { SettingsTextareaField } from "./settings-textarea-field";
import * as settings from "./settings-style";
import { ActionButton } from "./action-button";

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
      {status && <p role="status">{status}</p>}
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
      <div className={settings.managerBlock}>
        <SettingsTextareaField
          label="AI 润色测试"
          description="仅在点击发送时请求当前配置；测试文字不会写入日志。"
          ariaLabel="AI 测试输入"
          placeholder="输入一段待润色文字"
          value={input}
          onChange={onInputChange}
        />
        <div className={settings.managerActions}>
          <ActionButton
            action={onTest}
            disabled={busy || !input.trim()}
            label={busy ? "发送中…" : "发送并润色"}
          />
        </div>
        {result}
      </div>
    </GroupList>
  );
}
