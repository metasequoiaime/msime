import { useId, type ReactNode } from "react";
import { GroupList, NavGroup } from "../core/platform-controls";
import * as doc from "../settings/document-style";
import * as account from "./account-style";

export interface AccountMeSectionProps {
  title?: string;
  children: ReactNode;
}

export interface AccountMeGroupProps extends AccountMeSectionProps {}

/** 触屏「我的」页的分组外壳，可让不同页面保留各自的内容容器。 */
export function AccountMeSection({ title, children }: AccountMeSectionProps) {
  const titleId = useId();
  return (
    <section className={account.meGroup} aria-labelledby={title ? titleId : undefined}>
      {title && (
        <h3 id={titleId} className={account.meGroupTitle}>
          {title}
        </h3>
      )}
      {children}
    </section>
  );
}

/** 触屏「我的」页的一组跳转行，可选显示分组标题。 */
export function AccountMeGroup({ title, children }: AccountMeGroupProps) {
  return (
    <AccountMeSection title={title}>
      <NavGroup>{children}</NavGroup>
    </AccountMeSection>
  );
}

export interface AccountMeRowProps {
  title: string;
  disabled?: boolean;
  onClick: () => void;
}

/** 「我的」页的整行跳转按钮，保留现有文案、箭头和禁用语义。 */
export function AccountMeRow({ title, disabled, onClick }: AccountMeRowProps) {
  return (
    <button type="button" className={doc.linkRow} disabled={disabled} onClick={onClick}>
      <span className={doc.linkTitle}>{title}</span>
      <span aria-hidden="true">›</span>
    </button>
  );
}

export interface AccountMeSupportGroupProps {
  disabled?: boolean;
  onReplayOnboarding?: () => void;
  onOpenFeedback?: () => void;
  onOpenAbout?: () => void;
}

/** 触屏「我的」页末尾不带标题的引导、帮助和关于入口。 */
export function AccountMeSupportGroup({
  disabled,
  onReplayOnboarding,
  onOpenFeedback,
  onOpenAbout,
}: AccountMeSupportGroupProps) {
  if (!onReplayOnboarding && !onOpenFeedback && !onOpenAbout) return null;
  return (
    <GroupList>
      {onReplayOnboarding && (
        <AccountMeRow title="新手引导" disabled={disabled} onClick={onReplayOnboarding} />
      )}
      {onOpenFeedback && (
        <AccountMeRow title="帮助与反馈" disabled={disabled} onClick={onOpenFeedback} />
      )}
      {onOpenAbout && <AccountMeRow title="关于" disabled={disabled} onClick={onOpenAbout} />}
    </GroupList>
  );
}
