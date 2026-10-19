// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HandwritingModelNotice, type ResourcePacks } from "@msime/ui";

afterEach(cleanup);

function packs(overrides: Partial<ResourcePacks> = {}): ResourcePacks {
  return {
    statuses: [
      {
        id: "handwriting",
        state: "missing",
        size: 26_861_246,
        schemes: [],
      },
    ],
    progress: {},
    errors: {},
    ensure: vi.fn(),
    install: vi.fn(),
    cancel: vi.fn(),
    ...overrides,
  };
}

test("缺少手写模型时提供下载入口", () => {
  const resourcePacks = packs();
  render(<HandwritingModelNotice packs={resourcePacks} />);

  expect(screen.getByText(/手写识别需要先下载手写模型/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "下载" }));
  expect(resourcePacks.install).toHaveBeenCalledWith("handwriting");
});

test("下载中显示进度并可取消", () => {
  const resourcePacks = packs({
    progress: {
      handwriting: { id: "handwriting", stage: "download", downloaded: 10, total: 20 },
    },
  });
  render(<HandwritingModelNotice packs={resourcePacks} />);

  expect(screen.getByText(/正在下载手写模型.*50%/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(resourcePacks.cancel).toHaveBeenCalledWith("handwriting");
});

test("失败时可编辑镜像并保存后重试", async () => {
  const resourcePacks = packs({ errors: { handwriting: "下载失败" } });
  const mirror = {
    load: vi.fn(async () => "https://old.example.com"),
    save: vi.fn(async () => undefined),
  };
  render(<HandwritingModelNotice packs={resourcePacks} mirror={mirror} />);

  fireEvent.click(screen.getByRole("button", { name: "设置下载镜像" }));
  const input = await screen.findByRole("textbox", { name: "模型下载镜像" });
  fireEvent.change(input, { target: { value: "https://mirror.example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "保存并重试" }));

  await waitFor(() => expect(mirror.save).toHaveBeenCalledWith("https://mirror.example.com"));
  expect(resourcePacks.install).toHaveBeenCalledWith("handwriting");
});
