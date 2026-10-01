import { FeedbackChannels } from "./feedback-channels";
import { SettingField } from "./setting-field";
export function FeedbackSettingsSection({
  hero,
  eyebrow,
  heroTitle,
  note,
  feedbackList,
  feedbackCard,
  feedbackIcon,
  feedbackBody,
  feedbackTitle,
  serviceRow,
  kind,
  detail,
  reportCopied,
  feedbackCopied,
  supportDiagnostics,
  issuesUrl,
  copyText,
  openExternalUrl,
  onKindChange,
  onDetailChange,
  onCopyReport,
  onSubmitFeedback,
  onOpenIssues,
  onCopyGroup,
  onOpenTelegram,
}: {
  hero: string;
  eyebrow: string;
  heroTitle: string;
  note: string;
  feedbackList: string;
  feedbackCard: string;
  feedbackIcon: string;
  feedbackBody: string;
  feedbackTitle: string;
  serviceRow: string;
  kind: string;
  detail: string;
  reportCopied: boolean;
  feedbackCopied: boolean;
  supportDiagnostics: string;
  issuesUrl: string;
  copyText?: (text: string) => Promise<void>;
  openExternalUrl?: (url: string) => Promise<void>;
  onKindChange: (value: string) => void;
  onDetailChange: (value: string) => void;
  onCopyReport: () => void;
  onSubmitFeedback: () => void;
  onOpenIssues: () => void;
  onCopyGroup: () => void;
  onOpenTelegram: () => void;
}) {
  return (
    <>
      <div className={`section ${hero}`}>
        <div className={eyebrow}>反馈与交流</div>
        <div className={heroTitle}>告诉我们你的想法</div>
        <p>遇到问题或有功能建议时，可以通过以下渠道提交和交流。</p>
      </div>
      <div className="section" aria-label="问题报告">
        <div className="section-title">
          提交可复现的问题
          <small>报告只在你点击按钮时生成，不会读取或上传输入历史。</small>
        </div>
        <SettingField label="类型">
          <select
            aria-label="反馈类型"
            value={kind}
            onChange={(event) => onKindChange(event.target.value)}
          >
            <option>功能异常</option>
            <option>候选词不对</option>
            <option>功能建议</option>
            <option>其他</option>
          </select>
        </SettingField>
        <label className="section-title">
          描述
          <textarea
            aria-label="反馈描述"
            maxLength={4000}
            value={detail}
            onChange={(event) => onDetailChange(event.target.value)}
            placeholder="发生了什么？如果和打字有关，写出输入方案、编码和期望结果。"
            rows={6}
          />
        </label>
        <div className={note}>
          <strong>会一起附上的信息</strong>
          <span className="block break-anywhere text-xs text-secondary">{supportDiagnostics}</span>
        </div>
        <div className={serviceRow}>
          {copyText && (
            <button type="button" className="secondary" onClick={onCopyReport}>
              {reportCopied ? "已复制报告" : "复制报告"}
            </button>
          )}
          {openExternalUrl && (
            <button type="button" className="secondary" onClick={onSubmitFeedback}>
              在 GitHub 提交
            </button>
          )}
        </div>
        <small>
          提交会打开 GitHub 并预填报告；网址长度有限，过长描述会被截断，完整内容请先复制。
        </small>
      </div>
      <FeedbackChannels
        issuesUrl={issuesUrl}
        feedbackCopied={feedbackCopied}
        onOpenIssues={onOpenIssues}
        onCopyGroup={onCopyGroup}
        onOpenTelegram={onOpenTelegram}
        listClassName={feedbackList}
        cardClassName={feedbackCard}
        iconClassName={feedbackIcon}
        bodyClassName={feedbackBody}
        titleClassName={feedbackTitle}
      />
      <div className={`section ${note}`}>
        <strong>提交问题时建议附上</strong>
        <span>系统版本、输入方案、复现步骤、相关截图，以及 Debug 输出中的关键日志。</span>
      </div>
    </>
  );
}
