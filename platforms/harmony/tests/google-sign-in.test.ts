import {
  AccountCloudBridge,
  AccountSessionStore,
  AccountTransport,
} from "../entry/src/main/ets/account/AccountCloudBridge";
import { GoogleLoopbackRequestHead } from "../entry/src/main/ets/account/GoogleLoopbackRequestHead";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有这些定义。 */
async function group(name: string, body: () => void | Promise<void>): Promise<void> {
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
  if (!condition) {
    throw new Error(message);
  }
}

function bytes(text: string): Uint8Array {
  const value = new Uint8Array(text.length);
  for (let index = 0; index < text.length; index++) value[index] = text.charCodeAt(index);
  return value;
}

type Call = { method: string; path: string; body?: Record<string, unknown> };

function bridge(): { bridge: AccountCloudBridge; calls: Call[]; stored: () => string | null } {
  let stored: string | null = null;
  const store: AccountSessionStore = {
    load: () => stored,
    save: (value) => {
      stored = value;
    },
    clear: () => {
      stored = null;
    },
  };
  const calls: Call[] = [];
  const transport: AccountTransport = {
    request: async (method, path, _token, body) => {
      calls.push({ method, path, body });
      if (path === "/v1/auth/challenges")
        return {
          status: 200,
          body: JSON.stringify({
            challenge_id: "challenge",
            expires_in: 600,
            authorization_url: "https://accounts.google.com/o/oauth2/v2/auth?state=s",
          }),
        };
      if (path === "/v1/auth/login")
        return {
          status: 200,
          body: JSON.stringify({
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer",
            expires_in: 3600,
            user: { id: "u1", display_name: "Test", created_at: "2026-01-01" },
          }),
        };
      return { status: 404, body: "{}" };
    },
  };
  return { bridge: new AccountCloudBridge(transport, store), calls, stored: () => stored };
}

async function main(): Promise<void> {
  console.log("GoogleLoopbackRequestHead");

  await group("a head is complete at the first blank line, however it is split", () => {
    const head = new GoogleLoopbackRequestHead(8192);
    check(!head.push(bytes("GET /callback?state=s&code=c HTTP/1.1\r\nHost: 127")), "still reading");
    check(head.head() === null, "nothing to reply with yet");
    check(head.push(bytes(".0.0.1\r\n\r")) === false, "a lone CR is not the end");
    check(head.push(bytes("\nignored")), "the blank line ends it");
    check(
      head.head() === "GET /callback?state=s&code=c HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
      "the head stops at the blank line",
    );
    check(head.push(bytes("more")), "anything after the head is not read");
  });

  await group("an oversized or non-ASCII head is answered as no head", () => {
    const oversized = new GoogleLoopbackRequestHead(16);
    check(oversized.push(bytes("GET /callback?state=long")), "past the limit it stops reading");
    check(oversized.head() === null, "and reports no head");
    const foreign = new GoogleLoopbackRequestHead(8192);
    check(
      foreign.push(new Uint8Array([0x47, 0x45, 0x54, 0x20, 0xe4])),
      "a non-ASCII byte stops it",
    );
    check(foreign.head() === null, "and reports no head");
  });

  console.log("AccountCloudBridge Google sign-in");

  await group("a Google challenge carries only a loopback callback target", async () => {
    const { bridge: account, calls } = bridge();
    const accepted = JSON.parse(
      await account.handle(
        JSON.stringify({
          operation: "request_code",
          provider: "google",
          target: "http://127.0.0.1:53682/callback",
        }),
      ),
    );
    check(accepted.ok === true, "the loopback target is sent");
    check(
      accepted.value.authorization_url.startsWith("https://accounts.google.com/"),
      "the authorization URL reaches the host",
    );
    check(
      JSON.stringify(calls[0].body) ===
        '{"provider":"google","target":"http://127.0.0.1:53682/callback","purpose":"login"}',
      "the challenge asks for a login",
    );
    for (const target of [
      "http://192.168.1.2:53682/callback",
      "https://127.0.0.1:53682/callback",
      "http://127.0.0.1:80/callback",
      "http://127.0.0.1:53682/other",
      "user@example.com",
    ]) {
      const refused = JSON.parse(
        await account.handle(
          JSON.stringify({ operation: "request_code", provider: "google", target }),
        ),
      );
      check(refused.error === "account_invalid", `${target} is refused locally`);
    }
    check(calls.length === 1, "a refused target never reaches the backend");
  });

  await group("google_login posts the authorization code and keeps the session", async () => {
    const { bridge: account, calls, stored } = bridge();
    const reply = JSON.parse(
      await account.handle(
        JSON.stringify({
          operation: "google_login",
          challenge_id: "challenge",
          code: "4/0Afixture",
        }),
      ),
    );
    check(reply.ok === true && reply.value.user.id === "u1", "the sign-in returns the user");
    check(stored() !== null, "the session is saved");
    check(
      calls[0].path === "/v1/auth/login" &&
        JSON.stringify(calls[0].body) === '{"challenge_id":"challenge","credential":"4/0Afixture"}',
      "the code is the login credential, as on the desktop",
    );
    for (const code of ["", "has space", "\u0000", "中", "c".repeat(2049)]) {
      const refused = JSON.parse(
        await account.handle(
          JSON.stringify({ operation: "google_login", challenge_id: "challenge", code }),
        ),
      );
      check(refused.error === "account_invalid", `${JSON.stringify(code).slice(0, 20)} is refused`);
    }
    check(calls.length === 1, "a refused code never reaches the backend");
    // 六位数字验证码的 login 不接受授权码，反过来也一样。
    const mixed = JSON.parse(
      await account.handle(
        JSON.stringify({
          operation: "login",
          challenge_id: "challenge",
          credential: "4/0Afixture",
        }),
      ),
    );
    check(mixed.error === "account_invalid", "the code login keeps its six-digit rule");
  });

  console.log("");
  if (failures > 0) {
    throw new Error(`${failures} group(s) failed`);
  }
  console.log(`all groups passed (${checks} assertions)`);
}

void main();
