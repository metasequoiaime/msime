import { ModelSelect } from "./model-select";

export interface ProviderPreset {
  models?: readonly string[];
  documentation?: string;
}

export interface ProviderPresetSectionProps {
  label: string;
  preset?: ProviderPreset;
  model: string;
  onSelectModel: (model: string) => void;
  openExternalUrl?: (url: string) => void | Promise<void>;
  className?: string;
}

/** Known provider models and the provider's API key documentation link. */
export function ProviderPresetSection({
  label,
  preset,
  model,
  onSelectModel,
  openExternalUrl,
  className = "section provider-preset-section",
}: ProviderPresetSectionProps) {
  const models = preset?.models ?? [];
  const documentation = preset?.documentation;
  const linkable = Boolean(documentation && openExternalUrl);
  if (models.length === 0 && !linkable) return null;
  return (
    <div className={className}>
      {models.length > 0 && (
        <label className="section-header">
          <span className="section-title">
            预置模型<small>服务商已知支持的模型；也可以在模型框中自行填写</small>
          </span>
          <ModelSelect
            models={models}
            model={model}
            ariaLabel={`${label}预置模型`}
            emptyLabel="自定义模型…"
            onSelect={onSelectModel}
          />
        </label>
      )}
      {linkable && (
        <button
          type="button"
          className="secondary"
          onClick={() => void openExternalUrl?.(documentation!)}
        >
          {label}接入说明与 API Key
        </button>
      )}
    </div>
  );
}
