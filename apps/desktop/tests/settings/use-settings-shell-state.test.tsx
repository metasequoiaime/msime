// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { useSettingsShellState, type SettingsPageId } from "@msime/ui";

afterEach(cleanup);

test("设置外壳从对话页进入时保持对话挂载，离开后仍保留挂载状态", () => {
  const hook = renderHook(({ page }: { page: SettingsPageId }) => useSettingsShellState(page), {
    initialProps: { page: "input" as SettingsPageId },
  });

  expect(hook.result.current.chatMounted).toBe(false);
  act(() => hook.rerender({ page: "chat" as SettingsPageId }));
  expect(hook.result.current.chatMounted).toBe(true);
  act(() => hook.rerender({ page: "input" as SettingsPageId }));
  expect(hook.result.current.chatMounted).toBe(true);
});

test("设置外壳的临时视图和导航状态使用稳定的初始值", () => {
  const hook = renderHook(() => useSettingsShellState("input"));

  expect(hook.result.current.pluginView).toBe("mine");
  expect(hook.result.current.skinView).toBe("mine");
  expect(hook.result.current.navQuery).toBe("");
  expect(hook.result.current.titleCollapsedOn).toBeNull();
  expect(hook.result.current.chatFocusRequest).toBe(0);

  act(() => hook.result.current.setPluginView("community"));
  act(() => hook.result.current.setSkinView("community"));
  act(() => hook.result.current.setNavQuery("皮肤"));
  act(() => hook.result.current.setTitleCollapsedOn("skin"));
  act(() => hook.result.current.setChatFocusRequest(1));

  expect(hook.result.current.pluginView).toBe("community");
  expect(hook.result.current.skinView).toBe("community");
  expect(hook.result.current.navQuery).toBe("皮肤");
  expect(hook.result.current.titleCollapsedOn).toBe("skin");
  expect(hook.result.current.chatFocusRequest).toBe(1);
});
