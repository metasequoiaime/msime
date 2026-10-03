import { SettingsGroupNote } from "../settings-group-note";
import { SettingsManagerNote } from "../settings-manager-note";
import * as doc from "../document-style";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, PageIntro } from "../../core/platform-controls";
import { SubPageEntries } from "./sub-page-entries";
import { createSettingsExternalActions } from "../settings-external-actions";
import { FeedbackChannels } from "../feedback-channels";
import { FeedbackReportFields } from "../feedback-report-fields";
import { ActionButton } from "../action-button";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 设置表单的「帮助与反馈」页：先是「帮助」，然后是可复现问题的报告和各个反馈渠道。 */
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
  // 与在「维护与诊断」上绘制「诊断日志」组的宿主相同；在其他宿主上，这个链接会打开一个没有该组的页面。
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
    <SettingsPageFieldset disabled={busy} hidden={page !== "feedback"} ariaLabel="帮助与反馈">
      <PageIntro>遇到问题或有功能建议时，可以通过以下渠道提交和交流。</PageIntro>
      <SubPageEntries
        title="帮助"
        pages={[{ id: "help", description: "安装、切换输入法与常见问题" }]}
      />
      <GroupList title="提交可复现的问题">
        <div className={settings.rowStack} role="group" aria-label="问题报告">
          <SettingsGroupNote>报告只在你点击按钮时生成，不会读取或上传输入历史。</SettingsGroupNote>
          <FeedbackReportFields
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
                <ActionButton
                  action={copyFeedbackReport}
                  label={feedbackReportCopied ? "已复制报告" : "复制报告"}
                />
              )}
              {client.openExternalUrl && (
                <ActionButton action={submitFeedback} label="在 GitHub 提交" />
              )}
            </div>
            <SettingsManagerNote>
              提交会打开 GitHub 并预填报告；网址长度有限，过长描述会被截断，完整内容请先复制。
            </SettingsManagerNote>
          </FeedbackReportFields>
          <SettingsGroupNote>
            提交问题时建议附上系统版本、输入方案、复现步骤、相关截图，以及诊断日志中的关键片段。
          </SettingsGroupNote>
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
    </SettingsPageFieldset>
  );
}
