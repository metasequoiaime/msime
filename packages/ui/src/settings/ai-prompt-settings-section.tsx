export function AiPromptSettingsSection({
  promptId,
  prompt,
  promptCustom1,
  promptCustom2,
  promptCustom3,
  fallbackPrompt,
  onPromptIdChange,
  onPromptChange,
  onPromptCustom1Change,
  onPromptCustom2Change,
  onPromptCustom3Change,
}: {
  promptId?: string;
  prompt?: string;
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  fallbackPrompt: string;
  onPromptIdChange: (value: string) => void;
  onPromptChange: (value: string) => void;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
}) {
  return (
    <>
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            AI 联想提示词方案
            <small>使用选中的独立槽位；槽位留空时使用兼容提示词</small>
          </span>
          <select
            aria-label="AI 联想提示词方案"
            value={promptId === "custom" ? "custom_1" : promptId || "custom_1"}
            onChange={(event) => onPromptIdChange(event.target.value)}
          >
            <option value="custom_1">自定义一</option>
            <option value="custom_2">自定义二</option>
            <option value="custom_3">自定义三</option>
          </select>
        </label>
      </div>
      <div className="section">
        <label className="section-title">
          兼容提示词<small>旧版提示词，所选自定义槽位留空时使用</small>
        </label>
        <textarea
          aria-label="AI 润色提示词"
          placeholder="留空时使用内置的联想提示词"
          value={prompt ?? fallbackPrompt}
          onChange={(event) => onPromptChange(event.target.value)}
        />
      </div>
      <div className="section">
        <label className="section-title">
          自定义提示词一<small>发送给 AI 联想服务的额外提示词</small>
        </label>
        <textarea
          aria-label="自定义提示词一"
          value={promptCustom1}
          onChange={(event) => onPromptCustom1Change(event.target.value)}
        />
      </div>
      <div className="section">
        <label className="section-title">自定义提示词二</label>
        <textarea
          aria-label="自定义提示词二"
          value={promptCustom2}
          onChange={(event) => onPromptCustom2Change(event.target.value)}
        />
      </div>
      <div className="section">
        <label className="section-title">自定义提示词三</label>
        <textarea
          aria-label="自定义提示词三"
          value={promptCustom3}
          onChange={(event) => onPromptCustom3Change(event.target.value)}
        />
      </div>
    </>
  );
}
