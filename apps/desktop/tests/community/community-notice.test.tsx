// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityNotice } from "@msime/ui";

afterEach(cleanup);

test("renders shared community notice styling", () => {
  render(<CommunityNotice>暂时没有可显示的作品。</CommunityNotice>);

  const notice = screen.getByText("暂时没有可显示的作品。");
  expect(notice.tagName).toBe("P");
  expect(notice.className).toBe("my-[26px] text-center text-muted");
});
