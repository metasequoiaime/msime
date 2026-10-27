import {
  HUAWEI_ID_PERMISSIONS,
  HUAWEI_ID_SCOPES,
  huaweiChallenge,
  huaweiChallengeAction,
  huaweiIdConfigurationError,
  huaweiIdErrorCode,
  huaweiLoginAction,
  responseIdToken,
  stateMatches,
  validNonce,
} from "../entry/src/main/ets/account/HuaweiIdSignInPolicy";

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

const NONCE = "0123456789abcdef".repeat(4);

async function main(): Promise<void> {
  console.log("HuaweiIdSignInPolicy");

  await group("the authorization asks only for the user identifier and an ID token", () => {
    check(JSON.stringify(HUAWEI_ID_SCOPES) === '["openid"]', "only openid, no profile or phone");
    check(
      JSON.stringify(HUAWEI_ID_PERMISSIONS) === '["idtoken"]',
      "only the ID token; no authorization code means no client secret on the server",
    );
  });

  await group("the challenge reply gives the challenge and the nonce Account Kit accepts", () => {
    const value = huaweiChallenge(
      JSON.stringify({
        ok: true,
        value: { challengeId: "c".repeat(64), expiresIn: 300, nonce: NONCE },
      }),
    );
    check(value !== null && value.challengeId === "c".repeat(64), "challenge id read");
    check(
      value !== null && value.nonce === NONCE,
      "the server's hex nonce goes to the request as is",
    );
    for (const reply of [
      JSON.stringify({ ok: false, error: "account_unavailable" }),
      JSON.stringify({ ok: true, value: { challengeId: "c", expiresIn: 300 } }),
      JSON.stringify({ ok: true, value: { challengeId: "", expiresIn: 300, nonce: NONCE } }),
      JSON.stringify({ ok: true, value: { challengeId: "c", expiresIn: 300, nonce: "has space" } }),
      JSON.stringify({
        ok: true,
        value: { challengeId: "c", expiresIn: 300, nonce: "x".repeat(256) },
      }),
      JSON.stringify({ ok: true, value: null }),
      "not json",
    ]) {
      check(huaweiChallenge(reply) === null, `refused: ${reply.slice(0, 60)}`);
    }
  });

  await group("nonces follow the Account Kit pattern", () => {
    for (const nonce of [NONCE, "a", "A-Z_0.9:/x", "n".repeat(255)]) {
      check(validNonce(nonce), `${nonce.slice(0, 20)} is accepted`);
    }
    for (const nonce of ["", "n".repeat(256), "a b", "a+b", "a=b", "中文"]) {
      check(!validNonce(nonce), `${nonce.slice(0, 20)} is refused`);
    }
  });

  await group("the response must answer this request and carry an ID token", () => {
    check(stateMatches("s", "s"), "same state");
    check(stateMatches(undefined, undefined) && stateMatches("", undefined), "absent is empty");
    check(!stateMatches("s", "t") && !stateMatches("s", undefined), "a different state is refused");
    check(responseIdToken("a.b.c") === "a.b.c", "token kept");
    check(responseIdToken(undefined) === null && responseIdToken("") === null, "missing token");
    check(responseIdToken("x".repeat(2049)) === null, "longer than Account Kit documents");
  });

  await group("Account Kit errors become account codes the shared page understands", () => {
    check(
      huaweiIdErrorCode(1001502012) === "account_cancelled",
      "closing the page is a quiet cancel",
    );
    check(
      huaweiIdErrorCode(1001500002) === "account_cancelled",
      "a duplicate request is quiet too",
    );
    check(huaweiIdErrorCode(1001502005) === "account_network", "network is told apart");
    for (const code of [1001502001, 1001500001, 1001502014, 12300001, 1001502009, undefined]) {
      check(huaweiIdErrorCode(code) === "account_unavailable", `${code} is unavailable`);
    }
    check(
      huaweiIdConfigurationError(1001500001) && huaweiIdConfigurationError(1001502014),
      "fingerprint and permission errors point at the AppGallery Connect configuration",
    );
    check(!huaweiIdConfigurationError(1001502012), "a cancel is not a configuration problem");
  });

  await group("the two host actions are the Rust operation names", () => {
    check(huaweiChallengeAction() === '{"operation":"huawei_challenge"}', "challenge action");
    check(
      huaweiLoginAction("c", "a.b.c") ===
        '{"operation":"huawei_login","challenge_id":"c","id_token":"a.b.c"}',
      "login action",
    );
  });

  console.log(`${checks} checks, ${failures} failed`);
  if (failures > 0) {
    throw new Error(`${failures} huawei sign-in group(s) failed`);
  }
}

void main();
