import * as doc from "../document-style";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList } from "../../core/platform-controls";
import { SubPageEntries } from "./sub-page-entries";
import { createSettingsExternalActions } from "../settings-external-actions";
import { FeedbackChannels } from "../feedback-channels";
import { FeedbackReportFields } from "../feedback-report-fields";

/** The 反馈 page of the settings form. */
export function FeedbackSettingsPage() {
  const {
    client,
    platformIssuesUrl,
    busy,
    page,
    copyFeedbackGroup,
    feedbackCopied,
    feedbackKind,
    setFeedbackKind,
    feedbackDetail,
    setFeedbackDetail,
    copyFeedbackReport,
    feedbackReportCopied,
    supportDiagnostics,
    submitFeedback,
    openExternalUrl,
    mobilePlatform,
  } = useSettingsForm();
  const externalActions = createSettingsExternalActions({
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    issuesUrl: platformIssuesUrl,
  });
  return (
    <fieldset disabled={busy} hidden={page !== "feedback"} aria-label="反馈">
      <div className={settings.groups}>
        <p className={settings.groupNote}>遇到问题或有功能建议时，可以通过以下渠道提交和交流。</p>
        <GroupList title="提交可复现的问题">
          <div className={settings.rowStack} role="group" aria-label="问题报告">
            <p className={settings.groupNote}>报告只在你点击按钮时生成，不会读取或上传输入历史。</p>
            <FeedbackReportFields
              grouped
              kind={feedbackKind}
              detail={feedbackDetail}
              onKindChange={setFeedbackKind}
              onDetailChange={setFeedbackDetail}
            >
              <div className={doc.note}>
                <strong>会一起附上的信息</strong>
                <span className="block break-anywhere">{supportDiagnostics}</span>
              </div>
              <div className={settings.managerActions}>
                {client.copyText && (
                  <button type="button" className="secondary" onClick={copyFeedbackReport}>
                    {feedbackReportCopied ? "已复制报告" : "复制报告"}
                  </button>
                )}
                {client.openExternalUrl && (
                  <button type="button" className="secondary" onClick={submitFeedback}>
                    在 GitHub 提交
                  </button>
                )}
              </div>
              <p className={settings.managerNote}>
                提交会打开 GitHub 并预填报告；网址长度有限，过长描述会被截断，完整内容请先复制。
              </p>
            </FeedbackReportFields>
          </div>
        </GroupList>
        <GroupList title="反馈与交流">
          <FeedbackChannels
            issuesUrl={platformIssuesUrl}
            feedbackCopied={feedbackCopied}
            onOpenIssues={externalActions.onOpenIssues}
            onCopyGroup={copyFeedbackGroup}
            onOpenTelegram={externalActions.onOpenTelegram}
            cardClassName={doc.feedbackCard}
            iconClassName={doc.feedbackIcon}
            bodyClassName={doc.feedbackBody}
            titleClassName={doc.feedbackTitle}
          />
        </GroupList>
        <GroupList title="提交问题时建议附上">
          <p className={settings.groupNote}>
            系统版本、输入方案、复现步骤、相关截图，以及 Debug 输出中的关键日志。
          </p>
        </GroupList>
        <SubPageEntries
          title="帮助"
          pages={[{ id: "help", description: "安装、切换输入法与常见问题" }]}
        />
      </div>
    </fieldset>
  );
}
