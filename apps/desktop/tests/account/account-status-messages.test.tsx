// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { AccountStatusMessages } from "@msime/ui";

afterEach(cleanup);

const accountPage = Object.values(
  import.meta.glob<string>("../../../../packages/ui/src/account/account-page.tsx", {
    eager: true,
    query: "?raw",
    import: "default",
  }),
)[0];

test("shared account status messages render errors and notices with the requested notice class", () => {
  render(
    <AccountStatusMessages
      error="合成错误"
      notice="合成成功"
      noticeClassName="notice account-status"
    />,
  );

  expect(screen.getByRole("alert").textContent).toBe("合成错误");
  expect(screen.getByRole("status").textContent).toBe("合成成功");
  expect(screen.getByRole("status").className).toBe("notice account-status");
});

test("account surfaces delegate status rendering to the shared component", () => {
  expect(accountPage).toContain(
    'import { AccountStatusMessages } from "./account-status-messages";',
  );
  expect(accountPage).toContain("<AccountStatusMessages");
  expect(accountPage).not.toContain('<p role="alert" className="error">');
});
