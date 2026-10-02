import * as account from "./account-style";
import { accountProviderName } from "./account-labels";

export interface AccountIdentityDetailsProps {
  user: {
    id: string;
    createdAt: string;
    email?: string;
  };
  providers: string[];
  copied?: boolean;
  onCopy: () => void;
}

/** Shared account metadata list used by the mobile profile page and desktop dialog. */
export function AccountIdentityDetails({
  user,
  providers,
  copied = false,
  onCopy,
}: AccountIdentityDetailsProps) {
  return (
    <dl className={account.details}>
      <div>
        <dt>账号 ID</dt>
        <dd>
          <button type="button" className={account.copyId} onClick={onCopy}>
            {copied ? "已复制" : `#${user.id.slice(0, 6).toUpperCase()}`}
          </button>
        </dd>
      </div>
      {user.email && (
        <div>
          <dt>邮箱</dt>
          <dd>{user.email}</dd>
        </div>
      )}
      <div>
        <dt>登录方式</dt>
        <dd>{providers.map(accountProviderName).join("、") || "正在读取"}</dd>
      </div>
      <div>
        <dt>加入水杉</dt>
        <dd>{new Date(user.createdAt).toLocaleDateString("zh-CN")}</dd>
      </div>
    </dl>
  );
}
