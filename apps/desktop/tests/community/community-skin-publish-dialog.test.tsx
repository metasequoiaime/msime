// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunitySkinPublishDialog, type CommunitySkinPublishDialogProps } from "@msime/ui";
import { defaultTouchKeyboardSkinDesign as design } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import { harmonyTouchKeyboardRows } from "../../../../packages/ui/src/keyboard/keyboard-layouts";

afterEach(cleanup);

test("从包入口独立渲染发布表单，只需加载与发布能力，阻止表单默认提交", async () => {
  const saved = [
    { id: "synthetic-first", name: "合成森林", design },
    { id: "synthetic-second", name: "合成湖泊", design: { ...design, background: 0x123456 } },
  ];
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const props: CommunitySkinPublishDialogProps = {
    client: { publish: vi.fn(() => pending) },
    library: { load: vi.fn(async () => saved) },
    phonePreview: true,
    touchRows: harmonyTouchKeyboardRows.fourteen_key,
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunitySkinPublishDialog {...props} />);
  await screen.findByRole("option", { name: "合成湖泊" });
  expect(document.querySelector("[data-keyboard-toolbar]")).not.toBeNull();
  expect(document.querySelector("svg[data-preview-skin]")?.getAttribute("viewBox")).toMatch(
    /^0 0 390 /,
  );
  expect(document.querySelector("[data-keyboard-key='QW']")).not.toBeNull();
  fireEvent.change(screen.getByRole("combobox", { name: "发布设计" }), {
    target: { value: saved[1].id },
  });
  expect((screen.getByRole("textbox", { name: "发布皮肤名称" }) as HTMLInputElement).value).toBe(
    saved[1].name,
  );
  fireEvent.change(screen.getByRole("combobox", { name: "发布分类" }), {
    target: { value: "nature" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布素材权利" }));
  const event = new Event("submit", { bubbles: true, cancelable: true });
  fireEvent(screen.getByRole("dialog", { name: "发布我的皮肤" }), event);
  expect(event.defaultPrevented).toBe(true);
  expect(props.client.publish).toHaveBeenCalledWith(
    expect.any(String),
    saved[1].name,
    "",
    saved[1].design,
    "nature",
  );
  expect((screen.getByRole("button", { name: "取消" }) as HTMLButtonElement).disabled).toBe(true);
  expect(props.onPublished).not.toHaveBeenCalled();
  await act(async () => finish());
  await waitFor(() => expect(props.onPublished).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(props.onClose).toHaveBeenCalledOnce();
});

test("独立对话框空库提示和关闭入口保持可用", async () => {
  const onClose = vi.fn();
  const publish = vi.fn();
  render(
    <CommunitySkinPublishDialog
      client={{ publish }}
      library={{ load: async () => [] }}
      onClose={onClose}
      onPublished={async () => {}}
    />,
  );
  await screen.findByText("还没有命名保存的皮肤，请先在“设计我的皮肤”中保存一款。");
  expect((screen.getByRole("button", { name: "公开发布" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  fireEvent.click(screen.getByRole("button", { name: "关闭发布窗口" }));
  expect(onClose).toHaveBeenCalledOnce();
  expect(publish).not.toHaveBeenCalled();
});
