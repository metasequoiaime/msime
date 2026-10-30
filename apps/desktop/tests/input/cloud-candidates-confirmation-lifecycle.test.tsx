// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudCandidatesPanel, type CloudDictionaryPanelClient } from "@msime/ui";
import { answerConfirm } from "../support/confirm";

type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
const candidate = {
  code: "synthetic",
  canonical_pinyin: "syn'thetic",
  word: "合成候选",
  weight: 10,
};
const page: Response = { candidates: [candidate], context: "synthetic:context", revision: 7 };
const fixed: Response = {
  positions: [{ context: "synthetic:fixed", code: "syn'thetic", word: "合成候选", position: 2 }],
};
function host(): CloudDictionaryPanelClient {
  return {
    close: vi.fn(),
    request: vi
      .fn()
      .mockImplementation(async ({ operation }) =>
        operation === "candidates" ? page : operation === "fixed_positions" ? fixed : {},
      ),
  };
}
async function query() {
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "synthetic" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  await screen.findByRole("button", { name: "取消固定" });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "调频" }).hasAttribute("disabled")).toBe(false),
  );
}
afterEach(cleanup);

test("面板固定按钮使用当前选定位置与规范编码，确认后刷新原查询", async () => {
  const client = host();
  render(<CloudCandidatesPanel client={client} />);
  await query();
  fireEvent.change(screen.getByRole("combobox", { name: "固定位置" }), {
    target: { value: "4" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "synthetic-draft" },
  });
  fireEvent.click(screen.getByRole("button", { name: "固定" }));
  await screen.findByRole("alertdialog");
  expect(client.request).toHaveBeenCalledTimes(2);
  await answerConfirm("confirm");
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("固定位置已更新"));
  expect(client.request).toHaveBeenNthCalledWith(3, {
    operation: "set_fixed_position",
    context: "synthetic:context",
    code: "syn'thetic",
    word: "合成候选",
    position: 4,
    revision: 7,
  });
  expect(client.request).toHaveBeenNthCalledWith(4, {
    operation: "candidates",
    text: "synthetic",
    kind: "pinyin",
    scheme: "pinyin",
    profile: "xiaohe",
    limit: 100,
  });
});

test.each(["调频", "删除", "固定", "取消固定"])(
  "%s 确认期间替换客户端后不调用旧宿主",
  async (action) => {
    const old = host();
    const next = host();
    const panel = render(<CloudCandidatesPanel client={old} />);
    await query();
    fireEvent.click(screen.getByRole("button", { name: action }));
    await screen.findByRole("alertdialog");
    panel.rerender(<CloudCandidatesPanel client={next} />);
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(old.request).toHaveBeenCalledTimes(2);
    expect(next.request).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "调频" })).toBeNull();
  },
);

test.each(["调频", "删除", "固定", "取消固定"])("取消 %s 确认不发出修改", async (action) => {
  const client = host();
  render(<CloudCandidatesPanel client={client} />);
  await query();
  fireEvent.click(screen.getByRole("button", { name: action }));
  await answerConfirm("cancel");
  expect(client.request).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("button", { name: "调频" })).toBeTruthy();
});

test.each(["调频", "删除", "固定", "取消固定"])(
  "旧 %s 成功后不向失效宿主刷新，也不影响新结果",
  async (action) => {
    let finish!: (value: Response) => void;
    const old = host();
    vi.mocked(old.request).mockImplementation(async ({ operation }) => {
      if (operation === "candidates") return page;
      if (operation === "fixed_positions") return fixed;
      return new Promise<Response>((resolve) => {
        finish = resolve;
      });
    });
    const next = host();
    const panel = render(<CloudCandidatesPanel client={old} />);
    await query();
    fireEvent.click(screen.getByRole("button", { name: action }));
    await answerConfirm("confirm");
    expect(old.request).toHaveBeenCalledTimes(3);
    panel.rerender(<CloudCandidatesPanel client={next} />);
    await query();
    await act(async () => finish({ changed: true }));
    expect(old.request).toHaveBeenCalledTimes(3);
    expect(next.request).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("status").textContent).toBe("云端候选已刷新");
    expect(screen.getByRole("button", { name: "调频" }).hasAttribute("disabled")).toBe(false);
  },
);
