import * as settings from "./settings-style";
import { ActionButton } from "./action-button";

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
        <ActionButton
          action={onTest}
          ariaLabel={label}
          disabled={disabled || (visible && state.busy)}
          label={visible && state.busy ? "测试中…" : "测试配置"}
        />
        {visible && state.message && (
          <span role={state.ok ? "status" : "alert"}>{state.message}</span>
        )}
      </div>
    </div>
  );
}
