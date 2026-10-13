// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceModelSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("is a plain field when the provider names no models", () => {
  const onChange = vi.fn();
  render(<VoiceModelSection models={[]} value="model-v1" onChange={onChange} />);

  expect(screen.getByText("由语音服务选择对应模型")).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", { name: "识别模型" }), {
    target: { value: "model-v2" },
  });
  expect(onChange).toHaveBeenCalledWith("model-v2");
});

test("offers the provider's models in one list", () => {
  const onChange = vi.fn();
  render(
    <VoiceModelSection models={["model-v1", "model-v2"]} value="model-v1" onChange={onChange} />,
  );

  const select = screen.getByRole("combobox", { name: "识别模型" }) as HTMLSelectElement;
  expect(select.value).toBe("model-v1");
  expect(screen.queryByRole("textbox")).toBeNull();
  fireEvent.change(select, { target: { value: "model-v2" } });
  expect(onChange).toHaveBeenCalledWith("model-v2");
});
