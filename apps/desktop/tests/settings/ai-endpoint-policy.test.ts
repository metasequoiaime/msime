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
