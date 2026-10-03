// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { CommunityField } from "@msime/ui";

afterEach(cleanup);

test("shared community field wraps its label and control with field styling", () => {
  render(
    <CommunityField label="字段名称">
      <input aria-label="字段控件" />
    </CommunityField>,
  );

  const control = screen.getByLabelText("字段控件");
  const field = control.closest("label");

  expect(field).toBeTruthy();
  expect(field?.textContent).toContain("字段名称");
  expect(field?.className).toContain("flex flex-col gap-[7px]");
});
