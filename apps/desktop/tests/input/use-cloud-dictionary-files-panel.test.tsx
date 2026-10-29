// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import {
  useCloudDictionaryFilesPanel,
  type CloudDictionaryFilesPanelState,
  type CloudDictionaryPanelClient,
} from "@msime/ui";
import { answerConfirm } from "../support/confirm";

type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
const snapshot: Response = {
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
function host(request = vi.fn().mockResolvedValue({})): CloudDictionaryPanelClient {
  return { request, close: vi.fn() };
}
function mount(client: CloudDictionaryPanelClient) {
  let state!: CloudDictionaryFilesPanelState;
  function Probe({ client }: { client: CloudDictionaryPanelClient }) {
    state = useCloudDictionaryFilesPanel(client);
    return state.confirmation;
  }
  const view = render(<Probe client={client} />, { reactStrictMode: true });
  return {
    get current() {
      return state;
    },
    replace(next: CloudDictionaryPanelClient) {
      view.rerender(<Probe client={next} />);
    },
  };
}
afterEach(cleanup);

test("文件 hook 在确认前不上传，并保留格式和类型切换规则", async () => {
  const client = host();
  const panel = mount(client);
  const file = new File(["ni\t你\n"], "words.tsv");
  act(() => panel.current.chooseFile(file));
  await waitFor(() => expect(panel.current.file?.name).toBe("words.tsv"));
  expect(client.request).not.toHaveBeenCalled();
  act(() => panel.current.changeKind("wubi"));
  expect(panel.current.file).toBeNull();
  expect(panel.current.format).toBe("standard");
});

test("普通导出使用现有客户端保存能力，快照先预览再确认恢复", async () => {
  const saveExport = vi.fn().mockResolvedValue(undefined);
  const request = vi
    .fn()
    .mockImplementation(async ({ operation }) =>
      operation === "export"
        ? { text: "ni\t你\n", filename: "words.tsv" }
        : operation === "snapshot_restore_preview"
          ? snapshot
          : { revision: 13 },
    );
  const panel = mount({ ...host(request), snapshot: true, saveExport });
  await act(async () => {
    await panel.current.exportDictionary();
  });
  expect(saveExport).toHaveBeenCalledWith("words.tsv", "ni\t你\n");
  const restore = new File(["fixture"], "snapshot.ndjson");
  await act(async () => {
    await panel.current.chooseRestoreSnapshot(restore);
  });
  expect(panel.current.restorePreview?.expectedRevision).toBe(12);
  expect(request).not.toHaveBeenCalledWith(
    expect.objectContaining({ operation: "snapshot_restore" }),
  );
  act(() => {
    void panel.current.restoreSnapshot();
  });
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "snapshot_restore",
      text: "fixture",
      expected_sha256: "a".repeat(64),
      revision: 12,
    }),
  );
  expect(panel.current.notice).toContain("新版本 13");
});

test("快照导出和恢复确认在替换客户端后拒绝旧宿主", async () => {
  let resolve!: (value: Response) => void;
  const old = host(
    vi.fn().mockImplementation(({ operation }) =>
      operation === "snapshot_export"
        ? new Promise<Response>((r) => {
            resolve = r;
          })
        : Promise.resolve(snapshot),
    ),
  );
  const next = host();
  const panel = mount({ ...old, snapshot: true });
  act(() => {
    void panel.current.exportSnapshot();
  });
  panel.replace({ ...next, snapshot: true });
  await act(async () => resolve({ text: "stale", filename: "stale.ndjson" }));
  expect(panel.current.notice).not.toContain("已导出");
  expect(next.request).not.toHaveBeenCalled();
});

test("替换客户端会关闭待处理的恢复确认", async () => {
  const request = vi.fn().mockImplementation(async ({ operation }) => {
    if (operation === "snapshot_restore_preview") return snapshot;
    return {};
  });
  const old = host(request);
  const next = host();
  const panel = mount({ ...old, snapshot: true });
  await act(async () => {
    await panel.current.chooseRestoreSnapshot(new File(["fixture"], "snapshot.ndjson"));
  });
  act(() => {
    void panel.current.restoreSnapshot();
  });
  await screen.findByRole("alertdialog");
  panel.replace({ ...next, snapshot: true });
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(request).not.toHaveBeenCalledWith(
    expect.objectContaining({ operation: "snapshot_restore" }),
  );
});
