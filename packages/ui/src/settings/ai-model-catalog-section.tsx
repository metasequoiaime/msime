import { ModelSelect } from "./model-select";
import { SettingField } from "./setting-field";

export function AiModelCatalogSection({
  busy,
  origin,
  models,
  selectedModel,
  status,
  onFetch,
  onSelect,
}: {
  busy: boolean;
  origin: string;
  models?: readonly string[];
  selectedModel: string;
  status?: string;
  onFetch: () => void;
  onSelect: (model: string) => void;
}) {
  return (
    <div className="section">
      <div className="section-header">
        <span className="section-title">
          服务模型
          <small>从当前服务的模型目录读取；服务不支持时可继续手动填写模型。</small>
        </span>
        <button type="button" className="secondary" disabled={busy || !origin} onClick={onFetch}>
          {busy ? "获取中…" : "获取模型列表"}
        </button>
      </div>
      {models && models.length > 0 && (
        <SettingField label="已获取模型">
          <ModelSelect
            models={models}
            model={selectedModel}
            ariaLabel="已获取的 AI 模型"
            emptyLabel="选择模型…"
            onSelect={onSelect}
          />
        </SettingField>
      )}
      {status && <p role="status">{status}</p>}
    </div>
  );
}
