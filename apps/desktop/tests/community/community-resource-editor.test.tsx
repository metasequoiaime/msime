// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunityResourceEditor, type CommunityResourceEditorProps } from "@msime/ui";

afterEach(cleanup);

test("公共资源编辑器用最小客户端独立发布与关闭", async () => {
  const props: CommunityResourceEditorProps = {
    client: { publish: vi.fn(async () => {}) },
    kind: "reply",
    onPublished: vi.fn(async () => {}),
    onClose: vi.fn(),
  };
  render(<CommunityResourceEditor {...props} />);
  fireEvent.change(screen.getByRole("textbox", { name: "社区作品名称" }), {
    target: { value: "合成回复" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "社区回复提示词" }), {
    target: { value: "合成提示" },
  });
  fireEvent.click(screen.getByRole("checkbox", { name: "确认拥有发布内容权利" }));
  fireEvent.click(screen.getByRole("button", { name: "公开发布" }));
  await waitFor(() => expect(props.onPublished).toHaveBeenCalledOnce());
  expect(props.client.publish).toHaveBeenCalledWith(
    expect.any(String),
    "reply",
    "合成回复",
    "",
    { prompt: "合成提示" },
    0,
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(props.onClose).toHaveBeenCalledOnce();
});
