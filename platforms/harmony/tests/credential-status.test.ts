import { CredentialStatusPolicy } from "../entry/src/main/ets/keyboard/settings/CredentialStatusPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有这些定义。 */
function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) {
    throw new Error(message);
  }
}

console.log("CredentialStatusPolicy");

group("balance and server failures do not send the user back to the endpoint and model", () => {
  const other = "服务拒绝请求，请检查接口地址和模型配置";
  for (const [status, expected] of [
    [401, "认证失败（HTTP 401），请检查 API Key 和访问权限。"],
    [403, "认证失败（HTTP 403），请检查 API Key 和访问权限。"],
    [402, "账户余额不足（HTTP 402），请到服务商控制台充值后重试。"],
    [429, "服务限流或额度不足（HTTP 429），请稍后重试。"],
    [500, "服务暂时不可用（HTTP 500），请稍后重试；一直如此时，请确认该服务支持所填的模型。"],
    [503, "服务暂时不可用（HTTP 503），请稍后重试；一直如此时，请确认该服务支持所填的模型。"],
    [404, "服务拒绝请求，请检查接口地址和模型配置（HTTP 404）"],
    [422, "服务拒绝请求，请检查接口地址和模型配置（HTTP 422）"],
  ] as [number, string][]) {
    const message = CredentialStatusPolicy.rejected(status, other);
    check(message === expected, `${status}: ${message}`);
  }
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
