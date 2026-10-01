// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { NiuTransSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards credential edits and leaves choosing the service to 翻译服务", () => {
  const onAppIdChange = vi.fn();
  const onApiKeyChange = vi.fn();
  render(
    <NiuTransSection
      available
      appId="synthetic-app"
      apiKey="synthetic-key"
      onAppIdChange={onAppIdChange}
      onApiKeyChange={onApiKeyChange}
    />,
  );

  // The service is turned on only from the 翻译服务 select, so its own settings carry no switch.
  expect(screen.queryByRole("switch")).toBeNull();
  fireEvent.change(screen.getByLabelText("NiuTrans App ID"), {
    target: { value: "updated-app" },
  });
  fireEvent.change(screen.getByLabelText("NiuTrans API Key"), {
    target: { value: "updated-key" },
  });
  expect(onAppIdChange).toHaveBeenCalledWith("updated-app");
  expect(onApiKeyChange).toHaveBeenCalledWith("updated-key");
});

test("disables fields when candidate translations are unavailable", () => {
  render(
    <NiuTransSection
      available={false}
      appId="synthetic-app"
      apiKey="synthetic-key"
      onAppIdChange={vi.fn()}
      onApiKeyChange={vi.fn()}
    />,
  );

  expect((screen.getByLabelText("NiuTrans App ID") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("NiuTrans API Key") as HTMLInputElement).disabled).toBe(true);
});
