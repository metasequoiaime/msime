// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FeedbackChannels } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders shared feedback channels and routes their actions", () => {
  const onOpenIssues = vi.fn();
  const onCopyGroup = vi.fn();
  const onOpenTelegram = vi.fn();
  render(
    <FeedbackChannels
      issuesUrl="https://example.test/issues"
      feedbackCopied={false}
      onOpenIssues={onOpenIssues}
      onCopyGroup={onCopyGroup}
      onOpenTelegram={onOpenTelegram}
      cardClassName="card"
      iconClassName="icon"
      bodyClassName="body"
      titleClassName="title"
    />,
  );

  expect(screen.getByText("example.test/issues")).toBeTruthy();
  const marks = [...document.querySelectorAll(".icon")];
  expect(marks.map((mark) => mark.textContent)).toEqual(["", "", ""]);
  expect(marks[0].querySelector("span")?.getAttribute("style")).toContain("--channel-icon: url(");
  const brandSources = marks.slice(1).map((mark) => mark.querySelector("img")?.getAttribute("src"));
  expect(brandSources.every(Boolean)).toBe(true);
  expect(new Set(brandSources).size).toBe(2);
  fireEvent.click(screen.getByRole("button", { name: "查看 Issues" }));
  fireEvent.click(screen.getByRole("button", { name: "复制群号" }));
  fireEvent.click(screen.getByRole("button", { name: "打开群组" }));
  expect(onOpenIssues).toHaveBeenCalledOnce();
  expect(onCopyGroup).toHaveBeenCalledOnce();
  expect(onOpenTelegram).toHaveBeenCalledOnce();
});

test("shows the copied state on the QQ action", () => {
  render(
    <FeedbackChannels
      issuesUrl="https://example.test/issues"
      feedbackCopied
      onOpenIssues={vi.fn()}
      onCopyGroup={vi.fn()}
      onOpenTelegram={vi.fn()}
      cardClassName="card"
      iconClassName="icon"
      bodyClassName="body"
      titleClassName="title"
    />,
  );

  expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy();
});
