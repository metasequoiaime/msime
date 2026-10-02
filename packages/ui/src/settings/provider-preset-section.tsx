import { ModelSettingField } from "./model-setting-field";
import { ActionButton } from "./action-button";

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
        <ModelSettingField
          label="预置模型"
          inputLabel={`${label}预置模型`}
          description="服务商已知支持的模型；也可以在模型框中自行填写"
          models={models}
          model={model}
          emptyLabel="自定义模型…"
          onSelect={onSelectModel}
        />
      )}
      {linkable && (
        <ActionButton
          action={() => openExternalUrl?.(documentation!)}
          label={`${label}接入说明与 API Key`}
        />
      )}
    </div>
  );
}
