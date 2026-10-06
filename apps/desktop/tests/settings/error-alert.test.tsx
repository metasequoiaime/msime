// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { ErrorAlert } from "@msime/ui";

afterEach(cleanup);

test("renders arbitrary content as an error alert", () => {
  render(
    <ErrorAlert>
      请求失败。 <button type="button">重试</button>
    </ErrorAlert>,
  );

  const alert = screen.getByRole("alert");
  expect(alert.className).toBe("error");
  expect(alert.textContent).toContain("请求失败");
  expect(screen.getByRole("button", { name: "重试" })).not.toBeNull();
});

test("forwards paragraph attributes and supports a surface class", () => {
  render(
    <ErrorAlert className="warning" data-kind="photo">
      照片读取失败
    </ErrorAlert>,
  );

  const alert = screen.getByRole("alert");
  expect(alert.className).toBe("warning");
  expect(alert.getAttribute("data-kind")).toBe("photo");
});
