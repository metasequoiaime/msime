// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudDictionaryFilesPanel } from "@msime/ui";
import { createDesktopCloudDictionaryClient } from "../../src/dictionary/desktop-cloud-dictionary-client";
import { answerConfirm } from "../support/confirm";

const firstText = "he\t合成旧词\n";

function selectFile(file: File) {
  fireEvent.change(screen.getByLabelText("选择文件"), { target: { files: [file] } });
}

function mount(platform: "linux" | "windows" = "linux") {
  const request = vi.fn().mockResolvedValue({});
  const client = createDesktopCloudDictionaryClient({ close: vi.fn(), request }, platform, {});
  render(<CloudDictionaryFilesPanel client={client} />);
  return request;
}

afterEach(cleanup);

test.each(["unreadable", "nul", "decoded-empty"] as const)(
  "新词库文件 %s 后不能继续上传旧文件",
  async (failure) => {
    const request = mount();
    selectFile(new File([firstText], "first.tsv"));
    await screen.findByText("first.tsv");
    const next = new File(
      failure === "decoded-empty"
        ? [new Uint8Array([0xef, 0xbb, 0xbf])]
        : [failure === "nul" ? "next\u0000text" : "next text"],
      "next.tsv",
    );
    if (failure === "unreadable")
      vi.spyOn(next, "arrayBuffer").mockRejectedValue(new Error("synthetic read failure"));
    selectFile(next);
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe(
        "无法读取文件，请确认是大小不超过 64 KiB 的文本词库",
      ),
    );
    // 旧按钮如果仍可达，就完成真实确认，验证它不能把上一个文件上传。
    const staleUpload = screen.queryByRole("button", { name: "确认上传到云端" });
    if (staleUpload) {
      fireEvent.click(staleUpload);
      await answerConfirm("confirm");
    }
    expect(request).not.toHaveBeenCalled();
    expect(staleUpload).toBeNull();
    expect(screen.queryByText("first.tsv")).toBeNull();
  },
);

test.each(["empty", "oversized"] as const)("新词库文件 %s 时保持原有大小限制", async (failure) => {
  const request = mount();
  selectFile(new File([firstText], "first.tsv"));
  await screen.findByText("first.tsv");
  const next = new File(failure === "empty" ? [] : [new Uint8Array(65537)], "next.tsv");
  selectFile(next);
  expect(screen.getByRole("status").textContent).toBe("导入文件必须大于 0 且不超过 64 KiB");
  expect(screen.queryByRole("button", { name: "确认上传到云端" })).toBeNull();
  expect(request).not.toHaveBeenCalled();
});

test("新文件读取期间撤下旧预览，完成后只上传新文件", async () => {
  const request = mount("windows");
  selectFile(new File([firstText], "first.tsv"));
  await screen.findByText("first.tsv");
  const nextText = "xin\t合成新词\n";
  const next = new File([nextText], "next.tsv");
  let complete!: (bytes: ArrayBuffer) => void;
  vi.spyOn(next, "arrayBuffer").mockImplementation(
    () => new Promise<ArrayBuffer>((resolve) => (complete = resolve)),
  );
  selectFile(next);
  expect(screen.queryByText("first.tsv")).toBeNull();
  expect(screen.queryByRole("button", { name: "确认上传到云端" })).toBeNull();
  await act(async () => complete(new TextEncoder().encode(nextText).buffer));
  await screen.findByText("next.tsv");
  fireEvent.click(screen.getByRole("button", { name: "确认上传到云端" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledExactlyOnceWith({
      operation: "import",
      kind: "pinyin",
      format: "standard",
      text: nextText,
    }),
  );
});
