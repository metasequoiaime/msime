// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AiSkinGeneration } from "../../../../packages/ui/src/keyboard/ai-skin-generation";
import {
  defaultTouchKeyboardSkinDesign,
  type AiSkinClient,
  type AiSkinProposal,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
} from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";
import type { CommunitySkinClient } from "../../../../packages/ui/src/community/community-skin-model";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function proposal(description = "合成说明 1"): AiSkinProposal {
  return {
    name: "合成同名设计",
    description,
    artworkPrompt: "合成插画",
    design: { ...defaultTouchKeyboardSkinDesign },
    artwork: { b64_json: "YQ==", mime_type: "image/png", width: 1, height: 1 },
  };
}

function library(): CustomSkinLibraryClient {
  let items: SavedTouchKeyboardSkin[] = [];
  return {
    load: vi.fn(async () => items),
    mutate: vi.fn(async (action) => {
      if (action.operation === "create")
        items = [
          ...items,
          { id: `synthetic-${items.length}`, name: action.name, design: action.design },
        ];
      return items;
    }),
  };
}

function view(
  client: AiSkinClient,
  skins: CustomSkinLibraryClient,
  communitySkins?: CommunitySkinClient,
) {
  return (
    <AiSkinGeneration
      client={client}
      library={skins}
      communitySkins={communitySkins}
      onUse={vi.fn()}
      onClose={vi.fn()}
    />
  );
}

test("更换生成服务后恢复抽卡且不接收旧服务结果", async () => {
  const pending = deferred<AiSkinProposal[]>();
  const old = {
    generate: vi.fn((_id: string, _prompt: string) => pending.promise),
    cancel: vi.fn(async () => undefined),
  };
  const current = { generate: vi.fn(async () => []), cancel: vi.fn(async () => undefined) };
  const skins = library();
  const mounted = render(view(old, skins));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  const id = old.generate.mock.calls[0][0];
  mounted.rerender(view(current, skins));
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "抽三张皮肤" }).disabled).toBe(
    false,
  );
  expect(old.cancel).toHaveBeenCalledOnce();
  expect(old.cancel).toHaveBeenCalledWith(id);
  await act(async () => pending.resolve([proposal()]));
  expect(screen.queryByRole("heading", { name: "合成同名设计" })).toBeNull();
  expect(screen.queryByRole("status")).toBeNull();
});

test("重抽的同名新设计可以独立保存", async () => {
  const first = proposal();
  const second = { ...proposal("合成说明 2"), design: { ...first.design, accent: 0x445566 } };
  const client = {
    generate: vi.fn().mockResolvedValueOnce([first]).mockResolvedValueOnce([second]),
    cancel: vi.fn(async () => undefined),
  };
  const skins = library();
  render(view(client, skins));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  await screen.findByText("合成说明 1");
  fireEvent.click(screen.getByRole("button", { name: "保存到我的皮肤" }));
  await screen.findByRole("button", { name: "已保存" });
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  await screen.findByText("合成说明 2");
  expect(screen.queryByRole("button", { name: "已保存" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "保存到我的皮肤" }));
  await screen.findByRole("button", { name: "已保存" });
  expect(skins.mutate).toHaveBeenCalledTimes(2);
  expect(skins.mutate).toHaveBeenLastCalledWith(
    expect.objectContaining({
      name: "合成同名设计 2",
      design: expect.objectContaining({ accent: 0x445566 }),
    }),
  );
});

test.each(["load", "mutate"])("更换图库后隔离迟到的 %s，不打开旧提案发布表单", async (boundary) => {
  const item = proposal();
  const pending = deferred<SavedTouchKeyboardSkin[]>();
  const old = library();
  if (boundary === "load") old.load = vi.fn(() => pending.promise);
  else old.mutate = vi.fn(() => pending.promise);
  const client = { generate: vi.fn(async () => [item]), cancel: vi.fn(async () => undefined) };
  const communitySkins = {
    publish: vi.fn(async () => undefined),
  } as unknown as CommunitySkinClient;
  const mounted = render(view(client, old, communitySkins));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  await screen.findByText("合成说明 1");
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "发布到社区" }));
  });
  mounted.rerender(view(client, library(), communitySkins));
  await act(async () =>
    pending.resolve(
      boundary === "load" ? [] : [{ id: "synthetic-old", name: item.name, design: item.design }],
    ),
  );
  if (boundary === "load") expect(old.mutate).not.toHaveBeenCalled();
  expect(screen.queryByRole("dialog", { name: "发布 AI 皮肤" })).toBeNull();
  expect(screen.queryByRole("button", { name: "已保存" })).toBeNull();
  expect(screen.queryByText("已保存到“我的皮肤”。")).toBeNull();
});
