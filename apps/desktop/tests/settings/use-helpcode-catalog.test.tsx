// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useCustomHelpcodeSchemas,
  useHelpcodePacks,
  type CustomHelpcodeSchema,
  type HelpcodePackOption,
  type PluginCatalogResult,
  type PluginClient,
} from "@msime/ui";

afterEach(cleanup);

type SchemaProps = { reader: Parameters<typeof useCustomHelpcodeSchemas>[0] };
type PackProps = { catalog: PluginClient["catalog"] | undefined; open: boolean };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function schema(stem: string): CustomHelpcodeSchema {
  return { schema: `custom/${stem}`, file_stem: stem, name: `示例${stem}`, name_en: stem };
}

function catalogResult(id: string): PluginCatalogResult {
  return {
    packages: [
      {
        id,
        kind: "helpcode",
        name: `示例${id}`,
        version: "1",
        license: "MIT",
        author: null,
        description: null,
        builtin: false,
      },
    ],
    issues: [],
  };
}

test.each(["resolve", "reject"])(
  "an older schema reader that %s cannot change the new list",
  async (outcome) => {
    const pending = deferred<CustomHelpcodeSchema[]>();
    const oldReader = vi.fn(() => pending.promise);
    const newReader = vi.fn(async () => [schema("new")]);
    const { result, rerender } = renderHook<CustomHelpcodeSchema[], SchemaProps>(
      ({ reader }) => useCustomHelpcodeSchemas(reader),
      { initialProps: { reader: oldReader } },
    );
    rerender({ reader: newReader });
    await waitFor(() => expect(result.current).toEqual([schema("new")]));
    await act(async () => {
      if (outcome === "resolve") pending.resolve([schema("old")]);
      else pending.reject(new Error("synthetic stale failure"));
      await pending.promise.catch(() => undefined);
    });
    expect(result.current).toEqual([schema("new")]);
  },
);

test("schema loading retains the previous list until the current reader fails", async () => {
  const pending = deferred<CustomHelpcodeSchema[]>();
  const reader = vi.fn(async () => [schema("old")]);
  const failingReader = vi.fn(() => pending.promise);
  const { result, rerender } = renderHook<CustomHelpcodeSchema[], SchemaProps>(
    ({ reader }) => useCustomHelpcodeSchemas(reader),
    { initialProps: { reader } },
  );
  await waitFor(() => expect(result.current).toEqual([schema("old")]));
  rerender({ reader: failingReader });
  expect(result.current).toEqual([schema("old")]);
  await act(async () => {
    pending.reject(new Error("synthetic current failure"));
    await pending.promise.catch(() => undefined);
  });
  expect(result.current).toEqual([]);
});

test("removing schema-reading capability clears the previous list", async () => {
  const reader = vi.fn(async () => [schema("table")]);
  const { result, rerender } = renderHook<CustomHelpcodeSchema[], SchemaProps>(
    ({ reader }) => useCustomHelpcodeSchemas(reader),
    { initialProps: { reader } },
  );
  await waitFor(() => expect(result.current).toHaveLength(1));
  rerender({ reader: undefined });
  expect(result.current).toEqual([]);
});

test("packs filter the catalog and refresh when the input page reopens", async () => {
  const mixed: PluginCatalogResult = {
    packages: [
      ...catalogResult("table").packages,
      { ...catalogResult("sound").packages[0], kind: "sound" },
    ],
    issues: [],
  };
  const catalog = vi
    .fn<PluginClient["catalog"]>()
    .mockResolvedValueOnce(mixed)
    .mockResolvedValueOnce(catalogResult("new"));
  const { result, rerender } = renderHook(({ open }) => useHelpcodePacks(catalog, open), {
    initialProps: { open: false },
  });
  expect(catalog).not.toHaveBeenCalled();
  rerender({ open: true });
  await waitFor(() => expect(result.current).toEqual([{ id: "table", name: "示例table" }]));
  rerender({ open: false });
  expect(result.current).toEqual([]);
  rerender({ open: true });
  await waitFor(() => expect(result.current).toEqual([{ id: "new", name: "示例new" }]));
  expect(catalog).toHaveBeenCalledTimes(2);
});

test.each(["resolve", "reject"])(
  "a previous page request that %s cannot change the reopened list",
  async (outcome) => {
    const pending = deferred<PluginCatalogResult>();
    const catalog = vi
      .fn<PluginClient["catalog"]>()
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce(catalogResult("new"));
    const { result, rerender } = renderHook(({ open }) => useHelpcodePacks(catalog, open), {
      initialProps: { open: true },
    });
    rerender({ open: false });
    rerender({ open: true });
    await waitFor(() => expect(result.current).toEqual([{ id: "new", name: "示例new" }]));
    await act(async () => {
      if (outcome === "resolve") pending.resolve(catalogResult("old"));
      else pending.reject(new Error("synthetic stale failure"));
      await pending.promise.catch(() => undefined);
    });
    expect(result.current).toEqual([{ id: "new", name: "示例new" }]);
  },
);

test("current catalog failures clear a nonempty pack list", async () => {
  const pending = deferred<PluginCatalogResult>();
  const catalog = vi.fn(async () => catalogResult("old"));
  const failingCatalog = vi.fn(() => pending.promise);
  const { result, rerender } = renderHook<HelpcodePackOption[], PackProps>(
    ({ catalog, open }) => useHelpcodePacks(catalog, open),
    { initialProps: { catalog, open: true } },
  );
  await waitFor(() => expect(result.current).toEqual([{ id: "old", name: "示例old" }]));
  rerender({ catalog: failingCatalog, open: true });
  await act(async () => {
    pending.reject(new Error("synthetic current failure"));
    await pending.promise.catch(() => undefined);
  });
  expect(result.current).toEqual([]);
});

test("removing catalog capability clears a previous pack list", async () => {
  const catalog = vi.fn(async () => catalogResult("table"));
  const { result, rerender } = renderHook<HelpcodePackOption[], PackProps>(
    ({ catalog, open }) => useHelpcodePacks(catalog, open),
    { initialProps: { catalog, open: true } },
  );
  await waitFor(() => expect(result.current).toHaveLength(1));
  rerender({ catalog: undefined, open: true });
  expect(result.current).toEqual([]);
});
