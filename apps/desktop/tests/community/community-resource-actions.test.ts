import { afterEach, expect, expectTypeOf, test, vi } from "vitest";
import type { CommunityLocalDictionaryClient, CommunityResource } from "@msime/ui";
import {
  canInstallCommunityDictionary,
  installCommunityDictionaryLocally,
  addCommunityReply,
} from "../../../../packages/ui/src/community/community-resource-actions";

import type * as Public from "@msime/ui";
import type * as Model from "../../../../packages/ui/src/community/community-resource-model";
import type { CommunityResourceScope as LegacyScope } from "../../../../packages/ui/src/community/community-resource-scope-buttons";

const item: CommunityResource = {
  id: "10000000-0000-4000-8000-000000000001",
  kind: "dictionary",
  name: "合成词库",
  description: "合成说明",
  author: "示例作者",
  content: {},
  revision: 1,
  saves: 0,
  saved: false,
  owned: false,
  rating_count: 0,
  rating_average: 0,
  my_rating: 0,
};

afterEach(() => vi.restoreAllMocks());

test("只有宿主提供安装或导入方法时才支持本机安装", () => {
  expect(canInstallCommunityDictionary(undefined)).toBe(false);
  expect(canInstallCommunityDictionary({})).toBe(false);
  expect(canInstallCommunityDictionary({ import: async () => ({ applied: 0 }) })).toBe(true);
  expect(canInstallCommunityDictionary({ installCollection: async () => {} })).toBe(true);
});

test("命名词库优先，保留方法接收者且不再逐条导入", async () => {
  const installed: CommunityResource[] = [];
  const dictionary = {
    installed,
    async installCollection(resource: CommunityResource) {
      this.installed.push(resource);
    },
    async import() {
      throw new Error("不能退回逐条导入");
    },
  };
  expect(await installCommunityDictionaryLocally(dictionary, item)).toEqual({ kind: "collection" });
  expect(installed).toEqual([item]);
});

test("按首次出现的类型分组，保留组内顺序和宿主实际应用条数", async () => {
  vi.spyOn(Date, "now").mockReturnValue(1234);
  const calls: unknown[][] = [];
  const dictionary = {
    calls,
    async import(...args: Parameters<NonNullable<CommunityLocalDictionaryClient["import"]>>) {
      this.calls.push(args);
      return { applied: this.calls.length };
    },
  };
  expect(
    await installCommunityDictionaryLocally(dictionary, {
      ...item,
      content: {
        entries: [
          { kind: "quick", word: "合成短语", code: "synthetic", weight: 7 },
          { kind: "pinyin", word: "合成", code: "he cheng", weight: 8 },
          { kind: "quick", word: "第二短语", code: "second", weight: 9 },
          { kind: "wubi", word: "合", code: "wgkf", weight: 10 },
          { kind: "english", word: "synthetic", code: "synthetic", weight: 11 },
        ],
      },
    }),
  ).toEqual({ kind: "entries", applied: 10 });
  expect(calls).toEqual([
    [
      "quick_phrase",
      "standard",
      "合成短语\tsynthetic\t7\n第二短语\tsecond\t9",
      "community-local-1234-quick",
    ],
    ["pinyin", "standard", "合成\the cheng\t8", "community-local-1234-pinyin"],
    ["wubi", "standard", "合\twgkf\t10", "community-local-1234-wubi"],
    ["english", "standard", "synthetic\tsynthetic\t11", "community-local-1234-english"],
  ]);
});

test("空词库应用零条，无能力时不执行任何操作", async () => {
  expect(
    await installCommunityDictionaryLocally(
      {
        import: async () => {
          throw new Error("空词库不能发请求");
        },
      },
      item,
    ),
  ).toEqual({ kind: "entries", applied: 0 });
  expect(await installCommunityDictionaryLocally({}, item)).toBeUndefined();
});

test("命名词库失败原样抛出，不回退到本机导入", async () => {
  const failure = new Error("合成安装失败");
  const imported: boolean[] = [];
  await expect(
    installCommunityDictionaryLocally(
      {
        installCollection: async () => {
          throw failure;
        },
        import: async () => {
          imported.push(true);
          return { applied: 1 };
        },
      },
      item,
    ),
  ).rejects.toBe(failure);
  expect(imported).toEqual([]);
});

test("一组导入失败后不再提交后续组", async () => {
  const failure = new Error("合成导入失败");
  const imported: string[] = [];
  await expect(
    installCommunityDictionaryLocally(
      {
        import: async (kind) => {
          imported.push(kind);
          throw failure;
        },
      },
      {
        ...item,
        content: {
          entries: [
            { kind: "pinyin", word: "合成", code: "he cheng", weight: 1 },
            { kind: "english", word: "synthetic", code: "synthetic", weight: 1 },
          ],
        },
      },
    ),
  ).rejects.toBe(failure);
  expect(imported).toEqual(["pinyin"]);
});

test("回复模板等待收藏完成，再读取并存入最新版本", async () => {
  const events: unknown[] = [];
  let saved!: () => void;
  const pending = new Promise<void>((resolve) => {
    saved = resolve;
  });
  const latest = {
    ...item,
    kind: "reply" as const,
    revision: 2,
    content: { prompt: "合成提示词" },
  };
  const client = {
    events,
    async save(id: string, value: boolean) {
      this.events.push(["save", id, value]);
      await pending;
    },
    async detail(id: string) {
      this.events.push(["detail", id]);
      return latest;
    },
    async storeReply(resource: CommunityResource) {
      this.events.push(["store", resource]);
    },
  };
  const result = addCommunityReply(client, item);
  expect(events).toEqual([["save", item.id, true]]);
  saved();
  expect(await result).toBe("已添加「合成词库」到高情商回复键盘");
  expect(events).toEqual([
    ["save", item.id, true],
    ["detail", item.id],
    ["store", latest],
  ]);
});

test("没有回复键盘的宿主仍收藏并读取详情", async () => {
  const events: string[] = [];
  expect(
    await addCommunityReply(
      {
        save: async () => {
          events.push("save");
        },
        detail: async () => {
          events.push("detail");
          return item;
        },
      },
      item,
    ),
  ).toBe("已收藏「合成词库」");
  expect(events).toEqual(["save", "detail"]);
});

test.each(["save", "detail", "store"])("回复添加在 %s 失败时停止并传递错误", async (stage) => {
  const events: string[] = [];
  const failure = new Error("合成回复失败");
  const step = (name: string) => {
    events.push(name);
    if (name === stage) throw failure;
  };
  await expect(
    addCommunityReply(
      {
        save: async () => {
          step("save");
        },
        detail: async () => {
          step("detail");
          return item;
        },
        storeReply: async () => {
          step("store");
        },
      },
      item,
    ),
  ).rejects.toBe(failure);
  expect(events).toEqual(
    ["save", "detail", "store"].slice(0, ["save", "detail", "store"].indexOf(stage) + 1),
  );
});

test("公共资源模型保持包入口和旧范围组件契约", () => {
  expectTypeOf<Public.CommunityResource>().toEqualTypeOf<Model.CommunityResource>();
  expectTypeOf<Public.CommunityResourceClient>().toEqualTypeOf<Model.CommunityResourceClient>();
  expectTypeOf<Public.CommunityLocalDictionaryClient>().toEqualTypeOf<Model.CommunityLocalDictionaryClient>();
  expectTypeOf<Public.CommunityResourceKind>().toEqualTypeOf<Model.CommunityResourceKind>();
  expectTypeOf<Public.CommunityResourceContent>().toEqualTypeOf<Model.CommunityResourceContent>();
  expectTypeOf<Public.CommunityResourcePage>().toEqualTypeOf<Model.CommunityResourcePage>();
  expectTypeOf<Public.CommunityResourceApplication>().toEqualTypeOf<Model.CommunityResourceApplication>();
  expectTypeOf<Public.CommunitySharedWord>().toEqualTypeOf<Model.CommunitySharedWord>();
  expectTypeOf<Public.CommunityResourceScope>().toEqualTypeOf<Model.CommunityResourceScope>();
  expectTypeOf<LegacyScope>().toEqualTypeOf<Model.CommunityResourceScope>();
});
