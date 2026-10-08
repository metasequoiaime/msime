import { expect, test } from "vitest";
import { aiCredentialOrigin, aiEndpointHint, aiEndpointProblem } from "@msime/ui";
import contract from "../../../../shared/contracts/ai-endpoint/cases.json";

// 与 crates/client-core/src/ai/endpoint.rs 跑同一组用例：设置页的判断和 Rust 不一致时，页面会放行宿主拒绝的地址，或者把 Token 存在宿主找不到的来源键下。
test.each(contract.cases)("$endpoint is $result", ({ endpoint, result, origin }) => {
  const problem = aiEndpointProblem(endpoint);
  expect(problem ?? "allowed").toBe(result);
  expect(aiCredentialOrigin(endpoint)).toBe(origin);
});

test("explains why a public http endpoint is refused", () => {
  expect(aiEndpointHint("http://api.example.com/v1")).toContain("http:// 只能用于本机或局域网地址");
  expect(aiEndpointHint("http://api.example.com/v1")).toContain("https://");
  expect(aiEndpointHint("http://192.168.1.20:1234/v1")).toBe("");
  expect(aiEndpointHint("not a url")).toContain("请填写完整的接口地址");
});

// 与 Rust 的 `rejects_http_hosts_not_written_canonically` 一致：明文地址的主机只认规范写法，这些写法各宿主结论不同，不放进共享用例。
test.each([
  "http://0x7f000001:1234/v1",
  "http://10.1:1234/v1",
  "http://0x08080808/v1",
  "http://127.0.0.1./v1",
  "http://%6cocalhost/v1",
  "http://[0:0::1]/v1",
  "http://@localhost/v1",
])("refuses the non-canonical http host in %s", (endpoint) => {
  expect(aiEndpointProblem(endpoint)).toBe("invalid");
});

test("keeps https hosts and surrounding spaces as before", () => {
  expect(aiEndpointProblem("https://0x7f000001:1234/v1")).toBeNull();
  expect(aiEndpointProblem(" http://192.168.1.20:1234 ")).toBeNull();
});
