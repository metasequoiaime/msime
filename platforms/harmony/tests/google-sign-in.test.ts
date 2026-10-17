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

  console.log("");
  if (failures > 0) {
    throw new Error(`${failures} group(s) failed`);
  }
  console.log(`all groups passed (${checks} assertions)`);
}

void main();
