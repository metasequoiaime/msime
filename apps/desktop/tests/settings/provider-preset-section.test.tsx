// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProviderPresetSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects a known model and leaves a custom model untouched", () => {
  const onSelectModel = vi.fn();
  render(
    <ProviderPresetSection
      label="识别服务"
      preset={{ models: ["model-a", "model-b"] }}
      model="custom-model"
      onSelectModel={onSelectModel}
    />,
  );

  const select = screen.getByRole("combobox", { name: "识别服务预置模型" }) as HTMLSelectElement;
  expect(select.value).toBe("");
  fireEvent.change(select, { target: { value: "model-b" } });
  expect(onSelectModel).toHaveBeenCalledWith("model-b");
  fireEvent.change(select, { target: { value: "" } });
  expect(onSelectModel).toHaveBeenCalledTimes(1);
});

test("opens provider documentation when available", () => {
  const openExternalUrl = vi.fn(async () => {});
  render(
    <ProviderPresetSection
      label="AI "
      preset={{ documentation: "https://docs.example.test/api" }}
      model="model-a"
      onSelectModel={vi.fn()}
      openExternalUrl={openExternalUrl}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "AI 接入说明与 API Key" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://docs.example.test/api");
});

test("renders nothing without models or an external link", () => {
  render(
    <ProviderPresetSection label="自定义" preset={{}} model="model-a" onSelectModel={vi.fn()} />,
  );

  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.queryByRole("button")).toBeNull();
});
