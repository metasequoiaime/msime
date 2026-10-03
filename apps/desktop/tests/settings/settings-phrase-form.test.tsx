// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { SettingsPhraseForm } from "@msime/ui";

afterEach(cleanup);

test("renders the shared phrase form layout", () => {
  render(<SettingsPhraseForm>词条表单</SettingsPhraseForm>);

  const form = screen.getByText("词条表单");
  expect(form.tagName).toBe("DIV");
  expect(form.className).toBe("mt-3.5 flex flex-wrap items-end gap-2.5");
});

test("forwards attributes and appends a local class", () => {
  render(
    <SettingsPhraseForm className="compact" data-testid="phrase-form">
      词条
    </SettingsPhraseForm>,
  );

  const form = screen.getByTestId("phrase-form");
  expect(form.className).toContain("compact");
});
