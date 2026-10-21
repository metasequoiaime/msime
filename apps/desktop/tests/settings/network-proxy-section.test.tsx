// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { NetworkProxySection } from "@msime/ui";
import {
  defaultNetworkProxy,
  networkProxyServerIssue,
  updateNetworkProxy,
} from "../../../../packages/ui/src/settings/network-proxy-preferences";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

/** 出厂值与 Windows 正式版的 `[network]` 相同：跟随系统，不填服务器。 */
test("proxy defaults follow the Windows release", () => {
  expect(defaultNetworkProxy).toEqual({ proxy_mode: "system", proxy_server: "" });
  expect(updateNetworkProxy(undefined, {})).toEqual(defaultNetworkProxy);
});

/** 三种模式都能选，改动合成完整的 `network` 一节。 */
test("choosing a mode reports the whole network section", () => {
  const onChange = vi.fn();
  render(<NetworkProxySection value={undefined} onChange={onChange} />);
  expect((screen.getByLabelText("代理") as HTMLSelectElement).value).toBe("system");
  expect(screen.queryByLabelText("代理地址")).toBeNull();

  fireEvent.change(screen.getByLabelText("代理"), { target: { value: "custom" } });
  expect(onChange).toHaveBeenLastCalledWith({ proxy_mode: "custom", proxy_server: "" });

  fireEvent.change(screen.getByLabelText("代理"), { target: { value: "none" } });
  expect(onChange).toHaveBeenLastCalledWith({ proxy_mode: "none", proxy_server: "" });
});

/** custom 模式下才出现地址行，输入直接写进草稿。 */
test("the custom server row appears only in custom mode and edits the draft", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <NetworkProxySection value={{ proxy_mode: "custom", proxy_server: "" }} onChange={onChange} />,
  );
  const input = screen.getByLabelText("代理地址") as HTMLInputElement;
  expect(input.value).toBe("");
  fireEvent.change(input, { target: { value: "127.0.0.1:7890" } });
  expect(onChange).toHaveBeenLastCalledWith({
    proxy_mode: "custom",
    proxy_server: "127.0.0.1:7890",
  });

  rerender(<NetworkProxySection value={{ proxy_mode: "system" }} onChange={onChange} />);
  expect(screen.queryByLabelText("代理地址")).toBeNull();
});

/** 与 Rust 的 `valid_proxy_server` 同口径：`host:port`，可带前缀，留空表示回退系统代理。 */
test("proxy server validation matches the Rust rule", () => {
  for (const valid of [
    "",
    "127.0.0.1:7890",
    "http://127.0.0.1:7890",
    "https://proxy.example.test:8443",
    "http://proxy.example.test:80",
    "https://proxy.example.test:443",
    "proxy.example.test:00080",
    "http://[::1]:80",
  ]) {
    expect(networkProxyServerIssue(valid)).toBe("");
  }
  for (const invalid of [
    "127.0.0.1",
    "http://proxy.example.test",
    "https://proxy.example.test",
    "http://[::1]",
    "http://proxy.example.test:",
    "http://proxy.example.test:0",
    "http://@proxy.example.test:80",
    "http://proxy.example.test:80?",
    "socks5://127.0.0.1:1080",
    "http://user:pass@127.0.0.1:7890",
    "http://127.0.0.1:7890/path",
    "http://127.0.0.1:7890?x=1",
  ]) {
    expect(networkProxyServerIssue(invalid)).not.toBe("");
  }
});
