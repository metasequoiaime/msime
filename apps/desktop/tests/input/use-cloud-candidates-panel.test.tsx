// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import {
  useCloudCandidatesPanel,
  type CloudCandidatesPanelState,
  type CloudDictionaryPanelClient,
} from "@msime/ui";
import { answerConfirm } from "../support/confirm";

type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
const candidate = {
  code: "synthetic",
  canonical_pinyin: "syn'thetic",
  word: "合成候选",
  weight: 10,
};
const item = { context: "synthetic:fixed", code: "syn'thetic", word: "合成候选", position: 2 };
const page: Response = { candidates: [candidate], context: "synthetic:context", revision: 7 };
function host(request = vi.fn().mockResolvedValue(page)): CloudDictionaryPanelClient {
  return { request, close: vi.fn() };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function mount(client: CloudDictionaryPanelClient) {
  let state!: CloudCandidatesPanelState;
  function Probe({ client }: { client: CloudDictionaryPanelClient }) {
    state = useCloudCandidatesPanel(client);
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
    unmount: view.unmount,
  };
}
async function query(panel: ReturnType<typeof mount>) {
  act(() => panel.current.setText(" synthetic "));
  await act(async () => {
    await panel.current.queryCandidates();
  });
}
afterEach(cleanup);

test("StrictMode 不自动查询，提交时读取最新编码、简拼与双拼方案", async () => {
  const client = host(vi.fn().mockResolvedValue({ candidates: [candidate], revision: 7 }));
  const panel = mount(client);
  expect(client.request).not.toHaveBeenCalled();
  await act(async () => {
    await panel.current.queryCandidates();
  });
  expect(panel.current.notice).toBe("请输入编码");
  act(() => {
    panel.current.setText(" synthetic ");
    panel.current.setJianpin(true);
    panel.current.setScheme("shuangpin");
    panel.current.setProfile("ziranma");
  });
  await act(async () => {
    await panel.current.queryCandidates();
  });
  expect(client.request).toHaveBeenCalledExactlyOnceWith({
    operation: "candidates",
    text: "synthetic",
    kind: "jianpin",
    scheme: "shuangpin",
    profile: "ziranma",
    limit: 100,
  });
  expect(panel.current.candidates).toEqual([candidate]);
  expect(panel.current.revision).toBe(7);
  expect(panel.current.notice).toBe("云端候选已刷新");
});

test("先显示候选再等待固定位置，忙碌期间防重复且不切换类型", async () => {
  const pending = deferred<Response>();
  const request = vi.fn().mockResolvedValueOnce(page).mockReturnValueOnce(pending.promise);
  const panel = mount(host(request));
  act(() => panel.current.setText("synthetic"));
  act(() => {
    void panel.current.queryCandidates();
  });
  await waitFor(() => expect(panel.current.candidates).toEqual([candidate]));
  expect(panel.current.busy).toBe(true);
  expect(panel.current.query?.text).toBe("synthetic");
  expect(panel.current.positions).toEqual([]);
  act(() => {
    void panel.current.queryCandidates();
    panel.current.changeKind("quick");
  });
  expect(request).toHaveBeenCalledTimes(2);
  expect(panel.current.kind).toBe("pinyin");
  expect(request).toHaveBeenLastCalledWith({
    operation: "fixed_positions",
    context: "synthetic:context",
    offset: 0,
  });
  await act(async () => pending.resolve({ positions: [item] }));
  expect(panel.current.positions).toEqual([item]);
  expect(panel.current.busy).toBe(false);
});

test("固定位置查询失败保留已到达候选，下一次主查询失败清理全部结果", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce(page)
    .mockRejectedValue({ code: "account_network", detail: "synthetic private payload" });
  const panel = mount(host(request));
  await query(panel);
  expect(panel.current.candidates).toEqual([candidate]);
  expect(panel.current.query?.text).toBe("synthetic");
  expect(panel.current.notice).toBe("网络连接失败，请检查网络后重试。");
  await act(async () => {
    await panel.current.queryCandidates();
  });
  expect(panel.current.candidates).toEqual([]);
  expect(panel.current.positions).toEqual([]);
  expect(panel.current.query).toBeNull();
  expect(panel.current.context).toBe("");
  expect(panel.current.revision).toBe(0);
});

test.each([true, false])("调频使用已确认查询、规范拼音和参数，changed=%s", async (changed) => {
  const request = vi
    .fn()
    .mockImplementation(async ({ operation }) =>
      operation === "candidates"
        ? page
        : operation === "rank"
          ? { changed, selection_count: 3 }
          : { positions: [item] },
    );
  const panel = mount(host(request));
  await query(panel);
  act(() => {
    panel.current.setText("synthetic-draft");
    panel.current.setScheme("shuangpin");
    panel.current.setMode("linear");
    panel.current.setStep(9);
    panel.current.setTrigger(4);
    panel.current.setForceTop(true);
  });
  act(() => {
    void panel.current.rank(candidate);
  });
  await answerConfirm("confirm");
  await waitFor(() => expect(panel.current.busy).toBe(false));
  expect(request).toHaveBeenNthCalledWith(3, {
    operation: "rank",
    text: "synthetic",
    kind: "pinyin",
    scheme: "pinyin",
    profile: "xiaohe",
    limit: 100,
    code: "syn'thetic",
    word: "合成候选",
    revision: 7,
    mode: "linear",
    linear_step: 9,
    trigger_count: 4,
    force_top: true,
  });
  expect(request).toHaveBeenNthCalledWith(4, { operation: "candidates", ...panel.current.query });
  expect(panel.current.notice).toBe(
    changed ? "云端排序已更新" : "已记录本次选择，本次未调整排序；累计 3 次",
  );
});

test("删除限制按字符计数，英文单字符允许删除且刷新失败保留修改成功提示", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce(page)
    .mockResolvedValueOnce({ positions: [] })
    .mockResolvedValueOnce(page)
    .mockResolvedValueOnce({ positions: [] })
    .mockResolvedValueOnce({})
    .mockRejectedValueOnce(new Error("synthetic failure"));
  const panel = mount(host(request));
  await query(panel);
  await act(async () => {
    await panel.current.remove({ code: "synthetic", word: "𠀀", weight: 1 });
  });
  expect(panel.current.confirmation).toBeNull();
  expect(request).toHaveBeenCalledTimes(2);
  act(() => panel.current.changeKind("english"));
  await act(async () => {
    await panel.current.queryCandidates();
  });
  act(() => {
    void panel.current.remove({ code: "synthetic", word: "x", weight: 1 });
  });
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(panel.current.notice).toBe("云端候选已删除，但候选刷新失败，请重新查询"),
  );
  expect(request).toHaveBeenNthCalledWith(5, {
    operation: "remove_candidate",
    text: "synthetic",
    kind: "english",
    scheme: "pinyin",
    profile: "xiaohe",
    limit: 100,
    code: "synthetic",
    word: "x",
    revision: 7,
  });
  expect(panel.current.candidates).toEqual([]);
  expect(panel.current.query).toBeNull();
});

test("固定候选使用查询 context，取消固定使用固定记录 context", async () => {
  const request = vi
    .fn()
    .mockImplementation(async ({ operation }) =>
      operation === "candidates" ? page : { positions: [item] },
    );
  const panel = mount(host(request));
  await query(panel);
  act(() => panel.current.setPosition(4));
  act(() => {
    void panel.current.setFixed(candidate, panel.current.position);
  });
  await answerConfirm("confirm");
  await waitFor(() => expect(panel.current.notice).toBe("固定位置已更新"));
  expect(request).toHaveBeenNthCalledWith(3, {
    operation: "set_fixed_position",
    context: "synthetic:context",
    code: "syn'thetic",
    word: "合成候选",
    position: 4,
    revision: 7,
  });
  act(() => {
    void panel.current.unfix(item);
  });
  await answerConfirm("confirm");
  await waitFor(() => expect(panel.current.notice).toBe("固定位置已取消"));
  expect(request).toHaveBeenNthCalledWith(6, {
    operation: "set_fixed_position",
    context: "synthetic:fixed",
    code: "syn'thetic",
    word: "合成候选",
    position: null,
    revision: 7,
  });
});

test("类型切换清理结果并关闭简拼，快捷短语不请求固定位置", async () => {
  const client = host();
  const panel = mount(client);
  await query(panel);
  act(() => panel.current.setJianpin(true));
  act(() => panel.current.changeKind("quick"));
  expect(panel.current.jianpin).toBe(false);
  expect(panel.current.candidates).toEqual([]);
  expect(panel.current.positions).toEqual([]);
  expect(panel.current.query).toBeNull();
  expect(panel.current.context).toBe("");
  expect(client.request).toHaveBeenCalledTimes(2);
  await act(async () => {
    await panel.current.queryCandidates();
  });
  expect(client.request).toHaveBeenCalledTimes(3);
  expect(client.request).toHaveBeenLastCalledWith({
    operation: "candidates",
    text: "synthetic",
    kind: "quick",
    scheme: "pinyin",
    profile: "xiaohe",
    limit: 100,
  });
});

test.each(["resolve", "reject"] as const)(
  "旧固定位置请求的 %s 不影响新查询和忙碌状态",
  async (outcome) => {
    const old = deferred<Response>();
    const pending = deferred<Response>();
    const first = host(vi.fn().mockResolvedValueOnce(page).mockReturnValueOnce(old.promise));
    const next = host(
      vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue({ positions: [] }),
    );
    const panel = mount(first);
    act(() => panel.current.setText("synthetic"));
    act(() => {
      void panel.current.queryCandidates();
    });
    await waitFor(() => expect(first.request).toHaveBeenCalledTimes(2));
    panel.replace(next);
    act(() => {
      void panel.current.queryCandidates();
    });
    await act(async () => {
      if (outcome === "resolve") old.resolve({ positions: [item] });
      else old.reject(new Error("synthetic failure"));
    });
    expect(panel.current.positions).toEqual([]);
    expect(panel.current.candidates).toEqual([]);
    expect(panel.current.busy).toBe(true);
    await act(async () => pending.resolve({ ...page, revision: 9 }));
    expect(panel.current.revision).toBe(9);
    expect(panel.current.candidates).toEqual([candidate]);
    expect(panel.current.busy).toBe(false);
    expect(panel.current.notice).toBe("云端候选已刷新");
  },
);
