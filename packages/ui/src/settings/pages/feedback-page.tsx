import { SettingsGroupNote } from "../settings-group-note";
import { SettingsRowStack } from "../settings-row-stack";
import { SettingsManagerActions } from "../settings-manager-actions";
import { SettingsManagerNote } from "../settings-manager-note";
import * as doc from "../document-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow } from "../../core/platform-controls";
import { SubPageEntries } from "./sub-page-entries";
import { createSettingsExternalActions } from "../settings-external-actions";
import { FeedbackChannels } from "../feedback-channels";
import {
  FeedbackReportFields,
  FeedbackSubmissionForm,
  feedbackDiagnostics,
} from "../feedback-report-fields";
import { ActionButton } from "../action-button";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 设置表单的「帮助与反馈」页：先是「帮助」，然后是可复现问题的报告和各个反馈渠道。HarmonyOS 手机在宿主能直接提交反馈时换成新设计的应用内表单，「帮助」和各渠道放在提交按钮下面。桌面上这一页并进「关于」（`desktopMergedPages`），同样的几组由 `FeedbackSections` 画在「关于」里，`feedback` 只留作路由别名。 */
export function FeedbackSettingsPage() {
  const { client, busy, page, settingsPlatform, currentAppVersion, host, draft, mobilePlatform } =
    useSettingsForm();
  // 桌面上这一页永远显示成「关于」，同样的几组已经画在那里；不再挂一份藏起来的副本，免得同一张报告表单在页面里出现两次。
  if (!mobilePlatform) return null;
  // 只有手机使用应用内表单；2in1 和其他所有宿主保留下面的 GitHub 报告，它不需要背后有服务。
  if (settingsPlatform === "harmony" && client.feedback) {
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "feedback"} ariaLabel="帮助与反馈">
        <FeedbackSubmissionForm
          feedback={client.feedback}
          diagnostics={() =>
            feedbackDiagnostics({
              appVersion: currentAppVersion,
              edition: host?.edition?.id,
              host,
              preferences: draft,
              setup: client.home?.setup?.read(),
            })
          }
        />
        <HelpEntry />
        <FeedbackChannelsGroup />
      </SettingsPageFieldset>
    );
  }
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "feedback"} ariaLabel="帮助与反馈">
      <FeedbackSections intro />
    </SettingsPageFieldset>
  );
}

/** 「帮助」入口、「提交可复现的问题」和「反馈与交流」三组：「帮助与反馈」页的正文，桌面上画在「关于」里。 */
export function FeedbackSections({
  intro = false,
}: {
  /** 单独成页时（手机）带上页面导语，收进「反馈与交流」组名旁的「?」；并进「关于」时不需要。 */
  intro?: boolean;
} = {}) {
  const {
    client,
    feedbackKind,
    setFeedbackKind,
    feedbackDetail,
    setFeedbackDetail,
    copyFeedbackReport,
    feedbackReportCopied,
    supportDiagnostics,
    submitFeedback,
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
  return (
    <>
      <HelpEntry />
      <GroupList title="提交可复现的问题">
        <SettingsRowStack role="group" aria-label="问题报告">
          <SettingsGroupNote>报告只在你点击按钮时生成，不会读取或上传输入历史。</SettingsGroupNote>
          <FeedbackReportFields
            kind={feedbackKind}
            detail={feedbackDetail}
            onKindChange={setFeedbackKind}
            onDetailChange={setFeedbackDetail}
          >
            <div className={doc.note}>
              <strong>会一起附上的信息</strong>
              <span className="block wrap-anywhere">{supportDiagnostics}</span>
            </div>
            <SettingsManagerActions>
              {client.copyText && (
                <ActionButton
                  action={copyFeedbackReport}
                  label={feedbackReportCopied ? "已复制报告" : "复制报告"}
                />
              )}
              {client.openExternalUrl && (
                <ActionButton action={submitFeedback} label="在 GitHub 提交" />
              )}
            </SettingsManagerActions>
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
        </SettingsRowStack>
      </GroupList>
      <FeedbackChannelsGroup intro={intro} />
    </>
  );
}

function HelpEntry() {
  return (
    <SubPageEntries
      title="帮助"
      pages={[{ id: "help", description: "安装、切换输入法与常见问题" }]}
    />
  );
}

function FeedbackChannelsGroup({ intro = false }: { intro?: boolean }) {
  const {
    client,
    platformIssuesUrl,
    copyFeedbackGroup,
    feedbackCopied,
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
    <GroupList title="反馈与交流">
      {intro && (
        <SettingsGroupNote>遇到问题或有功能建议时，可以通过以下渠道提交和交流。</SettingsGroupNote>
      )}
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
  );
}
