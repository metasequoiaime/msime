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
    </div>
  );
}
