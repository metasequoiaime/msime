import { useState } from "react";
import { ActionButton } from "../core/action-button";
import { useToast } from "../core/toast";
import { useTemporaryFlag } from "../core/use-temporary-flag";
import { AccountAvatar } from "./account-avatar";
import { AccountConfirmation } from "./account-confirmation";
import { copyAccountId as copyAccountIdToClipboard } from "./account-id-copy";
import { AccountIdentityDetails, AccountIdentityRows } from "./account-identity-details";
import type { AccountClient, AccountProfile, AccountUser } from "./account-model";
import { AccountNicknameEditor, accountNicknameHelpText } from "./account-nickname-editor";
import { useAccountAction } from "./account-operation";
import { AccountProfileActions, type HarmonyProfileAction } from "./account-profile-actions";
import { AccountStatusMessages } from "./account-status-messages";
import * as account from "./account-style";
import { BottomSheet } from "./login-sheet";
import { MeSubpageHeader } from "./mobile-download-page";
import { useAccountRecoveryActions } from "./use-account-recovery-actions";
import { useAccountRename } from "./use-account-rename";
import {
  ACCOUNT_DELETED_VOICE_SAMPLES_KEPT,
  runAccountSessionAction,
  useAccountSessionActions,
} from "./use-account-session-actions";

interface AccountProfilePageProps {
  client: AccountClient;
  user: AccountUser;
  profile: AccountProfile | null;
  onBack: () => void;
  onProfileUpdated: (profile: AccountProfile) => void;
}

export interface MobileAccountProfilePageProps extends AccountProfilePageProps {
  onSignedOut: () => void;
}

export interface HarmonyProfilePageProps extends AccountProfilePageProps {
  /** 会话结束后调用一次，参数是说明结束方式的 toast。 */
  onSignedOut: (message: string) => void;
}

/** toast 读起来是一句短句，不带行内状态行末尾的句号。 */
function toastText(message: string): string {
  return message.replace(/。$/, "");
}

export function MobileAccountProfilePage({
  client,
  user,
  profile,
  onBack,
  onSignedOut,
  onProfileUpdated,
}: MobileAccountProfilePageProps) {
  const [name, setName] = useState(user.displayName);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmation, setConfirmation] = useState<
    "logout" | "logout-all" | "relogin" | "delete" | null
  >(null);
  const [copied, showCopied] = useTemporaryFlag(1800);
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setError,
    setNotice,
  );
  const { signOut, deleteAccount } = useAccountSessionActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onSignedOut,
    onDeleted: () => onSignedOut(),
  });
  const { clearExpired } = useAccountRecoveryActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onClearedExpired: onSignedOut,
  });
  const { normalizedName, validName, rename, editorState } = useAccountRename({
    client,
    name,
    mounted,
    clientGeneration,
    perform,
    onRenamed: (updated) => {
      onProfileUpdated(updated);
      setName(updated.user.displayName);
      setNotice("昵称已更新。");
    },
  });
  const confirmAction = () => {
    const action = confirmation;
    setConfirmation(null);
    if (action === "relogin") clearExpired();
    else if (action) runAccountSessionAction(action, { signOut, deleteAccount });
  };
  const copyId = () => {
    copyAccountIdToClipboard({
      id: user.id,
      isMounted: () => mounted.current,
      onCopied: showCopied,
      setError,
    });
  };

  return (
    <div className={account.page}>
      <div className={account.profilePageHeader}>
        <ActionButton action={onBack} className="secondary" disabled={busy} label="‹ 返回" />
        <h2 className={account.heading}>编辑资料</h2>
      </div>
      <AccountStatusMessages error={error} notice={notice} />
      <section className={`${account.section} ${account.profilePreviewLarge}`}>
        <AccountAvatar
          user={user}
          name={
            editorState.checking || editorState.unavailable
              ? undefined
              : normalizedName || "水杉用户"
          }
          load={client.avatar}
          size="large"
        />
        <h2 className={account.heading}>{normalizedName || "你的昵称"}</h2>
        <p className={account.muted}>在水杉，留下你的名字</p>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>社区昵称</h2>
        <AccountNicknameEditor
          {...editorState}
          variant="mobile"
          value={name}
          disabled={busy}
          normalizedName={normalizedName}
          validName={validName}
          onChange={setName}
        />
        <div className={account.actionRow}>
          <ActionButton
            action={rename}
            className={account.primary}
            disabled={busy || !validName || normalizedName === user.displayName}
            label="保存昵称"
          />
        </div>
      </section>
      <section className={`${account.section} ${account.stack}`}>
        <h2 className={account.heading}>账号信息</h2>
        <AccountIdentityDetails
          user={user}
          providers={profile?.providers ?? []}
          providerNames={profile?.providerNames ?? {}}
          copied={copied}
          onCopy={copyId}
        />
      </section>
      <AccountProfileActions variant="mobile" busy={busy} onSelect={setConfirmation} />
      {confirmation && (
        <AccountConfirmation
          action={confirmation}
          busy={busy}
          confirmLabel="确认"
          onConfirm={confirmAction}
          onCancel={() => setConfirmation(null)}
        />
      )}
    </div>
  );
}

/**
 * 鸿蒙手机上的「个人资料」，从「我的」卡片推入：头像、名字、邮箱和账号登录用的提供方，然后是「账号」和「登录方式」两组，会话操作是居中的红色行。「昵称」打开一个面板，校验与其他地方相同；「水杉 ID」点按即复制自身。结果按设计用 toast 报告；失败则作为错误留在页面上。
 *
 * 没有头像角标，也没有数据分组：这个宿主不能上传头像，也不能导出数据。
 */
export function HarmonyProfilePage({
  client,
  user,
  profile,
  onBack,
  onSignedOut,
  onProfileUpdated,
}: HarmonyProfilePageProps) {
  const [name, setName] = useState(user.displayName);
  const [renaming, setRenaming] = useState(false);
  const [error, setError] = useState("");
  const [confirmation, setConfirmation] = useState<HarmonyProfileAction | null>(null);
  const showToast = useToast();
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setError,
    (message) => {
      if (message) showToast(toastText(message));
    },
  );
  const { signOut, deleteAccount } = useAccountSessionActions({
    client,
    mounted,
    clientGeneration,
    perform,
    onSignedOut: (all) => onSignedOut(all ? "已退出所有设备" : "已退出登录"),
    onDeleted: (voiceSamplesKept) =>
      onSignedOut(voiceSamplesKept ? ACCOUNT_DELETED_VOICE_SAMPLES_KEPT : "账号已注销"),
  });
  const { normalizedName, validName, rename, editorState } = useAccountRename({
    client,
    name,
    mounted,
    clientGeneration,
    perform,
    onRenamed: (updated) => {
      onProfileUpdated(updated);
      setName(updated.user.displayName);
      setRenaming(false);
      showToast("昵称已更新");
    },
  });
  const shownName = user.preferredName;
  const provider = profile?.providers[0];
  const confirmAction = () => {
    const action = confirmation;
    setConfirmation(null);
    if (action) runAccountSessionAction(action, { signOut, deleteAccount });
  };
  const copyId = () =>
    copyAccountIdToClipboard({
      id: user.id,
      isMounted: () => mounted.current,
      onCopied: () => showToast("已复制"),
      setError,
    });

  return (
    <div className={account.subpage}>
      <MeSubpageHeader title="个人资料" onBack={onBack} backDisabled={busy} />
      <AccountStatusMessages error={error} />
      <div className={account.profileHero}>
        <AccountAvatar user={user} load={client.avatar} size="hero" />
        <h3 className={account.profileHeroName}>{shownName}</h3>
        {user.email && <span className={account.profileHeroEmail}>{user.email}</span>}
        {provider && (
          <span className={account.profileHeroPill}>
            通过 {profile?.providerNames[provider] ?? provider} 登录
          </span>
        )}
      </div>
      <AccountIdentityRows
        user={user}
        name={shownName}
        providers={profile?.providers ?? null}
        providerNames={profile?.providerNames ?? {}}
        disabled={busy}
        onRename={() => {
          setName(user.displayName);
          setRenaming(true);
        }}
        onCopyId={copyId}
      />
      <AccountProfileActions variant="harmony" busy={busy} onSelect={setConfirmation} />
      {confirmation && (
        <div className={account.profileConfirmation}>
          <AccountConfirmation
            action={confirmation}
            busy={busy}
            onConfirm={confirmAction}
            onCancel={() => setConfirmation(null)}
          />
        </div>
      )}
      {renaming && (
        <BottomSheet
          title="昵称"
          subtitle={accountNicknameHelpText}
          closeDisabled={busy}
          onClose={() => setRenaming(false)}
        >
          <div className={account.sheetField}>
            <AccountNicknameEditor
              {...editorState}
              variant="sheet"
              value={name}
              disabled={busy}
              normalizedName={normalizedName}
              validName={validName}
              onChange={setName}
            />
          </div>
          <button
            type="button"
            className={account.sheetPrimary}
            disabled={busy || !validName || normalizedName === user.displayName}
            onClick={rename}
          >
            保存
          </button>
        </BottomSheet>
      )}
    </div>
  );
}
