// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModelSettingField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled model select and forwards known model choices", () => {
  const onSelect = vi.fn();
  render(
    <ModelSettingField
      label="预置模型"
      inputLabel="Synthetic 预置模型"
      description="服务商支持的模型"
      models={["synthetic-model", "other-model"]}
      model="synthetic-model"
      emptyLabel="选择模型…"
      onSelect={onSelect}
    />,
  );

  const select = screen.getByLabelText("Synthetic 预置模型") as HTMLSelectElement;
  expect(screen.getByText("预置模型")).toBeTruthy();
  expect(screen.getByText("服务商支持的模型")).toBeTruthy();
  expect(select.value).toBe("synthetic-model");

  fireEvent.change(select, { target: { value: "other-model" } });
  expect(onSelect).toHaveBeenCalledWith("other-model");
});

test("uses the empty option for a custom model", () => {
  render(
    <ModelSettingField
      label="模型"
      inputLabel="AI 模型目录"
      models={["synthetic-model"]}
      model="custom-model"
      emptyLabel="自定义模型…"
      onSelect={vi.fn()}
    />,
  );

  const select = screen.getByLabelText("AI 模型目录") as HTMLSelectElement;
  expect(select.value).toBe("");
  expect(select.options[0]?.textContent).toBe("自定义模型…");
});
