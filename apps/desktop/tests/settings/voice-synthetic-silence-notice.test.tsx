// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { VoiceSyntheticSilenceNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains the synthetic audio used by a voice provider test", () => {
  render(<VoiceSyntheticSilenceNotice />);

  expect(
    screen.getByText("测试会向当前服务发送一秒合成静音，不使用麦克风；服务可能计入 API 用量。"),
  ).toBeTruthy();
});
