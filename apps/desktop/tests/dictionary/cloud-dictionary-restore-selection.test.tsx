// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudDictionaryFilesPanel } from "@msime/ui";
import { createDesktopCloudDictionaryClient } from "../../src/dictionary/desktop-cloud-dictionary-client";
import { answerConfirm } from "../support/confirm";

const firstPreview = {
  snapshot: {
    cloudRevision: 7,
    sha256: "a".repeat(64),
    bytes: 20,
    records: 1,
    entries: 1,
    overlays: 0,
    positions: 0,
    selections: 0,
  },
  expectedRevision: 12,
};

function selectSnapshot(file: File) {
  fireEvent.change(screen.getByLabelText("选择快照恢复到云端"), {
    target: { files: [file] },
  });
}

afterEach(cleanup);

test.each(["empty", "oversized", "unreadable", "invalid"] as const)(
  "重新选择 %s 快照后不能继续恢复上一个文件",
  async (failure) => {
    const request = vi.fn().mockResolvedValue(firstPreview);
    const client = createDesktopCloudDictionaryClient({ close: vi.fn(), request }, "linux", {});
    render(<CloudDictionaryFilesPanel client={client} />);
    selectSnapshot(new File(["synthetic first snapshot"], "first.ndjson"));
    await screen.findByText(/云端 revision 7/);

    const next = new File(failure === "empty" ? [] : ["invalid snapshot"], "next.ndjson");
    if (failure === "oversized")
      Object.defineProperty(next, "size", { value: 512 * 1024 * 1024 + 1 });
    if (failure === "unreadable")
      vi.spyOn(next, "text").mockRejectedValue(new Error("synthetic read failure"));
    if (failure === "invalid") request.mockRejectedValue(new Error("invalid snapshot"));
    selectSnapshot(next);

    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        failure === "empty" || failure === "oversized"
          ? "快照文件必须大于 0 且不超过 512 MiB"
          : "无法校验快照，云端词库未改变",
      ),
    );
    expect(screen.queryByRole("button", { name: "确认恢复云端词库" })).toBeNull();
    expect(screen.queryByText("已校验快照")).toBeNull();
    expect(request).not.toHaveBeenCalledWith(
      expect.objectContaining({ operation: "snapshot_restore" }),
    );
  },
);

test("重新选择时立即撤下旧预览，校验成功后只恢复新文件", async () => {
  let complete!: (preview: typeof firstPreview) => void;
  const request = vi
    .fn()
    .mockResolvedValueOnce(firstPreview)
    .mockImplementationOnce(
      () => new Promise<typeof firstPreview>((resolve) => (complete = resolve)),
    )
    .mockResolvedValue({ revision: 23 });
  const client = createDesktopCloudDictionaryClient({ close: vi.fn(), request }, "windows", {});
  render(<CloudDictionaryFilesPanel client={client} />);
  selectSnapshot(new File(["synthetic first snapshot"], "first.ndjson"));
  await screen.findByText(/云端 revision 7/);
  selectSnapshot(new File(["synthetic second snapshot"], "second.ndjson"));
  expect(screen.queryByText("已校验快照")).toBeNull();
  expect(screen.queryByRole("button", { name: "确认恢复云端词库" })).toBeNull();
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
  await act(async () => {
    complete({
      snapshot: { ...firstPreview.snapshot, cloudRevision: 8, sha256: "b".repeat(64) },
      expectedRevision: 22,
    });
  });
  await screen.findByText(/云端 revision 8/);
  fireEvent.click(screen.getByRole("button", { name: "确认恢复云端词库" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenLastCalledWith({
      operation: "snapshot_restore",
      text: "synthetic second snapshot",
      expected_sha256: "b".repeat(64),
      revision: 22,
    }),
  );
});
