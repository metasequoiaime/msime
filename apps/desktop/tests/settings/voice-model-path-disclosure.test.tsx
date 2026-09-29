// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { VoiceModelPathDisclosure } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test.each([true, false])("renders the model path editor with disclosure=%s", (disclosure) => {
  render(
    <VoiceModelPathDisclosure
      disclosure={disclosure}
      path="/synthetic/models/ggml.bin"
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByLabelText("Whisper 模型文件")).toBeTruthy();
  if (disclosure) {
    expect(screen.getByText("高级：手动指定 Whisper 模型文件")).toBeTruthy();
  } else {
    expect(screen.queryByText("高级：手动指定 Whisper 模型文件")).toBeNull();
  }
});
