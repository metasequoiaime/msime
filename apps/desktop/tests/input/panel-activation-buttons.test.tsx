// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { EmojiPanel, HandwritingPanel } from "@msime/ui";
import type { EmojiPanelClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

function handwritingClient() {
  return {
    close: vi.fn().mockResolvedValue(undefined),
    recognizeHandwriting: vi.fn().mockResolvedValue({ candidates: ["合"] }),
    copyHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
    submitHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
  };
}

function handwritingModes() {
  return within(screen.getByRole("group", { name: "点击手写候选的操作" }));
}

test("手写切换模式保存偏好，重新挂载后恢复选中状态", () => {
  const client = handwritingClient();
  const view = render(<HandwritingPanel client={client} />);
  expect(handwritingModes().getByRole("button", { name: "复制", pressed: true })).toBeTruthy();
  fireEvent.click(handwritingModes().getByRole("button", { name: "输入" }));
  expect(localStorage.getItem("msime.handwriting.activation")).toBe("input");
  view.unmount();
  render(<HandwritingPanel client={client} />);
  expect(handwritingModes().getByRole("button", { name: "输入", pressed: true })).toBeTruthy();
  fireEvent.click(handwritingModes().getByRole("button", { name: "复制" }));
  expect(localStorage.getItem("msime.handwriting.activation")).toBe("copy");
});

test("手写缺少复制能力时保留禁用的复制按钮并使用输入模式", () => {
  render(
    <HandwritingPanel client={{ ...handwritingClient(), copyHandwritingCandidate: undefined }} />,
  );
  const copy = handwritingModes().getByRole("button", { name: "复制", pressed: false });
  expect((copy as HTMLButtonElement).disabled).toBe(true);
  expect(handwritingModes().getByRole("button", { name: "输入", pressed: true })).toBeTruthy();
  fireEvent.click(copy);
  expect(localStorage.getItem("msime.handwriting.activation")).toBeNull();
});

test("手写缺少输入能力时隐藏输入按钮并从已存输入偏好回退复制", () => {
  localStorage.setItem("msime.handwriting.activation", "input");
  render(
    <HandwritingPanel client={{ ...handwritingClient(), submitHandwritingCandidate: undefined }} />,
  );
  expect(handwritingModes().queryByRole("button", { name: "输入" })).toBeNull();
  expect(handwritingModes().getByRole("button", { name: "复制", pressed: true })).toBeTruthy();
  expect(localStorage.getItem("msime.handwriting.activation")).toBe("input");
});

test("手写提交期间禁止切换模式，提交完成后恢复", async () => {
  const client = handwritingClient();
  let finish!: () => void;
  client.submitHandwritingCandidate.mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  render(<HandwritingPanel client={client} />);
  fireEvent.click(handwritingModes().getByRole("button", { name: "输入" }));
  const canvas = screen.getByLabelText("手写画布");
  for (const [type, point] of [
    ["pointerdown", 20],
    ["pointerup", 40],
  ] as const) {
    const event = new Event(type, { bubbles: true });
    Object.assign(event, {
      pointerId: 1,
      clientX: point,
      clientY: point,
      button: 0,
      isPrimary: true,
    });
    fireEvent(canvas, event);
  }
  fireEvent.click(await screen.findByRole("button", { name: "合" }));
  expect(client.submitHandwritingCandidate).toHaveBeenCalledExactlyOnceWith("合");
  for (const button of handwritingModes().getAllByRole("button")) {
    expect((button as HTMLButtonElement).disabled).toBe(true);
  }
  fireEvent.click(handwritingModes().getByRole("button", { name: "复制" }));
  expect(localStorage.getItem("msime.handwriting.activation")).toBe("input");
  await act(async () => finish());
  await waitFor(() =>
    expect(
      (handwritingModes().getByRole("button", { name: "复制" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
});

function emojiClient() {
  return {
    close: vi.fn().mockResolvedValue(undefined),
    copyText: vi.fn().mockResolvedValue(undefined),
    sendText: vi.fn().mockResolvedValue(undefined),
    loadCatalog: async () => ({
      emoji: [{ title: "合成目录", icon: "★", items: [{ text: "★", keywords: "合成星" }] }],
      kaomoji: [],
      symbols: [],
    }),
  };
}

test("表情模式切换更新选中提示，并按当前模式复制或输入", async () => {
  const client = emojiClient();
  render(<EmojiPanel client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
  const item = await screen.findByRole("button", { name: "★" });
  const modes = within(screen.getByRole("group", { name: "点击项目时的操作" }));
  fireEvent.click(modes.getByRole("button", { name: "输入到原应用" }));
  expect(modes.getByRole("button", { name: "输入到原应用", pressed: true })).toBeTruthy();
  expect(screen.getByText("点击项目即可输入到原应用")).toBeTruthy();
  await act(async () => fireEvent.click(item));
  expect(client.sendText).toHaveBeenCalledExactlyOnceWith("★");
  expect(client.copyText).not.toHaveBeenCalled();
  fireEvent.click(modes.getByRole("button", { name: "复制" }));
  expect(modes.getByRole("button", { name: "复制", pressed: true })).toBeTruthy();
  expect(screen.getByText("点击项目即可复制")).toBeTruthy();
  await act(async () => fireEvent.click(item));
  expect(client.copyText).toHaveBeenCalledExactlyOnceWith("★");
  expect(localStorage.getItem("msime.handwriting.activation")).toBeNull();
});

test.each(["copyText", "sendText"] as const)(
  "表情缺少 %s 时隐藏模式组并使用剩余能力",
  async (missing) => {
    const client = emojiClient();
    const available: EmojiPanelClient = { ...client, [missing]: undefined };
    render(<EmojiPanel client={available} />);
    fireEvent.click(await screen.findByRole("button", { name: "Emoji" }));
    expect(screen.queryByRole("group", { name: "点击项目时的操作" })).toBeNull();
    await act(async () => fireEvent.click(await screen.findByRole("button", { name: "★" })));
    expect(
      client[missing === "copyText" ? "sendText" : "copyText"],
    ).toHaveBeenCalledExactlyOnceWith("★");
    expect(client[missing]).not.toHaveBeenCalled();
  },
);
