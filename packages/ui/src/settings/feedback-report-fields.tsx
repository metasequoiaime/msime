import { useId, useState, type ReactNode } from "react";
import type { FeedbackClient, FeedbackType, ImeSetupState } from "../core/host-contracts";
import { errorCode } from "../core/error-code";
import { GroupList } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";
import { useToast } from "../core/toast";
import * as doc from "./document-style";
import { feedbackKindOptions } from "./feedback-kind-options";
import { SelectRow } from "./select-row";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SwitchRow } from "./switch-row";

export interface FeedbackReportFieldsProps {
  kind: string;
  detail: string;
  onKindChange: (value: string) => void;
  onDetailChange: (value: string) => void;
  children?: ReactNode;
}

/** Shared feedback type and description fields of the feedback page. */
export function FeedbackReportFields({
  kind,
  detail,
  onKindChange,
  onDetailChange,
  children,
}: FeedbackReportFieldsProps) {
  return (
    <>
      <SelectRow
        title="类型"
        aria-label="反馈类型"
        value={kind}
        onChange={(event) => onKindChange(event.target.value)}
      >
        {feedbackKindOptions()}
      </SelectRow>
      <SettingsManagerBlock>
        <SettingsTextareaField
          label="描述"
          ariaLabel="反馈描述"
          maxLength={4000}
          value={detail}
          onChange={onDetailChange}
          placeholder="发生了什么？如果和打字有关，写出输入方案、编码和期望结果。"
          rows={6}
        />
        {children}
      </SettingsManagerBlock>
    </>
  );
}

/** 服务对报告正文的长度上限，按字符计（Android `FeedbackApi.MAX_TEXT`）。 */
export const feedbackMaxText = 500;

/** 服务接受的报告类型及页面显示的名称（Android `FeedbackApi.Type`）。 */
export const feedbackTypes: readonly { value: FeedbackType; label: string }[] = [
  { value: "bug", label: "问题" },
  { value: "suggestion", label: "建议" },
  { value: "dictionary", label: "词库纠错" },
];

export interface FeedbackDiagnosticsSource {
  appVersion: string;
  edition?: string;
  host?: { platform: string; os_version?: string };
  preferences?: { scheme?: string; touch_keyboard_layout?: string; global_theme?: string };
  /** 输入法设置状态，宿主能判断时提供。 */
  setup?: ImeSetupState | null;
}

/**
 * 「附带诊断信息」附上的诊断字段，键名与服务保存的一致（Android `FeedbackApi.DIAGNOSTIC_KEYS`）：系统版本、应用版本和版本类型、输入方案、键盘布局、键盘绘制所用的主题，以及键盘是否已启用、是否为当前键盘。不含任何输入内容，不含日志。宿主无法回答的字段直接省略而不是发空值；设备型号就是其一，因为没有宿主能力上报它。
 */
export function feedbackDiagnostics({
  appVersion,
  edition,
  host,
  preferences,
  setup,
}: FeedbackDiagnosticsSource): Record<string, string> {
  const fields: [string, string | undefined][] = [
    ["os", host?.os_version ? `HarmonyOS ${host.os_version}` : undefined],
    ["app_version", appVersion],
    ["edition", edition],
    ["scheme", preferences?.scheme],
    ["keyboard_layout", preferences?.touch_keyboard_layout],
    ["skin", preferences?.global_theme],
    ["ime_enabled", typeof setup?.enabled === "boolean" ? String(setup.enabled) : undefined],
    ["ime_default", typeof setup?.current === "boolean" ? String(setup.current) : undefined],
  ];
  const diagnostics: Record<string, string> = {};
  for (const [key, value] of fields) if (value) diagnostics[key] = value;
  return diagnostics;
}

/** 提交失败时的说明，措辞与 Android 反馈页一致，在重试无济于事时指向下方的渠道。 */
function submissionFailure(error: unknown): string {
  switch (errorCode(error)) {
    case "account_rate_limited":
      return "提交得太频繁了，请过一会儿再试";
    case "account_unauthorized":
      return "登录账号后才能提交，也可以改用下面的渠道";
    case "account_unavailable":
      return "反馈服务暂时连不上，请稍后重试，或改用下面的渠道";
  }
  return "没有提交成功，请稍后重试";
}

export interface FeedbackSubmissionFormProps {
  feedback: FeedbackClient;
  /** 在发送时构建诊断字段，让它们描述用户报告时所处的状态；只在「附带诊断信息」打开时调用。 */
  diagnostics: () => Record<string, string>;
}

/**
 * 鸿蒙手机的应用内反馈（设计稿的「反馈」页，Android 的 `FeedbackPage`）：「类型」分组含报告类型和「附带诊断信息」开关，「描述」卡片含正文及其 `N / 500` 计数，以及一个通栏提交按钮。提交时经宿主的 `FeedbackClient` 发送报告；成功时按钮改为「已提交」并弹出 toast，失败时在按钮下方说明原因。提交后再编辑正文会让按钮重新可用。
 */
export function FeedbackSubmissionForm({ feedback, diagnostics }: FeedbackSubmissionFormProps) {
  const toast = useToast();
  const titleId = useId();
  const [type, setType] = useState<FeedbackType>("bug");
  const [attach, setAttach] = useState(false);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [failure, setFailure] = useState("");
  const ready = !sending && !sent && text.trim().length > 0 && text.length <= feedbackMaxText;

  async function submit() {
    if (!ready) return;
    setSending(true);
    setFailure("");
    try {
      await feedback.submit({
        type,
        text: text.trim(),
        diagnostics: attach ? diagnostics() : null,
      });
      setSent(true);
      toast("已提交，感谢反馈");
    } catch (error) {
      setFailure(submissionFailure(error));
    } finally {
      setSending(false);
    }
  }

  return (
    <>
      <GroupList title="类型">
        <SelectRow
          title="反馈类型"
          value={type}
          onChange={(event) => setType(event.target.value as FeedbackType)}
        >
          {feedbackTypes.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </SelectRow>
        <SwitchRow
          title="附带诊断信息"
          description="包含系统版本、应用版本、当前方案与键盘设置，不含输入内容"
          checked={attach}
          onChange={setAttach}
        />
      </GroupList>
      <section className={controls.group} aria-labelledby={titleId}>
        <h3 id={titleId} className={controls.groupTitle} data-group-title="">
          描述
        </h3>
        <div className={doc.feedbackTextCard}>
          <textarea
            className={doc.feedbackTextarea}
            aria-label="反馈描述"
            placeholder="遇到了什么问题？可以写复现步骤、出错的词或期望的结果"
            maxLength={feedbackMaxText}
            value={text}
            onChange={(event) => {
              setText(event.currentTarget.value);
              setSent(false);
              setFailure("");
            }}
          />
        </div>
        <span className={doc.feedbackCounter}>
          {text.length} / {feedbackMaxText}
        </span>
        <button
          type="button"
          className={doc.feedbackSubmit(ready)}
          disabled={!ready}
          aria-busy={sending}
          onClick={() => void submit()}
        >
          {sent ? "已提交" : sending ? "正在提交…" : "提交"}
        </button>
        {failure && (
          <p className={doc.feedbackError} role="alert">
            {failure}
          </p>
        )}
      </section>
    </>
  );
}
