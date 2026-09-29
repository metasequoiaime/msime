// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { NiuTransSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards the toggle and credential edits", () => {
  const onToggle = vi.fn();
  const onAppIdChange = vi.fn();
  const onApiKeyChange = vi.fn();
  render(
    <NiuTransSection
      enabled
      available
      appId="synthetic-app"
      apiKey="synthetic-key"
      onToggle={onToggle}
      onAppIdChange={onAppIdChange}
      onApiKeyChange={onApiKeyChange}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "小牛翻译（NiuTrans）" }));
  fireEvent.change(screen.getByLabelText("NiuTrans App ID"), {
    target: { value: "updated-app" },
  });
  fireEvent.change(screen.getByLabelText("NiuTrans API Key"), {
    target: { value: "updated-key" },
  });
  expect(onToggle).toHaveBeenCalledWith(false);
  expect(onAppIdChange).toHaveBeenCalledWith("updated-app");
  expect(onApiKeyChange).toHaveBeenCalledWith("updated-key");
});

test("disables fields when candidate translations are unavailable", () => {
  render(
    <NiuTransSection
      enabled
      available={false}
      appId="synthetic-app"
      apiKey="synthetic-key"
      onToggle={vi.fn()}
      onAppIdChange={vi.fn()}
      onApiKeyChange={vi.fn()}
    />,
  );

  expect(
    (screen.getByRole("switch", { name: "小牛翻译（NiuTrans）" }) as HTMLInputElement).disabled,
  ).toBe(true);
  expect((screen.getByLabelText("NiuTrans App ID") as HTMLInputElement).disabled).toBe(true);
});
