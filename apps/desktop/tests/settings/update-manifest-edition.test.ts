import { expect, test } from "vitest";
import {
  describeInstallerTrust,
  selectPlatformRelease,
} from "../../../../packages/ui/src/settings/update-manifest";

const page = "https://github.com/metasequoiaime/msime/releases";
const digest = (character: string) => `sha256:${character.repeat(64)}`;

// 各版本发布在同一个平台标签下，只靠资产名区分。
const linuxRelease = [
  {
    tag_name: "linux-v1.2.0",
    html_url: `${page}/tag/linux-v1.2.0`,
    assets: [
      { name: "msime-linux_1.2.0_amd64.deb", digest: digest("a") },
      { name: "msime-linux-1.2.0-linux-x86_64.tar.gz", digest: digest("b") },
      { name: "msime-linux-wubi_1.2.0_amd64.deb", digest: digest("c") },
      { name: "msime-linux-wubi-1.2.0-linux-x86_64.tar.gz", digest: digest("d") },
      { name: "msime-linux-pinyin_1.2.0_amd64.deb", digest: digest("e") },
      { name: "SHA256SUMS", digest: digest("f") },
    ],
  },
];

const windowsRelease = [
  {
    tag_name: "windows-v1.2.0",
    html_url: `${page}/tag/windows-v1.2.0`,
    assets: [
      { name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: digest("a") },
      { name: "MetasequoiaIME_Setup_v1.2.0.exe.sha256", digest: digest("b") },
      { name: "MetasequoiaIME-Wubi_Setup_v1.2.0.exe", digest: digest("c") },
      { name: "MetasequoiaIME-Pinyin_Setup_v1.2.0.exe", digest: digest("d") },
    ],
  },
];

function pick(
  releases: Parameters<typeof selectPlatformRelease>[0],
  platform: string,
  edition?: string,
) {
  const update = selectPlatformRelease(releases, platform, page, edition);
  return update && { name: update.installerName, sha256: update.installerSha256 };
}

test("full picks its own Linux package beside the other editions' packages", () => {
  for (const edition of [undefined, "full"]) {
    expect(pick(linuxRelease, "linux", edition)).toEqual({
      name: "msime-linux_1.2.0_amd64.deb",
      sha256: "a".repeat(64),
    });
  }
  // 没有 .deb 时退回 full 自己的压缩包，不会拿到五笔版的。
  expect(
    pick(
      [
        {
          ...linuxRelease[0]!,
          assets: linuxRelease[0]!.assets.filter((a) => !a.name.endsWith(".deb")),
        },
      ],
      "linux",
    ),
  ).toEqual({ name: "msime-linux-1.2.0-linux-x86_64.tar.gz", sha256: "b".repeat(64) });
});

test("an edition picks only its own package and installer", () => {
  expect(pick(linuxRelease, "linux", "wubi")).toEqual({
    name: "msime-linux-wubi_1.2.0_amd64.deb",
    sha256: "c".repeat(64),
  });
  expect(pick(linuxRelease, "linux", "pinyin")).toEqual({
    name: "msime-linux-pinyin_1.2.0_amd64.deb",
    sha256: "e".repeat(64),
  });
  expect(pick(windowsRelease, "windows")).toEqual({
    name: "MetasequoiaIME_Setup_v1.2.0.exe",
    sha256: "a".repeat(64),
  });
  expect(pick(windowsRelease, "windows", "wubi")).toEqual({
    name: "MetasequoiaIME-Wubi_Setup_v1.2.0.exe",
    sha256: "c".repeat(64),
  });
  // 发布里还没有这个版本的资产：版本照常提示，只是没有安装包可核对。
  expect(pick(windowsRelease, "windows", "cantonese")).toEqual({ name: null, sha256: null });
  // 不像版本 id 的取值不会拼进正则，也不会选中任何资产。
  expect(pick(windowsRelease, "windows", "wubi|.*")).toEqual({ name: null, sha256: null });
});

test("the installer placeholder names the edition's installer", () => {
  const update = {
    version: { display: "1.2.0", parts: [1, 2, 0] },
    releaseUrl: `${page}/tag/windows-v1.2.0`,
    installerName: null,
    installerSha256: null,
    signed: false,
  };
  expect(describeInstallerTrust(update, "windows").warning).toContain(
    "MetasequoiaIME_Setup_v<版本>.exe.sha256",
  );
  expect(describeInstallerTrust(update, "windows", "wubi").warning).toContain(
    "MetasequoiaIME-Wubi_Setup_v<版本>.exe.sha256",
  );
});
