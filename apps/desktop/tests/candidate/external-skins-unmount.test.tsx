// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useSkinCatalog, type SkinCatalog } from "../../../../packages/ui/src/skin/external-skins";

afterEach(cleanup);

const catalog: SkinCatalog = { directory: "/synthetic/skins", issues: [], packages: [] };

test("a completed skin import does not rescan after the catalog unmounts", async () => {
  const scan = vi.fn(async () => catalog);
  let resolveImport!: () => void;
  const openDirectory = () =>
    new Promise<void>((resolve) => {
      resolveImport = resolve;
    });
  const { result, unmount } = renderHook(() => useSkinCatalog(scan, openDirectory, true));
  await waitFor(() => expect(result.current.catalog).toEqual(catalog));
  let importing!: Promise<void>;
  act(() => {
    importing = result.current.openFolder();
  });

  unmount();
  await act(async () => {
    resolveImport();
    await importing;
  });

  expect(scan).toHaveBeenCalledTimes(1);
});
