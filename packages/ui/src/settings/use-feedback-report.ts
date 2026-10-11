import { useState } from "react";
import { useTemporaryFlag } from "../core/use-temporary-flag";

export interface UseFeedbackReportOptions {
  supportDiagnostics: string;
  issuesUrl: string;
  copyText?: (text: string) => Promise<void>;
  openExternalUrl?: (url: string) => Promise<void>;
}

/** Owns feedback draft fields, report serialization, and clipboard actions. */
export function useFeedbackReport({
  supportDiagnostics,
  issuesUrl,
  copyText,
  openExternalUrl,
}: UseFeedbackReportOptions) {
  const [kind, setKind] = useState("功能异常");
  const [detail, setDetail] = useState("");
  const [reportCopied, showReportCopied] = useTemporaryFlag(1600);
  const [feedbackCopied, showFeedbackCopied] = useTemporaryFlag(1600);
  const report = `### 类型\n${kind}\n\n### 描述\n${detail}\n\n### 环境\n${supportDiagnostics}\n`;

  const copyReport = () => {
    if (!copyText) return;
    void copyText(report).then(showReportCopied);
  };
  const submit = () => {
    if (!openExternalUrl) return;
    const body = report.slice(0, 4000);
    const query = new URLSearchParams({ title: kind, body });
    void openExternalUrl(`${issuesUrl}/new?${query.toString()}`);
  };
  const copyGroup = () => {
    if (!copyText) return;
    void copyText("829919142").then(showFeedbackCopied);
  };

  return {
    kind,
    setKind,
    detail,
    setDetail,
    report,
    reportCopied,
    feedbackCopied,
    copyReport,
    submit,
    copyGroup,
  } as const;
}
