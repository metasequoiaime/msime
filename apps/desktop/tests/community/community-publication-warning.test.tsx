// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityPublicationWarning } from "@msime/ui";

afterEach(cleanup);

test("renders publication warnings with the shared warning styling", () => {
  render(<CommunityPublicationWarning>发布后内容会公开展示。</CommunityPublicationWarning>);

  const warning = screen.getByText("发布后内容会公开展示。");
  expect(warning.textContent).toBe("发布后内容会公开展示。");
  expect(warning.className).toBe("m-0 text-xs leading-relaxed text-muted");
});
