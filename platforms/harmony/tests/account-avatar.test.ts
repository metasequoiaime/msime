import { AccountAvatarPolicy } from "../entry/src/main/ets/account/AccountAvatarPolicy";

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

function filled(length: number, prefix: number[]): Uint8Array {
  const value = new Uint8Array(length);
  value.set(prefix);
  return value;
}

const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const jpeg = [0xff, 0xd8, 0xff, 0xe0];
const webp = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50];

function main(): void {
  console.log("account avatar policy");

  // 与 client-core `account_avatar_url_allowed` 相同的规则：HTTPS、无凭据和端口、只认头像存储桶与 Google 头像主机。
  group("only HTTPS avatars on the upload bucket or Google's picture host are fetched", () => {
    for (const url of [
      "https://lh3.googleusercontent.com/a/ACg8ocSynthetic=s96-c",
      "https://googleusercontent.com/a/synthetic",
      "https://media.msime.app/avatars/abc.jpg",
      "https://LH4.GoogleUserContent.com/a/synthetic",
    ]) {
      check(AccountAvatarPolicy.allowed(url), `${url} should be allowed`);
    }
    for (const url of [
      "http://lh3.googleusercontent.com/a/synthetic",
      "https://media.msime.app:8443/avatars/abc.jpg",
      "https://user:secret@media.msime.app/avatars/abc.jpg",
      "https://msime.app/avatars/abc.jpg",
      "https://evilgoogleusercontent.com/a/synthetic",
      "https://googleusercontent.com.example.test/a/synthetic",
      "https://example.test/?next=https://lh3.googleusercontent.com/a",
      "https://lh3.googleusercontent.com\\@example.test/a",
      "https:///avatars/abc.jpg",
      "",
    ]) {
      check(!AccountAvatarPolicy.allowed(url), `${url} should be refused`);
    }
  });

  group("the image type comes from the bytes, not the response", () => {
    check(AccountAvatarPolicy.sniff(filled(32, png)) === "image/png", "PNG");
    check(AccountAvatarPolicy.sniff(filled(32, jpeg)) === "image/jpeg", "JPEG");
    check(AccountAvatarPolicy.sniff(filled(32, webp)) === "image/webp", "WebP");
    check(
      AccountAvatarPolicy.sniff(filled(32, [0x47, 0x49, 0x46, 0x38])) === null,
      "GIF is not accepted",
    );
    check(
      AccountAvatarPolicy.sniff(new Uint8Array([0x89, 0x50])) === null,
      "a truncated header is not an image",
    );
  });

  group("only a 200 within the size bound is shown", () => {
    check(AccountAvatarPolicy.accept(200, filled(1024, jpeg)) === "image/jpeg", "a normal avatar");
    check(AccountAvatarPolicy.accept(404, filled(1024, jpeg)) === null, "a non-200 answer");
    check(AccountAvatarPolicy.accept(200, new Uint8Array(0)) === null, "an empty body");
    check(
      AccountAvatarPolicy.accept(200, filled(AccountAvatarPolicy.MAX_BYTES + 1, jpeg)) === null,
      "a body over the bound",
    );
  });

  console.log("");
  if (failures > 0) {
    throw new Error(`${failures} group(s) failed`);
  }
  console.log(`all groups passed (${checks} assertions)`);
}

main();
