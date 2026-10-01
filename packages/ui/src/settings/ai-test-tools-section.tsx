import { GroupList } from "../core/platform-controls";
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
          <label className={settings.field}>
            <span>
              <span data-row-title="">AI 润色测试</span>{" "}
              仅在点击发送时请求当前配置；测试文字不会写入日志。
            </span>
            <textarea
              aria-label="AI 测试输入"
              className={settings.promptInput}
              placeholder="输入一段待润色文字"
              value={input}
              onChange={(event) => onInputChange(event.target.value)}
            />
          </label>
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
    <div className="section ai-test-tools">
      <div className="section-title">
        AI 润色测试
        <small>仅在点击发送时请求当前配置；测试文字不会写入日志。</small>
      </div>
      <textarea
        aria-label="AI 测试输入"
        placeholder="输入一段待润色文字"
        value={input}
        onChange={(event) => onInputChange(event.target.value)}
      />
      <button type="button" className="secondary" disabled={busy || !input.trim()} onClick={onTest}>
        {busy ? "发送中…" : "发送并润色"}
      </button>
      {result}
    </div>
  );
}
