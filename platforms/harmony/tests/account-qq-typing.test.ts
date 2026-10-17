// 「绑定 QQ 群助手」和打字统计「上传到云端」在账号桥接里的那一半：发什么请求、怎样校验回答、服务端的拒绝码怎样换成页面认得的码。上传的请求体由 Rust 的统计存储生成（client-services 里测），这里只看桥接把它原样发出、限大小、只用登录账号。
import {
  AccountCloudBridge,
  AccountSessionStore,
  AccountTransport,
  AccountTransportResponse,
} from "../entry/src/main/ets/account/AccountCloudBridge";

let failures = 0;
let checks = 0;

async function group(name: string, body: () => Promise<void>): Promise<void> {
  try {
    await body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) throw new Error(message);
}

type Json = Record<string, unknown>;
type Call = { method: string; path: string; token?: string; body?: Json };

const ACCESS = "a".repeat(64);

function session(): string {
  return JSON.stringify({
    access_token: ACCESS,
    refresh_token: "b".repeat(64),
    token_type: "Bearer",
    expires_at: Date.now() + 600_000,
    user: { id: "synthetic-user", display_name: "Synthetic", created_at: "2026-01-01" },
  });
}

function harness(
  respond: (call: Call) => AccountTransportResponse,
  signedIn = true,
): { bridge: AccountCloudBridge; calls: Call[]; handle(action: Json): Promise<Json> } {
  const calls: Call[] = [];
  let stored: string | null = signedIn ? session() : null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const transport: AccountTransport = {
    request: async (method, path, token, body) => {
      const call: Call = { method, path, token, body };
      calls.push(call);
      return respond(call);
    },
  };
  const bridge = new AccountCloudBridge(transport, store);
  const handle = async (action: Json): Promise<Json> =>
    JSON.parse(await bridge.handle(JSON.stringify(action))) as Json;
  return { bridge, calls, handle };
}

function reply(status: number, body: unknown = ""): AccountTransportResponse {
  return { status, body: typeof body === "string" ? body : JSON.stringify(body) };
}

const LINK = { id: "12", created_at: "2026-10-17T08:00:00Z" };

async function main(): Promise<void> {
  console.log("account QQ links and typing statistics upload");

  await group(
    "the link list keeps only id and created_at and refuses a malformed row",
    async () => {
      const ok = harness(() => reply(200, { links: [{ ...LINK, subject: "never shown" }] }));
      const listed = await ok.handle({ operation: "qq_links" });
      check(listed.ok === true, "a valid list is accepted");
      check(
        JSON.stringify(listed.value) === JSON.stringify({ links: [LINK] }),
        `only the two fields reach the page: ${JSON.stringify(listed.value)}`,
      );
      check(
        ok.calls[0].method === "GET" && ok.calls[0].path === "/v1/users/me/qq-links",
        "the list is one GET",
      );
      check(ok.calls[0].token === ACCESS, "the list carries the signed-in token");
      for (const links of [
        [{ id: "0", created_at: LINK.created_at }],
        [{ id: "12", created_at: "not a date" }],
        Array.from({ length: 11 }, () => LINK),
        "no",
      ]) {
        const bad = harness(() => reply(200, { links }));
        const refused = await bad.handle({ operation: "qq_links" });
        check(
          refused.error === "account_unavailable",
          `refused ${JSON.stringify(links).slice(0, 40)}`,
        );
      }
    },
  );

  await group(
    "a code is normalised, checked before sending and its refusals keep their codes",
    async () => {
      const ok = harness(() => reply(201, LINK));
      const linked = await ok.handle({ operation: "qq_link_redeem", code: " 1234-5678 " });
      check(linked.ok === true && (linked.value as Json).id === "12", "a valid code links");
      check(
        ok.calls[0].method === "POST" && JSON.stringify(ok.calls[0].body) === '{"code":"12345678"}',
        `spaces and dashes are removed: ${JSON.stringify(ok.calls[0].body)}`,
      );
      const short = harness(() => reply(201, LINK));
      check(
        (await short.handle({ operation: "qq_link_redeem", code: "1234" })).error ===
          "account_invalid",
        "seven digits or fewer are refused",
      );
      check(short.calls.length === 0, "a malformed code never reaches the network");
      const cases: [AccountTransportResponse, string][] = [
        [
          reply(404, { error: { code: "qq_link_code_not_found", message: "x" } }),
          "account_qq_link_code_not_found",
        ],
        [reply(409, { error: { code: "qq_link_limit", message: "x" } }), "account_qq_link_limit"],
        [
          reply(429, { error: { code: "rate_limit_exceeded", message: "x" } }),
          "account_rate_limited",
        ],
        [reply(201, { id: "x", created_at: LINK.created_at }), "account_unavailable"],
      ];
      for (const [response, code] of cases) {
        const refused = harness(() => response);
        const answer = await refused.handle({ operation: "qq_link_redeem", code: "12345678" });
        check(
          answer.error === code,
          `${response.status} becomes ${code}, got ${String(answer.error)}`,
        );
      }
    },
  );

  await group("unlinking checks the id and maps a missing link", async () => {
    const ok = harness(() => reply(204));
    const removed = await ok.handle({ operation: "qq_link_delete", id: "12" });
    check(removed.ok === true, "an own link is removed");
    check(
      ok.calls[0].method === "DELETE" && ok.calls[0].path === "/v1/users/me/qq-links/12",
      "the id is in the path",
    );
    const traversal = harness(() => reply(204));
    check(
      (await traversal.handle({ operation: "qq_link_delete", id: "../12" })).error ===
        "account_invalid",
      "a non-numeric id is refused",
    );
    check(traversal.calls.length === 0, "a refused id never reaches the network");
    const missing = harness(() =>
      reply(404, { error: { code: "qq_link_not_found", message: "x" } }),
    );
    check(
      (await missing.handle({ operation: "qq_link_delete", id: "12" })).error ===
        "account_not_found",
      "someone else's link is not found",
    );
  });

  await group(
    "the upload sends the store's body as it is and only with the signed-in account",
    async () => {
      const body = {
        device: "dev",
        platform: "harmony",
        days: [{ day: "2026-10-17", characters: 3, active_seconds: 1 }],
      };
      const ok = harness(() => reply(204));
      const sent = JSON.parse(await ok.bridge.typingStatisticsUpload(body)) as Json;
      check(sent.ok === true, "a 204 is success");
      check(
        ok.calls[0].method === "PUT" &&
          ok.calls[0].path === "/v1/users/me/typing-statistics" &&
          JSON.stringify(ok.calls[0].body) === JSON.stringify(body),
        "the body goes out unchanged",
      );
      const big = harness(() => reply(204));
      const tooLarge = JSON.parse(
        await big.bridge.typingStatisticsUpload({ device: "dev", padding: "x".repeat(70 * 1024) }),
      ) as Json;
      check(
        tooLarge.error === "account_invalid" && big.calls.length === 0,
        "an oversized body is refused locally",
      );
      const signedOut = harness(() => reply(204), false);
      const refused = JSON.parse(await signedOut.bridge.typingStatisticsUpload(body)) as Json;
      check(refused.error === "account_unauthorized", "without a sign-in nothing is sent");
      check(signedOut.calls.length === 0, "no request without a session");
      const limited = harness(() =>
        reply(429, { error: { code: "rate_limit_exceeded", message: "x" } }),
      );
      check(
        (JSON.parse(await limited.bridge.typingStatisticsUpload(body)) as Json).error ===
          "account_rate_limited",
        "a rate limit keeps its code",
      );
      const removed = harness(() => reply(204));
      check(
        (JSON.parse(await removed.bridge.typingStatisticsDelete()) as Json).ok === true &&
          removed.calls[0].method === "DELETE" &&
          removed.calls[0].path === "/v1/users/me/typing-statistics",
        "turning upload off deletes the cloud copy",
      );
    },
  );

  console.log("");
  if (failures > 0) throw new Error(`${failures} group(s) failed`);
  console.log(`all groups passed (${checks} assertions)`);
}

void main();
