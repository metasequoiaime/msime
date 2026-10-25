import { describe, expect, test } from "vitest";
import { createDesktopOobeClient } from "../../src/core/oobe-host";
import {
  oobeComplete,
  oobeFlowComplete,
  oobeNextAction,
  oobeSystemReady,
  type OobeState,
} from "../../../../packages/ui/src/core/host-contracts";

const ready = (): OobeState => ({
  resourcesReady: true,
  systemEnabled: true,
  systemCurrent: true,
  preferencesSaved: true,
  onboardingSeen: true,
});

test("桌面从原生真实快照读取，注册结果缺失不意味着未启用", async () => {
  const client = createDesktopOobeClient(
    "linux",
    async <T>() =>
      ({
        resources_ready: true,
        system_enabled: null,
        system_current: null,
        preferences_saved: true,
        onboarding_seen: null,
      }) as T,
    async () => {},
  )!;
  const state = await new Promise<OobeState>((resolve) => {
    const stop = client.subscribe((value) => {
      resolve(value);
      stop();
    });
  });
  expect(state.systemEnabled).toBeNull();
  expect(state.onboardingSeen).toBeNull();
  expect(oobeComplete(state)).toBe(false);
});

test("查询失败清除旧的成功结果，后续刷新可以恢复", async () => {
  let fails = false;
  const client = createDesktopOobeClient(
    "macos",
    async <T>() => {
      if (fails) throw new Error("synthetic failure");
      return {
        resources_ready: true,
        system_enabled: true,
        system_current: null,
        preferences_saved: true,
        onboarding_seen: null,
      } as T;
    },
    async () => {},
  )!;
  const values: OobeState[] = [];
  const stop = client.subscribe((value) => values.push(value));
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(client.read()?.resourcesReady).toBe(true);
  fails = true;
  client.refresh?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(client.read()?.resourcesReady).toBeNull();
  expect(client.read()?.systemEnabled).toBeNull();
  fails = false;
  client.refresh?.();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(client.read()?.systemEnabled).toBe(true);
  stop();
  expect(values).toHaveLength(3);
});

test.each([undefined, null, {}, { resources_ready: "true", system_enabled: 1 }])(
  "原生快照缺失或字段无效时保留未知：%j",
  async (reply) => {
    const client = createDesktopOobeClient(
      "linux",
      async <T>() => reply as T,
      async () => {},
    )!;
    const state = await new Promise<OobeState>((resolve) => {
      const stop = client.subscribe((value) => {
        stop();
        resolve(value);
      });
    });
    expect(Object.values(state)).toEqual([null, null, null, null, null]);
  },
);

describe("跨平台 OOBE 状态契约", () => {
  test("完整快照才算系统和流程都完成", () => {
    const state = ready();
    expect(oobeFlowComplete(state)).toBe(true);
    expect(oobeSystemReady(state)).toBe(true);
    expect(oobeComplete(state)).toBe(true);
    expect(oobeNextAction(state)).toBeNull();
  });

  test("明确失败按资源、启用、默认、偏好顺序给出步骤", () => {
    const state = ready();
    state.resourcesReady = false;
    expect(oobeNextAction(state)).toBe("prepare_resources");
    state.resourcesReady = true;
    state.systemEnabled = false;
    expect(oobeNextAction(state)).toBe("enable_system_input_method");
    state.systemEnabled = true;
    state.systemCurrent = false;
    expect(oobeNextAction(state)).toBe("make_default_input_method");
    state.systemCurrent = true;
    state.preferencesSaved = false;
    expect(oobeNextAction(state)).toBe("save_preferences");
  });

  test("iOS 等未知系统能力既不误报失败也不误报完成", () => {
    const state = ready();
    state.systemEnabled = null;
    state.systemCurrent = null;
    expect(oobeFlowComplete(state)).toBe(true);
    expect(oobeSystemReady(state)).toBe(false);
    expect(oobeComplete(state)).toBe(false);
    expect(oobeNextAction(state)).toBeNull();
  });
});
