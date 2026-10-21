import { useState } from "react";
import { useTemporaryFlag } from "../core/use-temporary-flag";
import { externalUrlMaxLength } from "./use-external-url";
import { useMountedRef } from "./use-mounted-ref";

function issueReportUrl(issuesUrl: string, title: string, report: string): string {
  const characters = Array.from(report);
  const urlFor = (length: number) => {
    const query = new URLSearchParams({ title, body: characters.slice(0, length).join("") });
    return `${issuesUrl}/new?${query.toString()}`;
  };
  // 宿主限制编码后的完整网址；按码点裁剪，避免截断百分号编码或代理对。
  let low = 0;
  let high = characters.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (urlFor(middle).length <= externalUrlMaxLength) low = middle;
    else high = middle - 1;
  }
  return urlFor(low);
}

export interface UseFeedbackReportOptions {
  supportDiagnostics: string;
  issuesUrl: string;
  copyText?: (text: string) => Promise<void>;
  openExternalUrl?: (url: string) => Promise<void>;
  setError?: (message: string) => void;
}

/** 管理反馈草稿、报告序列化和剪贴板操作。 */
export function useFeedbackReport({
  supportDiagnostics,
  issuesUrl,
  copyText,
  openExternalUrl,
  setError,
}: UseFeedbackReportOptions) {
  const mounted = useMountedRef();
  const [kind, setKind] = useState("功能异常");
  const [detail, setDetail] = useState("");
  const [reportCopied, showReportCopied] = useTemporaryFlag(1600);
  const [feedbackCopied, showFeedbackCopied] = useTemporaryFlag(1600);
  const report = `### 类型\n${kind}\n\n### 描述\n${detail}\n\n### 环境\n${supportDiagnostics}\n`;

  async function runHostAction(
    action: () => Promise<void>,
    failure: string,
    onSuccess?: () => void,
  ) {
    if (!mounted.current) return;
    setError?.("");
    try {
      await action();
      if (mounted.current) onSuccess?.();
    } catch {
      if (mounted.current) setError?.(failure);
    }
  }

  const copyReport = () => {
    if (!copyText) return;
    return runHostAction(() => copyText(report), "复制报告失败，请重试。", showReportCopied);
  };
  const submit = () => {
    if (!openExternalUrl) return;
    return runHostAction(
      () => openExternalUrl(issueReportUrl(issuesUrl, kind, report)),
      "无法打开外部链接，请稍后重试。",
    );
  };
  const copyGroup = () => {
    if (!copyText) return;
    return runHostAction(() => copyText("829919142"), "复制群号失败，请重试。", showFeedbackCopied);
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
