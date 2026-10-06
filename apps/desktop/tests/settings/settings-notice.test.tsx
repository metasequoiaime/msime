// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsNotice } from "@msime/ui";

afterEach(cleanup);

test("renders the shared settings notice with an optional live role", () => {
  render(<SettingsNotice role="status">已保存。</SettingsNotice>);

  const notice = screen.getByRole("status");
  expect(notice.className).toBe("notice");
  expect(notice.textContent).toBe("已保存。");
});
