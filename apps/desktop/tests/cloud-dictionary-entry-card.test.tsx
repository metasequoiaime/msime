// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

type EntryCardProps = {
  entry: { code: string; word: string; weight: number };
  editLabel: string;
  onEdit: () => void;
  onRemove: () => void;
  onDownload?: () => void;
  busy?: boolean;
};

test("shared cloud dictionary entry card forwards edit, remove, and optional download actions", () => {
  const EntryCard = (ui as unknown as { CloudDictionaryEntryCard: ComponentType<EntryCardProps> })
    .CloudDictionaryEntryCard;
  expect(EntryCard).toBeDefined();

  const onEdit = vi.fn();
  const onRemove = vi.fn();
  const onDownload = vi.fn();
  render(
    <EntryCard
      entry={{ code: "ni", word: "你", weight: 100 }}
      editLabel="编辑云词条 你"
      onEdit={onEdit}
      onRemove={onRemove}
      onDownload={onDownload}
    />,
  );

  expect(screen.getByRole("button", { name: "编辑云词条 你" })).toBeTruthy();
  expect(screen.getByText("ni · 权重 100")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "编辑云词条 你" }));
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  fireEvent.click(screen.getByRole("button", { name: "下载到本机 你" }));

  expect(onEdit).toHaveBeenCalledTimes(2);
  expect(onRemove).toHaveBeenCalledOnce();
  expect(onDownload).toHaveBeenCalledOnce();
});

test("shared cloud dictionary entry card omits download when unavailable and disables actions when busy", () => {
  const EntryCard = (ui as unknown as { CloudDictionaryEntryCard: ComponentType<EntryCardProps> })
    .CloudDictionaryEntryCard;
  render(
    <EntryCard
      entry={{ code: "hao", word: "好", weight: 80 }}
      editLabel="编辑完整目录词条 好"
      onEdit={vi.fn()}
      onRemove={vi.fn()}
      busy
    />,
  );

  expect(screen.queryByRole("button", { name: /下载到本机/ })).toBeNull();
  expect(screen.getByRole("button", { name: "编辑完整目录词条 好" })).toHaveProperty(
    "disabled",
    true,
  );
  expect(screen.getByRole("button", { name: "编辑" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "删除" })).toHaveProperty("disabled", true);
});
