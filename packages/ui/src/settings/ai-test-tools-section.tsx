import { GroupList } from "../core/platform-controls";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SettingTextarea } from "./setting-textarea";
import * as settings from "./settings-style";

export function AiTestToolsSection({
  input,
  busy,
  status,
  output,
  onInputChange,
  onTest,
  onCopyOutput,
  grouped = false,
}: {
  input: string;
  busy: boolean;
  status: string;
  output: string;
  onInputChange: (input: string) => void;
  onTest: () => void;
  onCopyOutput?: () => void;
  /** 画成 AI 辅助页的「测试工具」组；不传时仍是旧面板的卡片。 */
  grouped?: boolean;
}) {
  const result = (
    <>
      {status && <p role="status">{status}</p>}
      {output && (
        <div className="ai-test-result">
          <div>{output}</div>
          {onCopyOutput && (
            <button type="button" className="secondary" onClick={onCopyOutput}>
              复制结果
            </button>
          )}
        </div>
      )}
    </>
  );
  if (grouped) {
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
            <button
              type="button"
              className="secondary"
              disabled={busy || !input.trim()}
              onClick={onTest}
            >
              {busy ? "发送中…" : "发送并润色"}
            </button>
          </div>
          {result}
        </div>
      </GroupList>
    );
  }
  return (
    <SettingTextarea
      sectionClassName="section ai-test-tools"
      titleElement="div"
      label="AI 润色测试"
      description="仅在点击发送时请求当前配置；测试文字不会写入日志。"
      ariaLabel="AI 测试输入"
      placeholder="输入一段待润色文字"
      value={input}
      onChange={onInputChange}
    >
      <button type="button" className="secondary" disabled={busy || !input.trim()} onClick={onTest}>
        {busy ? "发送中…" : "发送并润色"}
      </button>
      {result}
    </SettingTextarea>
  );
}
