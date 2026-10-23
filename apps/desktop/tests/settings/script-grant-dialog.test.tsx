// @vitest-environment jsdom
import { useState } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { ScriptGrantDialog } from "../../../../packages/ui/src/settings/script-grant-dialog";
import type { PluginPackage } from "../../../../packages/ui/src/settings/plugin-types";

afterEach(cleanup);

const pack: PluginPackage = {
  id: "synthetic-script",
  name: "合成脚本",
  kind: "script",
  version: "1.0.0",
  license: "CC0-1.0",
  author: null,
  description: null,
  builtin: false,
  permissions: ["read_commit"],
};

test("授权写入期间禁止按钮、Escape 和遮罩取消，失败后保留选择并允许重试", async () => {
  let rejectWrite!: (error: unknown) => void;
  const write = vi
    .fn()
    .mockImplementationOnce(() => new Promise<void>((_resolve, reject) => (rejectWrite = reject)))
    .mockResolvedValueOnce(undefined);
  const onCancel = vi.fn();
  function Dialog() {
    const [open, setOpen] = useState(true);
    return open ? (
      <ScriptGrantDialog
        pack={pack}
        initial={[]}
        stale={false}
        onConfirm={async (granted) => {
          await write(granted);
          setOpen(false);
        }}
        onCancel={onCancel}
      />
    ) : null;
  }
  render(<Dialog />);
  const cancel = screen.getByRole<HTMLButtonElement>("button", { name: "取消" });
  expect(document.activeElement).toBe(cancel);
  const checkbox = screen.getByRole<HTMLInputElement>("checkbox", { name: /^读取输入的文字/ });
  fireEvent.click(checkbox);
  const confirm = screen.getByRole<HTMLButtonElement>("button", { name: "授权并启用" });
  fireEvent.click(confirm);
  expect(confirm.disabled).toBe(true);
  expect(cancel.disabled).toBe(true);
  expect(checkbox.disabled).toBe(true);
  const dialog = screen.getByRole("dialog");
  fireEvent.keyDown(dialog, { key: "Escape" });
  fireEvent.mouseDown(dialog.parentElement!);
  fireEvent.click(cancel);
  expect(onCancel).not.toHaveBeenCalled();
  expect(write).toHaveBeenCalledExactlyOnceWith(["read_commit"]);

  await act(async () => rejectWrite({ code: "plugin_script_memory" }));
  expect(screen.getByRole("alert").textContent).toContain("超出输入法留给脚本插件的内存");
  expect(checkbox.checked).toBe(true);
  expect(confirm.disabled).toBe(false);
  expect(cancel.disabled).toBe(false);
  expect(checkbox.disabled).toBe(false);
  await act(async () => fireEvent.click(confirm));
  expect(write).toHaveBeenLastCalledWith(["read_commit"]);
  expect(write).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("dialog")).toBeNull();
});
