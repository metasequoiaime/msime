export interface CreateSettingsReloadActionOptions {
  dirty: boolean;
  reload: () => Promise<void>;
  confirm: (options: {
    title: string;
    message: string;
    confirmLabel: string;
    danger?: boolean;
  }) => Promise<boolean>;
}

/** Creates the reload callback that protects unsaved settings changes. */
export function createSettingsReloadAction({
  dirty,
  reload,
  confirm,
}: CreateSettingsReloadActionOptions) {
  return async () => {
    if (!dirty) {
      await reload();
      return;
    }
    const confirmed = await confirm({
      title: "重新读取",
      message: "尚未保存的修改会被放弃。",
      confirmLabel: "放弃并重新读取",
      danger: true,
    });
    if (confirmed) await reload();
  };
}
