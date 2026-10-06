// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { TouchKeyboardSkinEditor } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor";
import {
  defaultTouchKeyboardSkinDesign,
  type CustomSkinLibraryAction,
  type SavedTouchKeyboardSkin,
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

test("ignores a stale library load after the client changes", async () => {
  let resolveFirst!: (items: SavedTouchKeyboardSkin[]) => void;
  let resolveSecond!: (items: SavedTouchKeyboardSkin[]) => void;
  const firstItem = { id: "old", name: "旧皮肤", design: defaultTouchKeyboardSkinDesign };
  const secondItem = { id: "new", name: "新皮肤", design: defaultTouchKeyboardSkinDesign };
  const first = {
    load: vi.fn(() => new Promise<SavedTouchKeyboardSkin[]>((resolve) => (resolveFirst = resolve))),
    mutate: vi.fn(),
  };
  const second = {
    load: vi.fn(
      () => new Promise<SavedTouchKeyboardSkin[]>((resolve) => (resolveSecond = resolve)),
    ),
    mutate: vi.fn(),
  };
  const props = {
    design: defaultTouchKeyboardSkinDesign,
    selected: false,
    theme: "light" as const,
    onChange: vi.fn(),
    onUse: vi.fn(),
    onClose: vi.fn(),
  };
  const { rerender } = render(<TouchKeyboardSkinEditor {...props} library={first} />);
  await waitFor(() => expect(first.load).toHaveBeenCalledOnce());
  rerender(<TouchKeyboardSkinEditor {...props} library={second} />);
  await waitFor(() => expect(second.load).toHaveBeenCalledOnce());

  resolveSecond([secondItem]);
  fireEvent.click(screen.getByRole("tab", { name: "我的" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "应用已保存皮肤 新皮肤" })).toBeTruthy(),
  );
  await act(async () => resolveFirst([firstItem]));
  expect(screen.getByRole("button", { name: "应用已保存皮肤 新皮肤" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "应用已保存皮肤 旧皮肤" })).toBeNull();
});

test("ignores a stale save while the replacement library is loading", async () => {
  let finishSave!: (items: SavedTouchKeyboardSkin[]) => void;
  let finishLoad!: (items: SavedTouchKeyboardSkin[]) => void;
  const first = {
    load: vi.fn().mockResolvedValue([]),
    mutate: vi.fn(() => new Promise<SavedTouchKeyboardSkin[]>((resolve) => (finishSave = resolve))),
  };
  const second = {
    load: vi.fn(() => new Promise<SavedTouchKeyboardSkin[]>((resolve) => (finishLoad = resolve))),
    mutate: vi.fn(),
  };
  const props = {
    design: defaultTouchKeyboardSkinDesign,
    selected: false,
    theme: "light" as const,
    onChange: vi.fn(),
    onUse: vi.fn(),
    onClose: vi.fn(),
  };
  const { rerender } = render(<TouchKeyboardSkinEditor {...props} library={first} />);
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "保存设计" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "保存设计" }));
  fireEvent.click(screen.getByRole("button", { name: "确认保存" }));
  expect(first.mutate).toHaveBeenCalledOnce();
  rerender(<TouchKeyboardSkinEditor {...props} library={second} />);
  expect(second.load).toHaveBeenCalledOnce();
  await act(async () => finishSave([]));
  expect((screen.getByRole("button", { name: "确认保存" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(screen.queryByRole("status")).toBeNull();
  expect(props.onUse).not.toHaveBeenCalled();
  await act(async () => finishLoad([]));
  expect((screen.getByRole("button", { name: "确认保存" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
});
