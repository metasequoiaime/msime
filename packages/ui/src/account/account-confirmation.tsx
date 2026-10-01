import * as account from "./account-style";

export type AccountConfirmationAction = "logout" | "logout-all" | "relogin" | "delete";

export interface AccountConfirmationProps {
  action: AccountConfirmationAction;
  busy: boolean;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

const details: Record<AccountConfirmationAction, { label: string; message: string }> = {
  logout: {
    label: "确认退出登录",
    message: "退出登录后，社区功能需要重新登录才能使用。",
  },
  "logout-all": {
    label: "确认退出所有设备",
    message: "退出所有设备后，所有设备都需要重新登录。",
  },
  relogin: {
    label: "确认重新登录",
    message: "清除本机登录状态后需要重新登录。",
  },
  delete: {
    label: "确认注销账号",
    message: "注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。",
  },
};

export function AccountConfirmation({
  action,
  busy,
  confirmLabel,
  onConfirm,
  onCancel,
}: AccountConfirmationProps) {
  const detail = details[action];
  return (
    <div className={account.confirmation} role="alertdialog" aria-label={detail.label}>
      <p className={account.note}>{detail.message}</p>
      <div className={account.actionRow}>
        <button
          type="button"
          className={action === "delete" ? account.dangerButton : account.primary}
          disabled={busy}
          onClick={onConfirm}
        >
          {confirmLabel ?? detail.label}
        </button>
        <button type="button" className="secondary" disabled={busy} onClick={onCancel}>
          取消
        </button>
      </div>
    </div>
  );
}
