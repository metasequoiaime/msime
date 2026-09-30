// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type EntryFormProps = {
  value: { code: string; word: string; weight: number };
  busy?: boolean;
  onChange: (patch: Partial<EntryFormProps["value"]>) => void;
  onSave: () => void;
  onCancel: () => void;
};

test("shared cloud dictionary entry form forwards field edits and actions", () => {
  const EntryForm = (ui as unknown as { CloudDictionaryEntryForm: ComponentType<EntryFormProps> })
    .CloudDictionaryEntryForm;
  expect(EntryForm).toBeDefined();

  const onChange = vi.fn();
  const onSave = vi.fn();
  const onCancel = vi.fn();
  render(
    <EntryForm
      value={{ code: "ni", word: "你", weight: 100 }}
      onChange={onChange}
      onSave={onSave}
      onCancel={onCancel}
    />,
  );

  fireEvent.change(screen.getByLabelText("编码"), { target: { value: "ni hao" } });
  fireEvent.change(screen.getByLabelText("词条"), { target: { value: "你好" } });
  fireEvent.change(screen.getByLabelText("权重"), { target: { value: "200" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));

  expect(onChange).toHaveBeenNthCalledWith(1, { code: "ni hao" });
  expect(onChange).toHaveBeenNthCalledWith(2, { word: "你好" });
  expect(onChange).toHaveBeenNthCalledWith(3, { weight: 200 });
  expect(onSave).toHaveBeenCalledOnce();
  expect(onCancel).toHaveBeenCalledOnce();
});
