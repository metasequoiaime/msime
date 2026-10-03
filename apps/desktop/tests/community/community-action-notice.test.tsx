// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityActionNotice } from "@msime/ui";

afterEach(cleanup);

test("shared community action notice exposes its message as a status", () => {
  render(<CommunityActionNotice>已完成合成操作。</CommunityActionNotice>);

  const notice = screen.getByRole("status");
  expect(notice.className).toContain("rounded");
  expect(notice.textContent).toBe("已完成合成操作。");
});
