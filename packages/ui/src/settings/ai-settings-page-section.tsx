import type { ReactNode } from "react";
import { AiBasicSettingsSection, type AiProviderOption } from "./ai-basic-settings-section";
import { AiCandidateLimitSection } from "./ai-candidate-limit-section";
import { AiModelCatalogSection } from "./ai-model-catalog-section";
import { AiPromptSettingsSection } from "./ai-prompt-settings-section";

export interface AiSettingsPageSectionProps {
  disabled: boolean;
  hidden: boolean;
  enabled: boolean;
  enabledDescription: string;
  provider: string;
  providerOptions: readonly AiProviderOption[];
  model: string;
  endpoint: string;
  providerPreset: ReactNode;
  onEnabledChange: (enabled: boolean) => void;
  onProviderChange: (provider: string) => void;
  onModelChange: (model: string) => void;
  onEndpointChange: (endpoint: string) => void;
  credentialSection: ReactNode;
  desktopCredentialTest: ReactNode;
  modelCatalog: {
    busy: boolean;
    origin: string;
    models: string[] | undefined;
    status: string;
    onFetch: () => void;
    onSelect: (model: string) => void;
  } | null;
  candidateLimit: number;
  onCandidateLimitChange: (candidateLimit: number) => void;
  promptId?: string;
  prompt?: string;
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  fallbackPrompt: string;
  onPromptIdChange: (promptId: string | undefined) => void;
  onPromptChange: (prompt: string | undefined) => void;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
  testTools: ReactNode;
  mcpConnect: ReactNode;
}

/** Page-level composition for the shared AI assistant settings. */
export function AiSettingsPageSection({
  disabled,
  hidden,
  enabled,
  enabledDescription,
  provider,
  providerOptions,
  model,
  endpoint,
  providerPreset,
  onEnabledChange,
  onProviderChange,
  onModelChange,
  onEndpointChange,
  credentialSection,
  desktopCredentialTest,
  modelCatalog,
  candidateLimit,
  onCandidateLimitChange,
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
  testTools,
  mcpConnect,
}: AiSettingsPageSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="AI 辅助">
      <AiBasicSettingsSection
        enabled={enabled}
        enabledDescription={enabledDescription}
        provider={provider}
        providerOptions={providerOptions}
        model={model}
        endpoint={endpoint}
        providerPreset={providerPreset}
        onEnabledChange={onEnabledChange}
        onProviderChange={onProviderChange}
        onModelChange={onModelChange}
        onEndpointChange={onEndpointChange}
      />
      {credentialSection}
      {desktopCredentialTest}
      {modelCatalog && (
        <AiModelCatalogSection
          busy={modelCatalog.busy}
          origin={modelCatalog.origin}
          models={modelCatalog.models}
          selectedModel={model}
          status={modelCatalog.status}
          onFetch={modelCatalog.onFetch}
          onSelect={modelCatalog.onSelect}
        />
      )}
      <AiCandidateLimitSection value={candidateLimit} onChange={onCandidateLimitChange} />
      <AiPromptSettingsSection
        promptId={promptId}
        prompt={prompt}
        promptCustom1={promptCustom1}
        promptCustom2={promptCustom2}
        promptCustom3={promptCustom3}
        fallbackPrompt={fallbackPrompt}
        onPromptIdChange={onPromptIdChange}
        onPromptChange={onPromptChange}
        onPromptCustom1Change={onPromptCustom1Change}
        onPromptCustom2Change={onPromptCustom2Change}
        onPromptCustom3Change={onPromptCustom3Change}
      />
      {testTools}
      {mcpConnect}
    </fieldset>
  );
}
