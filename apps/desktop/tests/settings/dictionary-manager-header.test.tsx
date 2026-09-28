// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DictionaryManagerHeader } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("dispatches dictionary actions and detects Rime imports", () => {
  const onQuery = vi.fn();
  const onAdd = vi.fn();
  const onExportCurrent = vi.fn();
  const onExportAll = vi.fn();
  const onImport = vi.fn();
  const onFormatChange = vi.fn();
  const { container } = render(
    <DictionaryManagerHeader
      disabled={false}
      dictionaryFormat="standard"
      onQuery={onQuery}
      onAdd={onAdd}
      onExportCurrent={onExportCurrent}
      onExportAll={onExportAll}
      onImport={onImport}
      onFormatChange={onFormatChange}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  fireEvent.click(screen.getByRole("button", { name: "新增词条" }));
  fireEvent.click(screen.getByRole("button", { name: "导出当前类型" }));
  fireEvent.click(screen.getByRole("button", { name: "导出全部" }));
  const file = new File(["synthetic"], "fixture.yaml", { type: "text/yaml" });
  fireEvent.change(container.querySelector('input[type="file"]')!, {
    target: { files: [file] },
  });

  expect(onQuery).toHaveBeenCalledTimes(1);
  expect(onAdd).toHaveBeenCalledTimes(1);
  expect(onExportCurrent).toHaveBeenCalledTimes(1);
  expect(onExportAll).toHaveBeenCalledTimes(1);
  expect(onFormatChange).toHaveBeenCalledWith("rime");
  expect(onImport).toHaveBeenCalledWith(file);
});

test("disables export and import controls while busy", () => {
  const { container } = render(
    <DictionaryManagerHeader
      disabled
      dictionaryFormat="hans"
      onQuery={vi.fn()}
      onAdd={vi.fn()}
      onExportCurrent={vi.fn()}
      onExportAll={vi.fn()}
      onImport={vi.fn()}
      onFormatChange={vi.fn()}
    />,
  );

  expect((screen.getByRole("button", { name: "查询" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "导出当前类型" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect((container.querySelector('input[type="file"]') as HTMLInputElement).disabled).toBe(true);
});
