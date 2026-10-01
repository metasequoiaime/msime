export interface FeedbackChannelsProps {
  issuesUrl: string;
  feedbackCopied: boolean;
  onOpenIssues: () => void;
  onCopyGroup: () => void;
  onOpenTelegram: () => void;
  listClassName?: string;
  cardClassName: string;
  iconClassName: string;
  bodyClassName: string;
  titleClassName: string;
}

/** Shared feedback channel cards used by the legacy and grouped settings surfaces. */
export function FeedbackChannels({
  issuesUrl,
  feedbackCopied,
  onOpenIssues,
  onCopyGroup,
  onOpenTelegram,
  listClassName,
  cardClassName,
  iconClassName,
  bodyClassName,
  titleClassName,
}: FeedbackChannelsProps) {
  const channels: {
    badge: string;
    title: string;
    description: string;
    code: string;
    action: string;
    onClick: () => void;
  }[] = [
    {
      badge: "GH",
      title: "GitHub Issues",
      description: "适合提交可复现的问题、功能建议和开发讨论。",
      code: issuesUrl.replace("https://", ""),
      action: "查看 Issues",
      onClick: onOpenIssues,
    },
    {
      badge: "QQ",
      title: "QQ 交流群",
      description: "适合中文用户进行日常交流、测试反馈和使用讨论。",
      code: "群号：829919142",
      action: feedbackCopied ? "已复制" : "复制群号",
      onClick: onCopyGroup,
    },
    {
      badge: "TG",
      title: "Telegram 群组",
      description: "面向国际用户和开发者的即时讨论频道。",
      code: "t.me/msimegroup",
      action: "打开群组",
      onClick: onOpenTelegram,
    },
  ];
  const cards = channels.map((channel) => (
    <div className={cardClassName} key={channel.badge}>
      <div className={iconClassName}>{channel.badge}</div>
      <div className={bodyClassName}>
        <div className={titleClassName}>{channel.title}</div>
        <p>{channel.description}</p>
        <code>{channel.code}</code>
      </div>
      <button type="button" className="secondary" onClick={channel.onClick}>
        {channel.action}
      </button>
    </div>
  ));
  return listClassName ? <div className={listClassName}>{cards}</div> : <>{cards}</>;
}
