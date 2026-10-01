export interface AsyncActionState {
  busy: boolean;
  isCurrent: () => boolean;
  setBusy: (busy: boolean) => void;
  setError: (message: string) => void;
  setNotice?: (message: string) => void;
}

export interface AsyncActionOptions {
  formatError: (error: unknown) => string;
  ignoreError?: (error: unknown) => boolean;
  onError?: (error: unknown) => void;
}

/** Runs a guarded UI action and publishes only results from its current owner. */
export async function runAsyncAction(
  { busy, isCurrent, setBusy, setError, setNotice }: AsyncActionState,
  operation: (isCurrent: () => boolean) => Promise<void>,
  { formatError, ignoreError, onError }: AsyncActionOptions,
): Promise<void> {
  if (busy || !isCurrent()) return;
  setBusy(true);
  setError("");
  setNotice?.("");
  try {
    await operation(isCurrent);
  } catch (error) {
    if (isCurrent()) {
      if (!ignoreError?.(error)) setError(formatError(error));
      onError?.(error);
    }
  } finally {
    if (isCurrent()) setBusy(false);
  }
}
