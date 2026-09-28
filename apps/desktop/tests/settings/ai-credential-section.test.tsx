// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AiCredentialSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("shows a mismatch and saves the current token configuration", () => {
  const onSave = vi.fn();
  render(
    <AiCredentialSection
      endpoint="https://ai.example.test/v1"
      model="new-model"
      origin="https://ai.example.test:443"
      token="synthetic-token"
      stored={{ endpoint: "https://old.example.test/v1", model: "old-model" }}
      invalid={false}
      busy={false}
      onTokenChange={vi.fn()}
      onSave={onSave}
      onClear={vi.fn()}
    />,
  );

  expect(screen.getByText(/已保存的凭据绑定/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect(onSave).toHaveBeenCalledOnce();
});

test("disables token input without a valid origin and exposes clear", () => {
  const onClear = vi.fn();
  render(
    <AiCredentialSection
      endpoint="invalid"
      model="model"
      origin={null}
      token="synthetic-token"
      stored={{ endpoint: "invalid", model: "model" }}
      invalid={true}
      busy={false}
      onTokenChange={vi.fn()}
      onSave={vi.fn()}
      onClear={onClear}
    />,
  );

  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(true);
  expect(screen.getByText(/现有 ai-provider.json 无效/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "清除凭据" }));
  expect(onClear).toHaveBeenCalledOnce();
});
