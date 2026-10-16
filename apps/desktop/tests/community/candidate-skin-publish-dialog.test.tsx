// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CandidateSkinPublishDialog, type CandidateSkinPublishDialogProps } from "@msime/ui";

afterEach(cleanup);

test("最小客户端下的空目录对话框可嵌入设置表单并关闭", async () => {
  const props: CandidateSkinPublishDialogProps = {
    client: { packPreview: vi.fn(), addPreview: vi.fn(), addLicense: vi.fn(), publish: vi.fn() },
    onClose: vi.fn(),
    onPublished: vi.fn(),
  };
  const save = vi.fn();
  render(
    <form onSubmit={save}>
      <CandidateSkinPublishDialog {...props} />
    </form>,
  );
  const dialog = screen.getByRole("dialog", { name: "发布候选窗口皮肤" });
  expect(dialog.tagName).toBe("DIV");
  expect(dialog.querySelector("form")).toBeNull();
  await screen.findByText(/还没有可发布的外部皮肤/);
  expect((screen.getByRole("button", { name: "公开发布" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(props.onClose).toHaveBeenCalledOnce();
  expect(save).not.toHaveBeenCalled();
  expect(props.client.packPreview).not.toHaveBeenCalled();
  expect(props.client.publish).not.toHaveBeenCalled();
});
