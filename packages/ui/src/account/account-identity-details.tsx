import { useId, type ReactNode } from "react";
import * as account from "./account-style";
import { accountProviderName } from "./account-labels";
import { ActionButton } from "../core/action-button";
import { formatZhDate } from "../core/format-date";

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
          <ActionButton
            action={onCopy}
            className={account.copyId}
            label={copied ? "已复制" : `#${user.id.slice(0, 6).toUpperCase()}`}
          />
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
        <dd>{formatZhDate(user.createdAt)}</dd>
      </div>
    </dl>
  );
}

function Chevron() {
  return (
    <svg
      width="7"
      height="12"
      viewBox="0 0 8 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={account.profileRowChevron}
    >
      <path d="m1 1 6 6-6 6" />
    </svg>
  );
}

/** 「个人资料」分组中的一行：标签、末端的值，点按有操作时还带一个箭头。 */
function ProfileRow({
  label,
  value,
  ariaLabel,
  disabled,
  onClick,
}: {
  label: string;
  value: string;
  ariaLabel?: string;
  disabled?: boolean;
  onClick?: () => void;
}) {
  const body = (
    <>
      <span className={account.profileRowLabel}>{label}</span>
      <span className={account.profileRowValue}>{value}</span>
      {onClick && <Chevron />}
    </>
  );
  if (!onClick) return <div className={account.profileRow}>{body}</div>;
  return (
    <button
      type="button"
      className={`${account.profileRow} ${account.profileRowButton}`}
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={onClick}
    >
      {body}
    </button>
  );
}

/** HarmonyOS「个人资料」页中带标题的分组，卡片下可选附带脚注。 */
export function ProfileGroup({
  title,
  footer,
  children,
}: {
  title?: string;
  footer?: string;
  children: ReactNode;
}) {
  const titleId = useId();
  return (
    <section className={account.profileGroup} aria-labelledby={title ? titleId : undefined}>
      {title && (
        <h3 id={titleId} className={account.profileGroupTitle}>
          {title}
        </h3>
      )}
      <div className={account.profileRows}>{children}</div>
      {footer && <p className={account.profileGroupFooter}>{footer}</p>}
    </section>
  );
}

export interface AccountIdentityRowsProps {
  user: {
    id: string;
    email?: string;
  };
  /** 显示的昵称；用户未设置时回退为生成的昵称。 */
  name: string;
  /** 已关联的登录提供方；「登录方式」分组会等它们就绪，而不是去猜。 */
  providers: string[] | null;
  disabled?: boolean;
  onRename: () => void;
  onCopyId: () => void;
}

/** HarmonyOS 手机「个人资料」页的「账号」和「登录方式」分组：「昵称」打开改名弹窗，「水杉 ID」点按复制自身，「邮箱」和每个已关联的提供方都是只读的。 */
export function AccountIdentityRows({
  user,
  name,
  providers,
  disabled,
  onRename,
  onCopyId,
}: AccountIdentityRowsProps) {
  return (
    <>
      <ProfileGroup title="账号">
        <ProfileRow
          label="昵称"
          value={name}
          ariaLabel={`昵称，${name}`}
          disabled={disabled}
          onClick={onRename}
        />
        <ProfileRow
          label="水杉 ID"
          value={user.id}
          ariaLabel={`复制水杉 ID，${user.id}`}
          onClick={onCopyId}
        />
        {user.email && <ProfileRow label="邮箱" value={user.email} />}
      </ProfileGroup>
      {providers && providers.length > 0 && (
        <ProfileGroup title="登录方式" footer="关联后可以用任意一种方式登录同一个账号">
          {providers.map((provider) => (
            <ProfileRow key={provider} label={accountProviderName(provider)} value="已关联" />
          ))}
        </ProfileGroup>
      )}
    </>
  );
}
