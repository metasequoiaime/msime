// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceCredentialFieldsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards App Key and token edits", () => {
  const onAppKeyChange = vi.fn();
  const onTokenChange = vi.fn();
  render(
    <VoiceCredentialFieldsSection
      showAppKey
      appKey="synthetic-app"
      tokenLabel="识别 API Token"
      token="synthetic-token"
      onAppKeyChange={onAppKeyChange}
      onTokenChange={onTokenChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("Doubao App Key"), {
    target: { value: "updated-app" },
  });
  fireEvent.change(screen.getByLabelText("识别 API Token"), {
    target: { value: "updated-token" },
  });

  expect(onAppKeyChange).toHaveBeenCalledWith("updated-app");
  expect(onTokenChange).toHaveBeenCalledWith("updated-token");
});

test("hides App Key and uses the dynamic token label", () => {
  render(
    <VoiceCredentialFieldsSection
      showAppKey={false}
      appKey=""
      tokenLabel="Doubao API Key"
      token=""
      onAppKeyChange={vi.fn()}
      onTokenChange={vi.fn()}
    />,
  );

  expect(screen.queryByLabelText("Doubao App Key")).toBeNull();
  expect(screen.getByLabelText("Doubao API Key")).toBeTruthy();
  expect(screen.getByText("仅保存在本机设置中")).toBeTruthy();
});
