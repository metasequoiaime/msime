import { expect, test, vi } from "vitest";
import { createSettingsReloadAction } from "@msime/ui";

test("reloads immediately when settings are clean", async () => {
  const reload = vi.fn().mockResolvedValue(undefined);
  const action = createSettingsReloadAction({ dirty: false, reload, confirm: vi.fn() });

  await action();

  expect(reload).toHaveBeenCalledOnce();
});

test("confirms before discarding dirty settings", async () => {
  const reload = vi.fn().mockResolvedValue(undefined);
  const confirm = vi.fn().mockResolvedValue(true);
  const action = createSettingsReloadAction({ dirty: true, reload, confirm });

  await action();

  expect(confirm).toHaveBeenCalledWith({
    title: "重新读取",
    message: "尚未保存的修改会被放弃。",
    confirmLabel: "放弃并重新读取",
    danger: true,
  });
  expect(reload).toHaveBeenCalledOnce();
});

test("does not reload when discarding is canceled", async () => {
  const reload = vi.fn().mockResolvedValue(undefined);
  const action = createSettingsReloadAction({
    dirty: true,
    reload,
    confirm: vi.fn().mockResolvedValue(false),
  });

  await action();

  expect(reload).not.toHaveBeenCalled();
});
