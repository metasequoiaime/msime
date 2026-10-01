import type { CSSProperties } from "react";

// Brand marks from yldm-tech/ai-logo (packages/static-svg, MIT).
const githubIcon = new URL("../assets/github.svg", import.meta.url).href;
const qqIcon = new URL("../assets/qq-color.svg", import.meta.url).href;
const telegramIcon = new URL("../assets/telegram-color.svg", import.meta.url).href;

/** Colour marks are drawn as-is; the monochrome GitHub mark is masked out of the text colour so it follows light and dark themes. */
const brandGlyph = "block size-6";
const monoGlyph =
  "block size-6 [background:var(--p-text)] [mask-image:var(--channel-icon)] [mask-position:center] [mask-repeat:no-repeat] [mask-size:contain] [-webkit-mask-image:var(--channel-icon)] [-webkit-mask-position:center] [-webkit-mask-repeat:no-repeat] [-webkit-mask-size:contain]";

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
    id: string;
    icon: string;
    mono?: boolean;
    title: string;
    description: string;
    code: string;
    action: string;
    onClick: () => void;
  }[] = [
    {
      id: "github",
      icon: githubIcon,
      mono: true,
      title: "GitHub Issues",
      description: "适合提交可复现的问题、功能建议和开发讨论。",
      code: issuesUrl.replace("https://", ""),
      action: "查看 Issues",
      onClick: onOpenIssues,
    },
    {
      id: "qq",
      icon: qqIcon,
      title: "QQ 交流群",
      description: "适合中文用户进行日常交流、测试反馈和使用讨论。",
      code: "群号：829919142",
      action: feedbackCopied ? "已复制" : "复制群号",
      onClick: onCopyGroup,
    },
    {
      id: "telegram",
      icon: telegramIcon,
      title: "Telegram 群组",
      description: "面向国际用户和开发者的即时讨论频道。",
      code: "t.me/msimegroup",
      action: "打开群组",
      onClick: onOpenTelegram,
    },
  ];
  const cards = channels.map((channel) => (
    <div className={cardClassName} key={channel.id}>
      <div className={iconClassName} aria-hidden="true">
        {channel.mono ? (
          <span
            className={monoGlyph}
            style={{ "--channel-icon": `url("${channel.icon}")` } as CSSProperties}
          />
        ) : (
          <img className={brandGlyph} src={channel.icon} alt="" />
        )}
      </div>
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
