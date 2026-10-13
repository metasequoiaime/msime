// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProviderPresetSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("opens provider documentation when available", () => {
  const openExternalUrl = vi.fn(async () => {});
  render(
    <ProviderPresetSection
      label="AI "
      preset={{ documentation: "https://docs.example.test/api" }}
      openExternalUrl={openExternalUrl}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "AI 接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://docs.example.test/api");
});

test("leaves the models to the model picker and renders nothing without a link", () => {
  render(
    <ProviderPresetSection
      label="识别服务"
      preset={{ models: ["model-a"], documentation: "https://docs.example.test/api" }}
    />,
  );

  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.queryByRole("button")).toBeNull();
});
