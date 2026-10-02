// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsFeedbackPage } from "@msime/ui";

test("keeps the feedback fieldset boundary and controls", () => {
  render(
    <SettingsFeedbackPage
      disabled={false}
      hidden={false}
      hero="hero"
      eyebrow="eyebrow"
      heroTitle="title"
      note="note"
      feedbackList="list"
      feedbackCard="card"
      feedbackIcon="icon"
      feedbackBody="body"
      feedbackTitle="feedback"
      serviceRow="service"
      kind="功能异常"
      detail=""
      reportCopied={false}
      feedbackCopied={false}
      supportDiagnostics="synthetic diagnostics"
      issuesUrl="https://example.invalid/issues"
      onKindChange={vi.fn()}
      onDetailChange={vi.fn()}
      onCopyReport={vi.fn()}
      onSubmitFeedback={vi.fn()}
      onOpenIssues={vi.fn()}
      onCopyGroup={vi.fn()}
      onOpenTelegram={vi.fn()}
    />,
  );

  expect(screen.getByRole("group", { name: "帮助与反馈" })).toBeTruthy();
  expect(screen.getByText("synthetic diagnostics")).toBeTruthy();
});
