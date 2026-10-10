import { expect, test } from "vitest";
import {
  describeInstallerTrust,
  fromHostUpdate,
  mirrorDownloadUrl,
  type HostReleaseUpdate,
  type ValidatedUpdate,
} from "../../../../packages/ui/src/settings/update-manifest";

// 哪个发布、哪个安装包属于某个平台、版本和架构，由 Rust 决定（`crates/client-core/src/update_check.rs` 及其测试）；这里覆盖的是页面拿到答复后怎么做。

const page = "https://github.com/metasequoiaime/msime/releases";

function hostUpdate(overrides: Partial<HostReleaseUpdate> = {}): HostReleaseUpdate {
  return {
    version: { display: "1.2.0", parts: [1, 2, 0] },
    release_url: `${page}/tag/windows-v1.2.0`,
    installer_name: "MetasequoiaIME-Full_Setup_v1.2.0.exe",
    installer_sha256: "a".repeat(64),
    signed: false,
    ...overrides,
  };
}

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

// 国内镜像给的是检查更新挑中的那个安装包，地址是镜像前缀加它在 GitHub 上的下载地址。
test("the mirror link points at the chosen installer under its GitHub download URL", () => {
  const mirror = "https://dl.msime.app/gh/";
  const windows = fromHostUpdate(hostUpdate());
  expect(windows && mirrorDownloadUrl(windows, mirror)).toBe(
    "https://dl.msime.app/gh/https://github.com/metasequoiaime/msime/releases/download/windows-v1.2.0/MetasequoiaIME-Full_Setup_v1.2.0.exe",
  );
  const wubi = fromHostUpdate(
    hostUpdate({
      release_url: `${page}/tag/linux-v1.2.0`,
      installer_name: "msime-linux-wubi_1.2.0_amd64.deb",
    }),
  );
  expect(wubi && mirrorDownloadUrl(wubi, mirror)).toBe(
    "https://dl.msime.app/gh/https://github.com/metasequoiaime/msime/releases/download/linux-v1.2.0/msime-linux-wubi_1.2.0_amd64.deb",
  );
  // 没挑出安装包，或者发布页不是某个 tag：只给 GitHub 发布页。
  const noInstaller = fromHostUpdate(hostUpdate({ installer_name: null }));
  expect(noInstaller && mirrorDownloadUrl(noInstaller, mirror)).toBeNull();
  const pageOnly: ValidatedUpdate = {
    version: { display: "1.2.0", parts: [1, 2, 0] },
    releaseUrl: page,
    installerName: "MetasequoiaIME-Full_Setup_v1.2.0.exe",
    installerSha256: null,
    signed: false,
  };
  expect(mirrorDownloadUrl(pageOnly, mirror)).toBeNull();
});

test("the host's release becomes the page's update", () => {
  expect(fromHostUpdate(hostUpdate())).toEqual({
    version: { display: "1.2.0", parts: [1, 2, 0] },
    releaseUrl: `${page}/tag/windows-v1.2.0`,
    installerName: "MetasequoiaIME-Full_Setup_v1.2.0.exe",
    installerSha256: "a".repeat(64),
    signed: false,
  });
  expect(
    fromHostUpdate(hostUpdate({ installer_name: null, installer_sha256: null, signed: null })),
  ).toMatchObject({ installerName: null, installerSha256: null, signed: null });
});

test("a release the page would open or show unsafely is refused or trimmed", () => {
  // 只会打开共用仓库的 tag 页面。
  for (const release_url of [
    "https://github.com/metasequoiaime/MSIME-Windows/releases/tag/v1.2.0",
    page,
    `${page}/tag/windows-v1.2.0"onclick`,
    "http://github.com/metasequoiaime/msime/releases/tag/windows-v1.2.0",
  ]) {
    expect(fromHostUpdate(hostUpdate({ release_url }))).toBeNull();
  }
  expect(fromHostUpdate(hostUpdate({ version: { display: "next", parts: [] } }))).toBeNull();
  // 需要 shell 引号的文件名或格式不对的摘要会被丢弃，不会出现在可复制的命令里。
  expect(
    fromHostUpdate(
      hostUpdate({ installer_name: "--x;rm -rf ~.exe", installer_sha256: "A".repeat(64) }),
    ),
  ).toMatchObject({ installerName: null, installerSha256: null });
});
