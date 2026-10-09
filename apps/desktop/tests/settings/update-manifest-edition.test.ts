import { expect, test } from "vitest";
import {
  describeInstallerTrust,
  mirrorDownloadUrl,
  selectPlatformRelease,
  validateManifest,
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
      // glibc 2.28 系统用的 legacy 包（#6311）：包名同样是 msime-linux，附件按 msime-linux-legacy_ 命名，full 和各版本都不能选中它。
      { name: "msime-linux-legacy_1.2.0_amd64.deb", digest: digest("9") },
      { name: "SHA256SUMS", digest: digest("f") },
    ],
  },
];

const windowsRelease = [
  {
    tag_name: "windows-v1.2.0",
    html_url: `${page}/tag/windows-v1.2.0`,
    assets: [
      { name: "MetasequoiaIME-Full_Setup_v1.2.0.exe", digest: digest("a") },
      { name: "MetasequoiaIME-Full_Setup_v1.2.0.exe.sha256", digest: digest("b") },
      // msime-windows 的安装包名。本仓库的发布里不会有它，放在这里确认 full 不会把它当成自己的。
      { name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: digest("e") },
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
  for (const edition of [undefined, "full"]) {
    expect(pick(windowsRelease, "windows", edition)).toEqual({
      name: "MetasequoiaIME-Full_Setup_v1.2.0.exe",
      sha256: "a".repeat(64),
    });
  }
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
    "MetasequoiaIME-Full_Setup_v<版本>.exe.sha256",
  );
  expect(describeInstallerTrust(update, "windows", "wubi").warning).toContain(
    "MetasequoiaIME-Wubi_Setup_v<版本>.exe.sha256",
  );
});

test("an edition picks its own package for the host's architecture", () => {
  const bothArchitectures = [
    {
      ...linuxRelease[0]!,
      assets: [
        ...linuxRelease[0]!.assets,
        { name: "msime-linux_1.2.0_arm64.deb", digest: digest("1") },
        { name: "msime-linux-wubi_1.2.0_arm64.deb", digest: digest("2") },
        { name: "msime-linux-wubi-1.2.0-linux-aarch64.tar.gz", digest: digest("3") },
        { name: "msime-linux-legacy_1.2.0_arm64.deb", digest: digest("8") },
      ],
    },
  ];
  const pickFor = (edition: string | undefined, arch: string) => {
    const update = selectPlatformRelease(bothArchitectures, "linux", page, edition, arch);
    return update && { name: update.installerName, sha256: update.installerSha256 };
  };
  expect(pickFor(undefined, "aarch64")).toEqual({
    name: "msime-linux_1.2.0_arm64.deb",
    sha256: "1".repeat(64),
  });
  expect(pickFor("wubi", "aarch64")).toEqual({
    name: "msime-linux-wubi_1.2.0_arm64.deb",
    sha256: "2".repeat(64),
  });
  expect(pickFor("wubi", "x86_64")).toEqual({
    name: "msime-linux-wubi_1.2.0_amd64.deb",
    sha256: "c".repeat(64),
  });
  // pinyin has no aarch64 package in this release, so an aarch64 host is offered none of the x86_64 ones.
  expect(pickFor("pinyin", "aarch64")).toEqual({ name: null, sha256: null });
});

// 国内镜像给的是检查更新挑中的那个安装包，地址是镜像前缀加它在 GitHub 上的下载地址。
test("the mirror link points at the chosen installer under its GitHub download URL", () => {
  const mirror = "https://dl.msime.app/gh/";
  const windows = selectPlatformRelease(windowsRelease, "windows", page, "full");
  expect(windows && mirrorDownloadUrl(windows, mirror)).toBe(
    "https://dl.msime.app/gh/https://github.com/metasequoiaime/msime/releases/download/windows-v1.2.0/MetasequoiaIME-Full_Setup_v1.2.0.exe",
  );
  const wubi = selectPlatformRelease(linuxRelease, "linux", page, "wubi");
  expect(wubi && mirrorDownloadUrl(wubi, mirror)).toBe(
    "https://dl.msime.app/gh/https://github.com/metasequoiaime/msime/releases/download/linux-v1.2.0/msime-linux-wubi_1.2.0_amd64.deb",
  );
  // update.json 一路（msime-windows 的安装包）同样适用。
  const legacyPage = "https://github.com/metasequoiaime/MSIME-Windows/releases";
  const legacy = validateManifest(
    {
      version: "0.9.5",
      releaseUrl: `${legacyPage}/tag/v0.9.5`,
      installerName: "MetasequoiaIME_Setup_v0.9.5.exe",
      installerSha256: "a".repeat(64),
    },
    legacyPage,
  );
  expect(legacy && mirrorDownloadUrl(legacy, mirror)).toBe(
    "https://dl.msime.app/gh/https://github.com/metasequoiaime/MSIME-Windows/releases/download/v0.9.5/MetasequoiaIME_Setup_v0.9.5.exe",
  );
  // 没挑出安装包，或者发布页不是某个 tag：只给 GitHub 发布页。
  const noInstaller = validateManifest(
    { version: "0.9.5", releaseUrl: `${legacyPage}/tag/v0.9.5` },
    legacyPage,
  );
  expect(noInstaller && mirrorDownloadUrl(noInstaller, mirror)).toBeNull();
  const pageOnly = validateManifest(
    { version: "0.9.5", releaseUrl: legacyPage, installerName: "MetasequoiaIME_Setup_v0.9.5.exe" },
    legacyPage,
  );
  expect(pageOnly && mirrorDownloadUrl(pageOnly, mirror)).toBeNull();
});
