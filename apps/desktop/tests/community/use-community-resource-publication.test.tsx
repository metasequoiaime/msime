// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunityResourcePublication } from "../../../../packages/ui/src/community/use-community-resource-publication";
import type {
  CommunityResource,
  CommunityResourceClient,
} from "../../../../packages/ui/src/community/community-resource-model";

afterEach(cleanup);

function options() {
  return {
    client: { publish: vi.fn<CommunityResourceClient["publish"]>(async () => {}) },
    kind: "reply" as const,
    onPublished: vi.fn(async () => {}),
  };
}
function existing(): CommunityResource {
  return {
    id: "10000000-0000-4000-8000-000000000001",
    kind: "dictionary",
    name: "合成词库",
    description: "合成说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", code: "he", word: "合成词", weight: 100 }] },
    revision: 7,
    saves: 0,
    saved: false,
    owned: true,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}
function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("独立发布只需最小客户端，授权后规范化名称说明并保留提示词", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourcePublication(props));
  act(() => {
    result.current.onNameChange("  合成回复  ");
    result.current.onDescriptionChange("  合成说明  ");
    result.current.onPromptChange("  合成提示  ");
  });
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  expect(result.current.error).toContain("权利");
  act(() => result.current.onAgreedChange(true));
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledWith(
    expect.stringMatching(/^[0-9a-f-]{36}$/),
    "reply",
    "合成回复",
    "合成说明",
    { prompt: "  合成提示  " },
    0,
  );
  expect(props.onPublished).toHaveBeenCalledOnce();
});

test("现有词库沿用 ID 与版本且无需再次授权，批次删除后添加保留新词", async () => {
  const props = { ...options(), kind: "dictionary" as const, existing: existing() };
  const { result } = renderHook(() => useCommunityResourcePublication(props));
  expect(result.current.name).toBe("合成词库");
  expect(result.current.description).toBe("合成说明");
  act(() => {
    result.current.onCodeChange("  xin  ");
    result.current.onWordChange(" 新词 ");
    result.current.onWeightChange("0");
    result.current.onEntryKindChange("quick");
  });
  act(() => {
    result.current.removeEntry(0);
    result.current.addEntry();
  });
  const entries = [{ kind: "quick", code: "xin", word: " 新词 ", weight: 0 }];
  expect(result.current.entries).toEqual(entries);
  expect(result.current.code).toBe("");
  expect(result.current.word).toBe("");
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledWith(
    props.existing.id,
    "dictionary",
    "合成词库",
    "合成说明",
    { entries },
    7,
  );
});

test.each(["", "-1", "1.5", "9007199254740992"])("空词条或非法权重 %s 不加入词库", (weight) => {
  const { result } = renderHook(() =>
    useCommunityResourcePublication({ ...options(), kind: "dictionary" }),
  );
  act(() => {
    result.current.onWeightChange(weight);
  });
  if (weight !== "")
    act(() => {
      result.current.onCodeChange("ci");
      result.current.onWordChange("合成词");
    });
  act(() => result.current.addEntry());
  expect(result.current.entries).toEqual([]);
  expect(result.current.error).toContain("非负整数");
});

test("词条不得重复或超过 128 条，空词库无法提交", async () => {
  const props = { ...options(), kind: "dictionary" as const };
  const { result, unmount } = renderHook(() => useCommunityResourcePublication(props));
  act(() => {
    result.current.onNameChange("合成词库");
    result.current.onAgreedChange(true);
  });
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  act(() => {
    result.current.onCodeChange("ci");
    result.current.onWordChange("合成词");
  });
  act(() => result.current.addEntry());
  act(() => {
    result.current.onCodeChange(" ci ");
    result.current.onWordChange("合成词");
  });
  act(() => result.current.addEntry());
  expect(result.current.entries).toHaveLength(1);
  expect(result.current.error).toContain("不能重复");
  unmount();
  const item = existing();
  item.content.entries = Array.from({ length: 128 }, (_, i) => ({
    kind: "pinyin",
    code: `ci${i}`,
    word: `合成词${i}`,
    weight: 1,
  }));
  const full = renderHook(() => useCommunityResourcePublication({ ...props, existing: item }));
  act(() => {
    full.result.current.onCodeChange("new");
    full.result.current.onWordChange("新合成词");
  });
  act(() => full.result.current.addEntry());
  expect(full.result.current.entries).toHaveLength(128);
  expect(full.result.current.error).toContain("128");
});

test("提示词按 Unicode 码点限制 2000 字，拒绝空白并保留原始内容", async () => {
  const props = options();
  const { result } = renderHook(() => useCommunityResourcePublication(props));
  act(() => {
    result.current.onNameChange("合成回复");
    result.current.onAgreedChange(true);
    result.current.onPromptChange(" ");
  });
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  act(() => result.current.onPromptChange("𠮷".repeat(2001)));
  await act(async () => result.current.submit());
  expect(props.client.publish).not.toHaveBeenCalled();
  act(() => result.current.onPromptChange("𠮷".repeat(2000)));
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledOnce();
});

test("同步防重，失败重试沿用 ID，发布成功后才运行回调", async () => {
  const pending = deferred();
  const props = options();
  props.client.publish.mockImplementationOnce(() => pending.promise);
  const { result } = renderHook(() => useCommunityResourcePublication(props));
  act(() => {
    result.current.onNameChange("合成回复");
    result.current.onPromptChange("合成提示");
    result.current.onAgreedChange(true);
  });
  let first!: Promise<void>;
  act(() => {
    first = result.current.submit();
    void result.current.submit();
  });
  expect(result.current.busy).toBe(true);
  expect(props.client.publish).toHaveBeenCalledOnce();
  expect(props.onPublished).not.toHaveBeenCalled();
  await act(async () => {
    pending.reject(new Error("合成失败"));
    await first;
  });
  expect(result.current.busy).toBe(false);
  expect(result.current.error).toBeTruthy();
  await act(async () => result.current.submit());
  expect(props.client.publish).toHaveBeenCalledTimes(2);
  expect(props.client.publish.mock.calls[0][0]).toBe(props.client.publish.mock.calls[1][0]);
  expect(props.onPublished).toHaveBeenCalledOnce();
});

test.each(["success", "failure"])(
  "客户端替换后旧 %s 不覆盖新提交状态或执行旧回调",
  async (outcome) => {
    const stale = deferred();
    const current = deferred();
    const first = options();
    const next = options();
    first.client.publish.mockImplementation(() => stale.promise);
    next.client.publish.mockImplementation(() => current.promise);
    const { result, rerender } = renderHook((props) => useCommunityResourcePublication(props), {
      initialProps: first,
      wrapper: StrictMode,
    });
    act(() => {
      result.current.onNameChange("合成回复");
      result.current.onPromptChange("合成提示");
      result.current.onAgreedChange(true);
    });
    let oldSubmission!: Promise<void>;
    act(() => {
      oldSubmission = result.current.submit();
    });
    rerender(next);
    expect(result.current.busy).toBe(false);
    let submission!: Promise<void>;
    act(() => {
      submission = result.current.submit();
    });
    await act(async () => {
      if (outcome === "success") stale.resolve();
      else stale.reject(new Error("过期合成失败"));
      await oldSubmission;
    });
    expect(first.onPublished).not.toHaveBeenCalled();
    expect(result.current.busy).toBe(true);
    expect(result.current.error).toBe("");
    await act(async () => {
      current.resolve();
      await submission;
    });
    expect(next.onPublished).toHaveBeenCalledOnce();
    expect(result.current.busy).toBe(false);
  },
);

test("卸载后完成发布不运行回调", async () => {
  const pending = deferred();
  const props = options();
  props.client.publish.mockImplementation(() => pending.promise);
  const { result, unmount } = renderHook(() => useCommunityResourcePublication(props), {
    wrapper: StrictMode,
  });
  act(() => {
    result.current.onNameChange("合成回复");
    result.current.onPromptChange("合成提示");
    result.current.onAgreedChange(true);
  });
  let submission!: Promise<void>;
  act(() => {
    submission = result.current.submit();
  });
  unmount();
  await act(async () => {
    pending.resolve();
    await submission;
  });
  expect(props.onPublished).not.toHaveBeenCalled();
});
