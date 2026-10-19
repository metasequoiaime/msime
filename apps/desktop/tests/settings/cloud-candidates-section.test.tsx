// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CloudCandidatesSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("cloud candidates switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<CloudCandidatesSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("云候选"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("cloud candidates switch uses the enabled default", () => {
  render(<CloudCandidatesSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("云候选") as HTMLInputElement).checked).toBe(true);
});

test("custom cloud settings expose HTTP fields and name only the recipient host", () => {
  const onProviderChange = vi.fn();
  const provider = {
    source: "custom" as const,
    endpoint: "https://cloud.example.invalid/api?q={query}&access=synthetic",
    method: "POST" as const,
    headers_json: '{"Authorization":"Bearer synthetic"}',
    body_template: '{"messages":[{"role":"user","content":"{query}"}]}',
    response_format: "json_path" as const,
    response_path: "choices.0.message.content",
    timeout_ms: 200,
    cache_enabled: false,
  };
  render(
    <CloudCandidatesSection
      value={true}
      onChange={vi.fn()}
      provider={provider}
      onProviderChange={onProviderChange}
    />,
  );
  expect(
    screen.getByText(/仅将当前正在组的拼写发送给 cloud.example.invalid/).textContent,
  ).not.toContain("access=");
  expect((screen.getByLabelText("云候选请求体模板") as HTMLTextAreaElement).value).toBe(
    provider.body_template,
  );
  expect((screen.getByLabelText("云候选 JSON 路径") as HTMLInputElement).value).toBe(
    provider.response_path,
  );
  fireEvent.change(screen.getByLabelText("云候选请求头"), {
    target: { value: '{"X-Token":"synthetic-new"}' },
  });
  expect(onProviderChange).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存云候选服务" }));
  expect(onProviderChange).toHaveBeenLastCalledWith({
    ...provider,
    headers_json: '{"X-Token":"synthetic-new"}',
  });
  fireEvent.change(screen.getByLabelText("云候选超时"), { target: { value: "500" } });
  fireEvent.click(screen.getByRole("button", { name: "保存云候选服务" }));
  expect(onProviderChange).toHaveBeenLastCalledWith({
    ...provider,
    headers_json: '{"X-Token":"synthetic-new"}',
    timeout_ms: 500,
  });
  fireEvent.click(screen.getByLabelText("缓存云候选"));
  fireEvent.click(screen.getByRole("button", { name: "保存云候选服务" }));
  expect(onProviderChange).toHaveBeenLastCalledWith({
    ...provider,
    headers_json: '{"X-Token":"synthetic-new"}',
    timeout_ms: 500,
    cache_enabled: true,
  });
  fireEvent.change(screen.getByLabelText("云候选来源"), { target: { value: "google" } });
  fireEvent.click(screen.getByRole("button", { name: "保存云候选服务" }));
  expect(onProviderChange).toHaveBeenLastCalledWith({
    ...provider,
    headers_json: '{"X-Token":"synthetic-new"}',
    timeout_ms: 500,
    cache_enabled: true,
    source: "google",
  });
});

// 来源和地址一起保存，不能因空地址被共享校验拒绝而卡在 Google。
test("can configure a new custom service before committing the source", () => {
  const onProviderChange = vi.fn();
  render(
    <CloudCandidatesSection value={false} onChange={vi.fn()} onProviderChange={onProviderChange} />,
  );
  fireEvent.change(screen.getByLabelText("云候选来源"), { target: { value: "custom" } });
  fireEvent.change(screen.getByLabelText("云候选接口地址"), {
    target: { value: "https://new.example.invalid/?q={query}" },
  });
  expect(onProviderChange).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存云候选服务" }));
  expect(onProviderChange).toHaveBeenCalledWith(
    expect.objectContaining({
      source: "custom",
      endpoint: "https://new.example.invalid/?q={query}",
    }),
  );
});
