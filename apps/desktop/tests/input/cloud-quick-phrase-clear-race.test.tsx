// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { useCloudDictionaryPanel, type CloudDictionaryPanelClient } from "@msime/ui";
import { answerConfirm } from "../support/confirm";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

test.each(["确认期间", "预览之后", "无并发"] as const)(
  "清空云端快捷短语保留其他词库的更新：%s",
  async (timing) => {
    const { webcrypto } = await vi.importActual<{ webcrypto: Crypto }>("node:crypto");
    vi.stubGlobal("crypto", webcrypto);
    let revision = 7;
    let words = ["合成旧词"];
    let phrases = ["合成短语"];
    const snapshot = () =>
      [
        JSON.stringify({
          type: "header",
          format: "msime-dictionary-snapshot",
          version: 1,
          revision,
        }),
        ...words.map((word) => JSON.stringify({ type: "entry", data: { kind: "pinyin", word } })),
        ...phrases.map((word) => JSON.stringify({ type: "entry", data: { kind: "quick", word } })),
        JSON.stringify({
          type: "footer",
          records: 1 + words.length + phrases.length,
          sha256: "synthetic",
        }),
        "",
      ].join("\n");
    const updateElsewhere = () => {
      words.push("合成并发新词");
      revision += 1;
    };
    const request = vi
      .fn<CloudDictionaryPanelClient["request"]>()
      .mockImplementation(async (action) => {
        if (action.operation === "snapshot_export") return { text: snapshot() };
        if (action.operation === "snapshot_restore_preview") {
          const expectedRevision = revision;
          if (timing === "预览之后") updateElsewhere();
          return {
            expectedRevision,
            snapshot: {
              cloudRevision: 7,
              sha256: "f".repeat(64),
              bytes: 1,
              records: 2,
              entries: 1,
              overlays: 0,
              positions: 0,
              selections: 0,
            },
          };
        }
        if (action.operation === "snapshot_restore") {
          if (action.revision !== revision)
            throw Object.assign(new Error("conflict"), { code: "conflict" });
          const records = action.text
            .split("\n")
            .filter(Boolean)
            .map((line) => JSON.parse(line));
          words = records
            .filter((row) => row.type === "entry" && row.data.kind === "pinyin")
            .map((row) => row.data.word);
          phrases = records
            .filter((row) => row.type === "entry" && row.data.kind === "quick")
            .map((row) => row.data.word);
          return { saved: true };
        }
        return { entries: [], offset: 0, has_more: false };
      });
    const host: CloudDictionaryPanelClient = { request, close: vi.fn(), snapshot: true };
    let panel!: ReturnType<typeof useCloudDictionaryPanel>;
    function Probe() {
      panel = useCloudDictionaryPanel(host);
      return panel.confirmation;
    }
    render(<Probe />);
    await waitFor(() => expect(panel.busy).toBe(false));
    await act(async () => panel.changeKind("quick"));
    act(() => {
      void panel.clearQuickPhrases();
    });
    await waitFor(() => expect(panel.confirmation).not.toBeNull());
    if (timing === "确认期间") updateElsewhere();
    await answerConfirm("confirm");
    await waitFor(() => expect(panel.busy).toBe(false));
    if (timing === "无并发") {
      expect(words).toEqual(["合成旧词"]);
      expect(phrases).toEqual([]);
      expect(panel.notice).toContain("已清空 1 条");
    } else {
      expect(words).toEqual(["合成旧词", "合成并发新词"]);
      expect(phrases).toEqual(["合成短语"]);
      expect(panel.notice).toContain("云词库已被其他设备更新");
      if (timing === "确认期间") {
        expect(request.mock.calls.some(([action]) => action.operation === "snapshot_restore")).toBe(
          false,
        );
      }
    }
  },
);
