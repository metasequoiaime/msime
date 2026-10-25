// @vitest-environment jsdom
import { accountProviderNames } from "../support/account-provider-names";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useAccountRename } from "../../../../packages/ui/src/account/use-account-rename";
import type { AccountNamePresentation } from "@msime/ui";
import { accountNameResult } from "../support/account-name";

afterEach(cleanup);
const profile = {
  user: {
    id: "synthetic-user",
    displayName: "合成昵称",
    preferredName: "合成昵称",
    createdAt: "2026-01-01T00:00:00Z",
  },
  providers: ["email"],
  providerNames: accountProviderNames,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function setup(namePresentation = vi.fn().mockResolvedValue(accountNameResult("共享结果"))) {
  return {
    client: { namePresentation, rename: vi.fn().mockResolvedValue(profile) },
    name: "  原始输入  ",
    mounted: { current: true },
    clientGeneration: { current: 1 },
    perform: (operation: () => Promise<void>) => operation(),
    onRenamed: vi.fn(),
  };
}

test("使用共享规范化结果与动态上限，保存前重新读取共享校验", async () => {
  const props = setup(
    vi
      .fn()
      .mockResolvedValue(
        accountNameResult("共享结果", { maximum_chars: 12, maximum_utf16_units: 24 }),
      ),
  );
  const { result } = renderHook(() => useAccountRename(props));
  expect(result.current.validName).toBe(false);
  expect(result.current.normalizedName).toBe("  原始输入  ");
  await waitFor(() => expect(result.current.validName).toBe(true));
  expect(result.current.editorState).toEqual({
    maximumChars: 12,
    maximumUtf16Units: 24,
    checking: false,
    unavailable: false,
  });
  props.client.namePresentation.mockResolvedValueOnce(accountNameResult("保存时共享结果"));
  await act(async () => {
    await result.current.rename();
  });
  expect(props.client.rename).toHaveBeenCalledWith("保存时共享结果");
  expect(props.onRenamed).toHaveBeenCalledWith(profile);
});

test("输入变化立即禁用保存，迟到旧输入不能覆盖当前结果", async () => {
  const old = deferred<AccountNamePresentation>();
  const props = setup(
    vi.fn().mockReturnValueOnce(old.promise).mockResolvedValue(accountNameResult("新共享结果")),
  );
  const view = renderHook(({ name }) => useAccountRename({ ...props, name }), {
    initialProps: { name: "旧输入" },
  });
  await waitFor(() => expect(props.client.namePresentation).toHaveBeenCalledWith("旧输入"));
  view.rerender({ name: "新输入" });
  expect(view.result.current.validName).toBe(false);
  await waitFor(() => expect(view.result.current.normalizedName).toBe("新共享结果"));
  await act(async () => {
    old.resolve(accountNameResult("旧共享结果"));
  });
  expect(view.result.current.normalizedName).toBe("新共享结果");
});

test("替换客户端或卸载后不发布旧校验结果", async () => {
  const old = deferred<AccountNamePresentation>();
  const props = setup(vi.fn().mockReturnValue(old.promise));
  const next = setup();
  const view = renderHook(({ client }) => useAccountRename({ ...props, client }), {
    initialProps: { client: props.client },
  });
  await waitFor(() => expect(props.client.namePresentation).toHaveBeenCalled());
  view.rerender({ client: next.client });
  expect(view.result.current.validName).toBe(false);
  await waitFor(() => expect(view.result.current.normalizedName).toBe("共享结果"));
  await act(async () => {
    old.resolve(accountNameResult("旧客户端结果"));
  });
  expect(view.result.current.normalizedName).toBe("共享结果");
  view.unmount();
});

test.each(["reject", "malformed", "invalid"])("共享校验 %s 时不能保存", async (kind) => {
  const method =
    kind === "reject"
      ? vi.fn().mockRejectedValue({ code: "account_unavailable" })
      : vi
          .fn()
          .mockResolvedValue(
            kind === "malformed" ? {} : accountNameResult("无效结果", { valid: false }),
          );
  const props = setup(method);
  const { result } = renderHook(() => useAccountRename(props));
  await waitFor(() => expect(result.current.editorState.checking).toBe(false));
  expect(result.current.validName).toBe(false);
  expect(result.current.editorState.unavailable).toBe(kind !== "invalid");
  await expect(result.current.rename()).rejects.toMatchObject({
    code: kind === "invalid" ? "account_invalid" : "account_unavailable",
  });
  expect(props.client.rename).not.toHaveBeenCalled();
});

test.each(["generation", "unmount"])("保存预检期间 %s 变化不能发改名请求", async (kind) => {
  const pending = deferred<AccountNamePresentation>();
  const props = setup();
  const view = renderHook(() => useAccountRename(props));
  await waitFor(() => expect(view.result.current.validName).toBe(true));
  props.client.namePresentation.mockReturnValueOnce(pending.promise);
  const saving = view.result.current.rename();
  if (kind === "generation") props.clientGeneration.current++;
  else {
    props.mounted.current = false;
    view.unmount();
  }
  await act(async () => {
    pending.resolve(accountNameResult("共享结果"));
    await saving;
  });
  expect(props.client.rename).not.toHaveBeenCalled();
});

test("改名请求返回前替换客户端不能发布旧资料", async () => {
  const pending = deferred<typeof profile>();
  const props = setup();
  props.client.rename.mockReturnValue(pending.promise);
  const { result } = renderHook(() => useAccountRename(props));
  await waitFor(() => expect(result.current.validName).toBe(true));
  const saving = result.current.rename();
  await waitFor(() => expect(props.client.rename).toHaveBeenCalled());
  props.clientGeneration.current++;
  await act(async () => {
    pending.resolve(profile);
    await saving;
  });
  expect(props.onRenamed).not.toHaveBeenCalled();
});

test("新输入校验等待与失败期间保留同客户端的共享长度上限", async () => {
  const props = setup(
    vi
      .fn()
      .mockResolvedValueOnce(
        accountNameResult("共享结果", { maximum_chars: 12, maximum_utf16_units: 24 }),
      )
      .mockRejectedValue({ code: "account_unavailable" }),
  );
  const view = renderHook(({ name }) => useAccountRename({ ...props, name }), {
    initialProps: { name: "旧输入" },
  });
  await waitFor(() => expect(view.result.current.validName).toBe(true));
  view.rerender({ name: "新输入" });
  expect(view.result.current.editorState).toMatchObject({
    maximumChars: 12,
    maximumUtf16Units: 24,
    checking: true,
  });
  expect(view.result.current.validName).toBe(false);
  await waitFor(() => expect(view.result.current.editorState.unavailable).toBe(true));
  expect(view.result.current.editorState).toMatchObject({
    maximumChars: 12,
    maximumUtf16Units: 24,
  });
});
