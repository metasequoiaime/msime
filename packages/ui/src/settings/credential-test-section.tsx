import * as settings from "./settings-style";

export interface CredentialTestState {
  signature: string;
  busy: boolean;
  ok?: boolean;
  message: string;
}

export interface CredentialTestSectionProps {
  label: string;
  config: Record<string, unknown>;
  state?: CredentialTestState;
  disabled?: boolean;
  available?: boolean;
  onTest: () => void;
}

/** Test button and result status shared by provider configuration sections. */
export function CredentialTestSection({
  label,
  config,
  state,
  disabled = false,
  available = true,
  onTest,
}: CredentialTestSectionProps) {
  if (!available) return null;
  const signature = JSON.stringify(config);
  const visible = state?.signature === signature;
  return (
    <div className={settings.serviceRow}>
      <div>
        <button
          type="button"
          className="secondary"
          aria-label={label}
          disabled={disabled || (visible && state.busy)}
          onClick={onTest}
        >
          {visible && state.busy ? "测试中…" : "测试配置"}
        </button>
        {visible && state.message && (
          <span role={state.ok ? "status" : "alert"}>{state.message}</span>
        )}
      </div>
    </div>
  );
}
