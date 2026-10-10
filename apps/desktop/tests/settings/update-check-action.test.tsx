// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useUpdateCheck, type UpdateCheckResult } from "@msime/ui";

afterEach(() => {
  vi.restoreAllMocks();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

const page = "https://github.com/metasequoiaime/msime/releases";

function available(version: string): UpdateCheckResult {
  return {
    status: "available",
    update: {
      version: { display: version, parts: version.split(".").map(Number) },
      release_url: `${page}/tag/windows-v${version}`,
      installer_name: null,
      installer_sha256: null,
      signed: false,
    },
  };
}

test("an update check already in progress ignores a second trigger", async () => {
  const request = deferred<UpdateCheckResult>();
  const checkUpdate = vi.fn().mockReturnValue(request.promise);
  const { result } = renderHook(() =>
    useUpdateCheck({ checkUpdate, platform: "windows", currentAppVersion: "1.0.0" }),
  );

  let first!: Promise<void>;
  act(() => {
    first = result.current.checkForUpdate();
  });
  await waitFor(() => expect(result.current.busy).toBe(true));

  let second!: Promise<void>;
  act(() => {
    second = result.current.checkForUpdate();
  });
  expect(checkUpdate).toHaveBeenCalledOnce();

  request.reject(new Error("update_check_unavailable"));
  await act(async () => {
    await first;
    await second;
  });
  expect(result.current.status).toBe("检查失败，请稍后重试");
});

test("ignores a same-tick duplicate update check", async () => {
  const request = deferred<UpdateCheckResult>();
  const checkUpdate = vi.fn().mockReturnValue(request.promise);
  const { result } = renderHook(() =>
    useUpdateCheck({ checkUpdate, platform: "windows", currentAppVersion: "1.0.0" }),
  );

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.checkForUpdate();
    second = result.current.checkForUpdate();
  });
  expect(checkUpdate).toHaveBeenCalledOnce();
  request.reject(new Error("update_check_unavailable"));
  await act(async () => {
    await first;
    await second;
  });
});

test("a check started for an older app version is ignored after the version changes", async () => {
  const request = deferred<UpdateCheckResult>();
  const checkUpdate = vi.fn().mockReturnValue(request.promise);
  const { result, rerender } = renderHook(
    ({ currentAppVersion }) =>
      useUpdateCheck({ checkUpdate, platform: "windows", currentAppVersion }),
    { initialProps: { currentAppVersion: "1.0.0" } },
  );

  let pending!: Promise<void>;
  act(() => {
    pending = result.current.checkForUpdate();
  });
  await waitFor(() => expect(result.current.busy).toBe(true));
  rerender({ currentAppVersion: "2.0.0" });
  request.resolve(available("1.5.0"));
  await act(async () => pending);

  expect(result.current.available).toBeNull();
  expect(result.current.status).toBe("");
  expect(result.current.busy).toBe(false);
});

test("the host is asked with the platform, running version, edition and architecture", async () => {
  const checkUpdate = vi.fn().mockResolvedValue(available("1.5.0"));
  const { result } = renderHook(() =>
    useUpdateCheck({
      checkUpdate,
      platform: "linux",
      edition: "wubi",
      arch: "aarch64",
      currentAppVersion: "1.0.0",
    }),
  );
  await act(async () => result.current.checkForUpdate());
  expect(checkUpdate).toHaveBeenCalledWith({
    platform: "linux",
    currentVersion: "1.0.0",
    edition: "wubi",
    arch: "aarch64",
  });
  expect(result.current.status).toBe("发现新版本 v1.5.0");
  expect(result.current.available?.releaseUrl).toBe(`${page}/tag/windows-v1.5.0`);
});

test("without a host check or a platform there is nothing to check", async () => {
  const checkUpdate = vi.fn();
  for (const options of [
    { platform: "windows", currentAppVersion: "1.0.0" },
    { checkUpdate, platform: null, currentAppVersion: "1.0.0" },
  ]) {
    const { result } = renderHook(() => useUpdateCheck(options));
    expect(result.current.supported).toBe(false);
    await act(async () => result.current.checkForUpdate());
    expect(result.current.status).toBe("");
  }
  expect(checkUpdate).not.toHaveBeenCalled();
});

test("a release the page cannot show safely is reported as a failed check", async () => {
  const outside: UpdateCheckResult = {
    status: "available",
    update: {
      ...(available("1.5.0") as { update: object }).update,
      release_url: "https://github.com/metasequoiaime/MSIME-Windows/releases/tag/v1.5.0",
    } as Extract<UpdateCheckResult, { update: unknown }>["update"],
  };
  const checkUpdate = vi.fn().mockResolvedValue(outside);
  const { result } = renderHook(() =>
    useUpdateCheck({ checkUpdate, platform: "windows", currentAppVersion: "1.0.0" }),
  );
  await act(async () => result.current.checkForUpdate());
  expect(result.current.available).toBeNull();
  expect(result.current.status).toBe("检查失败，请稍后重试");
});
