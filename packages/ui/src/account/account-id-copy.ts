export interface AccountIdCopyOptions {
  id?: string | null;
  isMounted: () => boolean;
  setCopied: (copied: boolean) => void;
  setError: (message: string) => void;
}

/** Copies an account ID while keeping transient UI state safe across unmounts. */
export function copyAccountId({ id, isMounted, setCopied, setError }: AccountIdCopyOptions): void {
  if (!id || !navigator.clipboard?.writeText) return;
  void navigator.clipboard
    .writeText(id)
    .then(() => {
      if (!isMounted()) return;
      setCopied(true);
      window.setTimeout(() => {
        if (isMounted()) setCopied(false);
      }, 1800);
    })
    .catch(() => {
      if (isMounted()) setError("账号 ID 暂时无法复制，请稍后重试。");
    });
}
