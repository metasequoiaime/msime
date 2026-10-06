import * as settings from "./settings-style";
import { ActionButton } from "./action-button";

export interface DictionaryFailureNotice {
  request_id: string;
  label: string;
  error: string;
}

export interface DictionaryFailuresNoticeProps {
  failures: readonly DictionaryFailureNotice[];
  busy: boolean;
  canRetry: boolean;
  canDismiss: boolean;
  onRetry: (requestId: string) => void;
  onDismiss: (requestId: string) => void;
}

/** Displays dictionary synchronization failures and their recovery actions. */
export function DictionaryFailuresNotice({
  failures,
  busy,
  canRetry,
  canDismiss,
  onRetry,
  onDismiss,
}: DictionaryFailuresNoticeProps) {
  if (failures.length === 0) return null;
  return (
    <div className={settings.failures} role="alert">
      <p>有 {failures.length} 项词库请求同步失败，可以重试或移除失败记录。</p>
      <ul>
        {failures.map((failure) => (
          <li key={failure.request_id}>
            <span>
              <strong>{failure.label}</strong>
              <small>{failure.error}</small>
            </span>
            <span>
              <ActionButton
                action={() => onRetry(failure.request_id)}
                disabled={busy || !canRetry}
                label="重试"
              />{" "}
              <ActionButton
                action={() => onDismiss(failure.request_id)}
                disabled={busy || !canDismiss}
                label="移除记录"
              />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
