// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModelPickerRows } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Picker({ models, initial }: { models: readonly string[]; initial: string }) {
  const [model, setModel] = useState(initial);
  return (
    <>
      <ModelPickerRows
        title="模型"
        label="AI 模型"
        models={models}
        model={model}
        onChange={setModel}
      />
      <output aria-label="当前模型">{model}</output>
    </>
  );
}

const current = () => screen.getByRole("status", { name: "当前模型" }).textContent;
const select = () => screen.getByRole("combobox", { name: "AI 模型" }) as HTMLSelectElement;
const customField = () => screen.queryByRole("textbox", { name: "AI 模型（自定义）" });

test("one list holds every known model once, with 自定义… last", () => {
  render(<Picker models={["glm-5.3-flash", "glm-5", "glm-5.3-flash", ""]} initial="glm-5" />);

  expect([...select().options].map((option) => option.textContent)).toEqual([
    "glm-5.3-flash",
    "glm-5",
    "自定义…",
  ]);
  expect(select().value).toBe("glm-5");
  expect(customField()).toBeNull();
  expect(screen.getAllByRole("combobox")).toHaveLength(1);
});

test("choosing a model sets it and 自定义… opens a field with the current model", () => {
  render(<Picker models={["a", "b"]} initial="a" />);

  fireEvent.change(select(), { target: { value: "b" } });
  expect(current()).toBe("b");
  fireEvent.change(select(), { target: { value: "" } });
  expect(current()).toBe("b");
  expect((customField() as HTMLInputElement).value).toBe("b");

  fireEvent.change(customField()!, { target: { value: "my-model" } });
  expect(current()).toBe("my-model");
  fireEvent.change(select(), { target: { value: "a" } });
  expect(current()).toBe("a");
  expect(customField()).toBeNull();
});

test("a model outside the list shows as 自定义… with its field", () => {
  render(<Picker models={["a", "b"]} initial="my-model" />);

  expect(select().value).toBe("");
  expect((customField() as HTMLInputElement).value).toBe("my-model");
});

test("typing a known name into the field keeps the field open", () => {
  render(<Picker models={["glm-5", "glm-5.3-flash"]} initial="custom" />);

  fireEvent.change(customField()!, { target: { value: "glm-5" } });
  expect(current()).toBe("glm-5");
  expect(customField()).not.toBeNull();
});

test("a new list goes back to choosing from it", () => {
  const { rerender } = render(
    <ModelPickerRows title="模型" label="AI 模型" models={["a"]} model="a" onChange={vi.fn()} />,
  );
  fireEvent.change(select(), { target: { value: "" } });
  expect(customField()).not.toBeNull();

  rerender(
    <ModelPickerRows
      title="模型"
      label="AI 模型"
      models={["a", "b"]}
      model="a"
      onChange={vi.fn()}
    />,
  );
  expect(customField()).toBeNull();
  expect(select().value).toBe("a");
});

test("without known models it is just the field", () => {
  render(<Picker models={[]} initial="x" />);

  expect(screen.queryByRole("combobox")).toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "AI 模型" }), { target: { value: "y" } });
  expect(current()).toBe("y");
});
