import {
  CustomTranslationProbeDescriptor,
  CustomTranslationProbePolicy,
} from "../entry/src/main/ets/keyboard/settings/CustomTranslationProbePolicy";

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

declare const __dirname: string;
declare function require(name: "fs"): { readFileSync(path: string, encoding: "utf8"): string };

function repositoryFile(path: string): string {
  return require("fs").readFileSync(`${__dirname}/../../../${path}`, "utf8");
}

interface ContractCase {
  name: string;
  endpoint: string;
  api_key: string;
  request: CustomTranslationProbeDescriptor | null;
}

const contract = JSON.parse(repositoryFile("shared/contracts/custom-translation/cases.json")) as {
  cases: ContractCase[];
};

/** Tauri 设置页显示的 DeepL 提示，从 Rust 的 `deepl_failure_message` 读出来作为期望值，鸿蒙一份不能自己改。 */
function rustDeepLMessages(): Map<string, string> {
  const source = repositoryFile("crates/client-core/src/credential/translation.rs");
  const start = source.indexOf("fn deepl_failure_message(");
  const body = start < 0 ? "" : source.substring(start, source.indexOf("\n}\n", start));
  const messages = new Map<string, string>();
  for (const match of body.matchAll(/Some\((\d{3})\) => \{?\s*"([^"]+)"/g)) {
    messages.set(match[1], match[2]);
  }
  const fallback = /_ => "([^"]+)"/.exec(body);
  if (fallback !== null) messages.set("other", fallback[1]);
  return messages;
}

console.log("CustomTranslationProbePolicy");

group("the probe asks the shared descriptor for 测试 from Chinese to English", () => {
  const request = CustomTranslationProbePolicy.request("https://api-free.deepl.com/v2/translate", "synthetic-key:fx");
  // NAPI 按这些 snake_case 字段解析，与 Rust 设置页连接测试的 `custom_translation_request(endpoint, key, "测试", "zh", "en")` 是同一个探测。
  check(
    JSON.stringify(request) === JSON.stringify({
      config: { enabled: true, endpoint: "https://api-free.deepl.com/v2/translate", api_key: "synthetic-key:fx" },
      text: "测试",
      source_language: "zh",
      target_language: "en",
    }),
    `unexpected probe request ${JSON.stringify(request)}`,
  );
  const keyless = CustomTranslationProbePolicy.request("http://127.0.0.1:1188/translate", "");
  check(keyless.config.api_key === "", "an empty key is passed through for DeepLX, which may need none");
  check(keyless.config.enabled, "the probe tests the configuration as if it were switched on");
});

group("DeepL is recognised from the shared descriptor, for every shared case", () => {
  const described = contract.cases.filter((item: ContractCase): boolean => item.request !== null);
  check(described.length >= 10, "the shared contract is present");
  for (const item of described) {
    const descriptor = item.request as CustomTranslationProbeDescriptor;
    // 共享构造把 DeepL 地址统一改写到这两个 URL；其余地址原样保留，仿冒主机名、子域名和非默认端口都不会落到这里。
    const official =
      descriptor.url === "https://api.deepl.com/v2/translate" || descriptor.url === "https://api-free.deepl.com/v2/translate";
    check(
      CustomTranslationProbePolicy.isDeepL(descriptor) === official,
      `${item.name}: isDeepL should be ${official}`,
    );
  }
});

group("DeepL detection reads only the Authorization prefix the shared builder writes", () => {
  const descriptor = (authorization: string | null): CustomTranslationProbeDescriptor => {
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (authorization !== null) headers["Authorization"] = authorization;
    return { url: "https://api.deepl.com/v2/translate", headers: headers, body: {} };
  };
  check(CustomTranslationProbePolicy.isDeepL(descriptor("DeepL-Auth-Key synthetic-key")), "DeepL-Auth-Key is DeepL");
  check(!CustomTranslationProbePolicy.isDeepL(descriptor("Bearer synthetic-key")), "Bearer is DeepLX even on a DeepL URL");
  check(!CustomTranslationProbePolicy.isDeepL(descriptor(null)), "no Authorization header is not DeepL");
  check(!CustomTranslationProbePolicy.isDeepL(descriptor("deepl-auth-key synthetic-key")), "the prefix is matched as written");
  check(
    repositoryFile("crates/client-core/src/translation.rs").includes(
      `format!("${CustomTranslationProbePolicy.DEEPL_AUTHORIZATION_PREFIX}{api_key}")`,
    ),
    "the shared builder writes the same DeepL-Auth-Key prefix",
  );
});

group("the request carries the descriptor headers plus Accept, without touching the descriptor", () => {
  const original: Record<string, string> = {
    "Content-Type": "application/json",
    Authorization: "DeepL-Auth-Key synthetic-key",
  };
  const descriptor: CustomTranslationProbeDescriptor = { url: "https://api.deepl.com/v2/translate", headers: original, body: {} };
  const headers = CustomTranslationProbePolicy.headers(descriptor);
  check(headers["Content-Type"] === "application/json", "Content-Type is kept");
  check(headers["Authorization"] === "DeepL-Auth-Key synthetic-key", "Authorization is kept");
  check(headers["Accept"] === "application/json", "Accept is added");
  check(Object.keys(headers).length === 3, "nothing else is added");
  check(original["Accept"] === undefined, "the descriptor itself is left alone");
});

group("only a 2xx string reply within 256 KiB reaches the shared parser", () => {
  const limit = CustomTranslationProbePolicy.MAX_RESPONSE_BYTES;
  check(limit === 256 * 1024, "the reply limit is 256 KiB");
  check(CustomTranslationProbePolicy.acceptsResponse(200, "{}"), "200 with text is parsed");
  check(CustomTranslationProbePolicy.acceptsResponse(299, "{}"), "the whole 2xx range is parsed");
  for (const status of [0, 199, 300, 403, 429, 456, 500]) {
    check(!CustomTranslationProbePolicy.acceptsResponse(status, "{}"), `${status} is not parsed`);
  }
  check(!CustomTranslationProbePolicy.acceptsResponse(200, undefined), "a missing body is not parsed");
  check(!CustomTranslationProbePolicy.acceptsResponse(200, new ArrayBuffer(2)), "a binary body is not parsed");
  check(CustomTranslationProbePolicy.acceptsResponse(200, "a".repeat(limit)), "exactly 256 KiB is parsed");
  check(!CustomTranslationProbePolicy.acceptsResponse(200, "a".repeat(limit + 1)), "one byte over is not parsed");
  // 上限按 UTF-8 字节算，不按字符数：87382 个「测」是 262146 字节。
  check(!CustomTranslationProbePolicy.acceptsResponse(200, "测".repeat(87382)), "the limit counts UTF-8 bytes");
});

group("DeepL failures are explained by status, word for word as the Rust probe says them", () => {
  const rust = rustDeepLMessages();
  check(
    ["403", "429", "456", "other"].every((key: string): boolean => rust.has(key)) && rust.size === 4,
    `could not read the four DeepL messages from the Rust probe: ${Array.from(rust.keys()).join(", ")}`,
  );
  for (const status of ["403", "456", "429"]) {
    const actual = CustomTranslationProbePolicy.failureMessage(true, Number(status));
    check(actual === rust.get(status), `DeepL ${status}: Harmony ${JSON.stringify(actual)}, Rust ${JSON.stringify(rust.get(status))}`);
    check(actual.includes(`HTTP ${status}`), `the ${status} message names its status`);
  }
  // 2xx 却解析不出译文、服务端错误、连不上（没有状态码）都落到笼统的 DeepL 提示。
  for (const status of [0, 200, 400, 401, 500, 503]) {
    check(
      CustomTranslationProbePolicy.failureMessage(true, status) === rust.get("other"),
      `DeepL ${status} falls back to the general DeepL message`,
    );
  }
});

group("other services keep the general failure whatever the status", () => {
  for (const status of [0, 200, 403, 429, 456, 500]) {
    check(
      CustomTranslationProbePolicy.failureMessage(false, status) === CustomTranslationProbePolicy.GENERIC_FAILURE,
      `DeepLX ${status} is the general failure`,
    );
  }
  check(!CustomTranslationProbePolicy.GENERIC_FAILURE.includes("DeepL"), "the general failure does not mention DeepL");
});

console.log("");
if (failures > 0) {
  throw new Error(`${failures} group(s) failed`);
}
console.log(`all groups passed (${checks} assertions)`);
