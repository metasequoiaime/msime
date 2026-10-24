// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import type { FormEvent } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  DictionaryNameDialog,
  DictionaryWordDialog,
} from "../../../../packages/ui/src/dictionary/dictionary-edit-dialogs";

afterEach(cleanup);

test("新建词库对话框校验名称，更新回调时保留草稿，并能嵌入设置表单", async () => {
  const first = vi.fn(async (_name: string) => {});
  const second = vi.fn(async (_name: string) => {});
  const outerSubmit = vi.fn((event: FormEvent) => event.preventDefault());
  const close = vi.fn();
  const form = (save: (name: string) => Promise<void>) => (
    <form onSubmit={outerSubmit}>
      <DictionaryNameDialog onClose={close} onSubmit={save} />
    </form>
  );
  const { container, rerender } = render(form(first));
  expect(container.querySelectorAll("form")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "创建" }));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", "词库名需要 1–32 个字。");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "创建" })).toHaveProperty("disabled", false),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "词库名称" }), {
    target: { value: "  合成词库  " },
  });
  expect(screen.queryByRole("alert")).toBeNull();
  rerender(form(second));
  const field = screen.getByRole("textbox", { name: "词库名称" });
  expect(field).toHaveProperty("value", "  合成词库  ");
  expect(fireEvent.keyDown(field, { key: "Enter" })).toBe(false);
  await waitFor(() => expect(second).toHaveBeenCalledWith("合成词库"));
  expect(first).not.toHaveBeenCalled();
  expect(outerSubmit).not.toHaveBeenCalled();
  expect(close).not.toHaveBeenCalled();
});

test("添加词条对话框校验字段并向宿主回调传递规范化词语和拼音", async () => {
  const save = vi.fn(async (_word: string, _code: string) => {});
  render(<DictionaryWordDialog onClose={() => {}} onSubmit={save} />);
  const add = screen.getByRole("button", { name: "添加" });
  fireEvent.click(add);
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", "请填写词语。");
  await waitFor(() => expect(add).toHaveProperty("disabled", false));
  fireEvent.change(screen.getByRole("textbox", { name: "词语" }), {
    target: { value: "  合成  " },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "拼音" }), { target: { value: "123" } });
  fireEvent.click(add);
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "拼音只能是字母，音节之间可以用空格或 ' 分开。",
  );
  expect(save).not.toHaveBeenCalled();
  await waitFor(() => expect(add).toHaveProperty("disabled", false));
  fireEvent.change(screen.getByRole("textbox", { name: "拼音" }), {
    target: { value: " HE’CHENG " },
  });
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(add);
  await waitFor(() => expect(save).toHaveBeenCalledWith("合成", "he'cheng"));
});

test.each(["新建词库", "添加词条"])(
  "%s 提交期间阻止重复保存和关闭，结束后恢复交互",
  async (title) => {
    let finish!: () => void;
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const save = vi.fn(() => pending);
    const close = vi.fn();
    const Component = title === "新建词库" ? DictionaryNameDialog : DictionaryWordDialog;
    render(<Component onClose={close} onSubmit={save} />);
    const field = screen.getByRole("textbox", { name: title === "新建词库" ? "词库名称" : "词语" });
    fireEvent.change(field, { target: { value: "合成" } });
    if (title === "添加词条")
      fireEvent.change(screen.getByRole("textbox", { name: "拼音" }), {
        target: { value: "he cheng" },
      });
    const submit = screen.getByRole("button", { name: title === "新建词库" ? "创建" : "添加" });
    fireEvent.click(submit);
    expect(save).toHaveBeenCalledOnce();
    expect(submit).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "取消" })).toHaveProperty("disabled", true);
    fireEvent.keyDown(field, { key: "Enter" });
    fireEvent.keyDown(field, { key: "Escape" });
    fireEvent.mouseDown(screen.getByRole("dialog").parentElement!);
    expect(save).toHaveBeenCalledOnce();
    expect(close).not.toHaveBeenCalled();
    await act(async () => {
      finish();
      await pending;
    });
    expect(submit).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "取消" })).toHaveProperty("disabled", false);
    fireEvent.keyDown(field, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  },
);
