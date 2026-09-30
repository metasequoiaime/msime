// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type Kind = "pinyin" | "wubi" | "quick" | "english";
type CloudDictionaryKindSelectProps = {
  value: Kind;
  disabled?: boolean;
  onChange: (kind: Kind) => void;
};

test("cloud dictionary kind select renders shared options and forwards changes", () => {
  const Select = (
    ui as unknown as { CloudDictionaryKindSelect: ComponentType<CloudDictionaryKindSelectProps> }
  ).CloudDictionaryKindSelect;
  expect(Select).toBeDefined();

  const onChange = vi.fn();
  render(<Select value="pinyin" onChange={onChange} />);

  expect((screen.getByRole("combobox", { name: "词库类型" }) as HTMLSelectElement).value).toBe(
    "pinyin",
  );
  fireEvent.change(screen.getByRole("combobox", { name: "词库类型" }), {
    target: { value: "english" },
  });
  expect(onChange).toHaveBeenCalledWith("english");
});
