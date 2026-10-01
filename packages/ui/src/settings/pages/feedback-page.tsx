import * as doc from "../document-style";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, PageIntro } from "../../core/platform-controls";
import { SubPageEntries } from "./sub-page-entries";
import { createSettingsExternalActions } from "../settings-external-actions";
import { FeedbackChannels } from "../feedback-channels";
import { FeedbackReportFields } from "../feedback-report-fields";

/** The 帮助与反馈 page of the settings form: 帮助 first, then the reproducible-issue report and the channels. */
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
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    pageEntry,
    selectPage,
  } = useSettingsForm();
  // The same hosts the 诊断日志 group on 维护与诊断 is drawn for; elsewhere the link would open a page without it.
  const diagnosticLogsOffered =
    Boolean(pageEntry("developer")) &&
    (!client.host || linuxPlatform || windowsPlatform || macosPlatform);
  const externalActions = createSettingsExternalActions({
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    issuesUrl: platformIssuesUrl,
  });
  return (
    <fieldset disabled={busy} hidden={page !== "feedback"} aria-label="帮助与反馈">
      <div className={settings.groups}>
        <PageIntro>遇到问题或有功能建议时，可以通过以下渠道提交和交流。</PageIntro>
        <SubPageEntries
          title="帮助"
          pages={[{ id: "help", description: "安装、切换输入法与常见问题" }]}
        />
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
            <p className={settings.groupNote}>
              提交问题时建议附上系统版本、输入方案、复现步骤、相关截图，以及诊断日志中的关键片段。
            </p>
            {diagnosticLogsOffered && (
              <LinkRow
                title="诊断日志"
                description="在维护与诊断页开启日志并打开日志目录"
                onClick={() => selectPage("developer")}
              />
            )}
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
      </div>
    </fieldset>
  );
}
