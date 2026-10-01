// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { TouchKeyboardSkinEditor } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor";
import {
  defaultTouchKeyboardSkinDesign,
  type CustomSkinLibraryAction,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(() => cleanup());

test("ignores duplicate skin saves while the first save is pending", async () => {
  let resolveMutation!: (items: never[]) => void;
  const mutate = vi.fn(
    (_action: CustomSkinLibraryAction) =>
      new Promise<never[]>((resolve) => {
        resolveMutation = resolve;
      }),
  );
  const library = {
    load: vi.fn().mockResolvedValue([]),
    mutate,
  };
  render(
    <TouchKeyboardSkinEditor
      design={defaultTouchKeyboardSkinDesign}
      selected={false}
      theme="light"
      library={library}
      onChange={vi.fn()}
      onUse={vi.fn()}
      onClose={vi.fn()}
    />,
  );

  await waitFor(() => expect(library.load).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "保存设计" }));
  fireEvent.change(screen.getByLabelText("皮肤名称"), { target: { value: "合成皮肤" } });
  const confirm = screen.getByRole("button", { name: "确认保存" });
  await act(async () => {
    fireEvent.click(confirm);
    fireEvent.click(confirm);
  });
  expect(mutate).toHaveBeenCalledOnce();
  resolveMutation([]);
  await waitFor(() => expect(mutate).toHaveBeenCalledOnce());
});
