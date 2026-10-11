export interface AccountIdCopyOptions {
  id?: string | null;
  isMounted: () => boolean;
  onCopied: () => void;
  setError: (message: string) => void;
}

/** 复制账号 ID；成功后由调用方选择行内提示或 Toast，并忽略卸载后的结果。 */
export function copyAccountId({ id, isMounted, onCopied, setError }: AccountIdCopyOptions): void {
  if (!id || !navigator.clipboard?.writeText) return;
  void navigator.clipboard
    .writeText(id)
    .then(() => {
      if (!isMounted()) return;
      onCopied();
    })
    .catch(() => {
      if (isMounted()) setError("账号 ID 暂时无法复制，请稍后重试。");
    });
}
