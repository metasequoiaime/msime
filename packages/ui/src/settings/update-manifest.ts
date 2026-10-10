import { releasesPageUrl } from "./app-resources";

export type Version = { display: string; parts: number[] };

/** 设置页向宿主发出的请求：本平台的发布 tag 前缀、当前运行的版本号，以及宿主报告了时的版本 id 和架构。 */
export type UpdateCheckRequest = {
  platform: string;
  currentVersion: string;
  edition?: string;
  arch?: string;
};

/** `msime_client_core::update_check::ReleaseUpdate` 序列化后的发布。 */
export type HostReleaseUpdate = {
  version: { display: string; parts: number[] };
  release_url: string;
  installer_name: string | null;
  installer_sha256: string | null;
  signed: boolean | null;
};

/** 宿主检查更新的结果（`msime_client_core::update_check::UpdateCheck`）：有更新的发布、当前运行的已是最新版本，或者本平台还没有发布。 */
export type UpdateCheckResult =
  | { status: "available" | "current"; update: HostReleaseUpdate }
  | { status: "none" };

const projectReleaseTagUrl =
  /^https:\/\/github\.com\/metasequoiaime\/([\w.-]+)\/releases\/tag\/([\w.+-]+)$/;

/**
 * 国内镜像上这次更新的安装包：`<前缀>https://github.com/metasequoiaime/<仓库>/releases/download/<tag>/<安装包名>`。安装包名是检查更新时按平台、版本和架构挑出来并校验过形状的那一个，和页面上让用户核对的 SHA256 是同一个文件。发布页地址不是某个 tag，或者没有挑出安装包时返回 null，页面只给 GitHub 发布页。
 */
export function mirrorDownloadUrl(update: ValidatedUpdate, prefix: string): string | null {
  const tag = projectReleaseTagUrl.exec(update.releaseUrl);
  if (!tag || !update.installerName) return null;
  return `${prefix}https://github.com/metasequoiaime/${tag[1]}/releases/download/${tag[2]}/${update.installerName}`;
}

export type ValidatedUpdate = {
  version: Version;
  releaseUrl: string;
  installerName: string | null;
  installerSha256: string | null;
  signed: boolean | null;
};

/** 把宿主报告的版本字符串（`v1.2.0`、`1.2.0-beta`）规整为页面展示的点分数字。 */
export function parseVersion(value: string): Version | null {
  const match = value.trim().match(/^v?(\d+(?:\.\d+)*)(?:[-+].*)?$/i);
  if (!match?.[1]) return null;
  return { display: match[1], parts: match[1].split(".").map(Number) };
}

const sha256Pattern = /^[0-9a-f]{64}$/;
// 文件名会出现在用户可能复制的 shell 命令里；宿主只会报告这种形状的文件名。
const installerNamePattern = /^[A-Za-z0-9][\w.+~-]*$/;

/**
 * 把宿主给的发布转换成页面使用的形状；页面不能安全展示时返回 null：发布页必须是共用仓库的 tag 页面，不像安装包名或摘要的值会被丢弃。宿主的 Rust 检查已经保证了这些；页面对将要打开的 URL 不做无条件信任。
 */
export function fromHostUpdate(update: HostReleaseUpdate): ValidatedUpdate | null {
  if (!update || typeof update !== "object") return null;
  const { version, release_url: releaseUrl } = update;
  if (
    typeof releaseUrl !== "string" ||
    !releaseUrl.startsWith(`${releasesPageUrl}/tag/`) ||
    /[\s"'`<>\\|&]/.test(releaseUrl)
  )
    return null;
  const parsed =
    version && typeof version.display === "string" ? parseVersion(version.display) : null;
  if (!parsed) return null;
  return {
    version: parsed,
    releaseUrl,
    installerName:
      typeof update.installer_name === "string" && installerNamePattern.test(update.installer_name)
        ? update.installer_name
        : null,
    installerSha256:
      typeof update.installer_sha256 === "string" && sha256Pattern.test(update.installer_sha256)
        ? update.installer_sha256
        : null,
    signed: typeof update.signed === "boolean" ? update.signed : null,
  };
}

const editionIdPattern = /^[a-z][a-z0-9]*$/;

/** 版本的 Windows 安装包名前缀，例如 full（也是没有版本时）是 `MetasequoiaIME-Full_Setup_v`，五笔版是 `MetasequoiaIME-Wubi_Setup_v`；不是合法的版本 id 时为 null。与 `crates/client-core/src/update_check.rs` 的 `edition_installer_prefix` 相同。 */
function editionInstallerPrefix(edition: string | undefined): string | null {
  const id = edition ?? "full";
  if (!editionIdPattern.test(id)) return null;
  return `MetasequoiaIME-${id.charAt(0).toUpperCase()}${id.slice(1)}_Setup_v`;
}

export function describeInstallerTrust(
  update: ValidatedUpdate,
  platform: string | null,
  edition?: string,
): {
  warning: string | null;
  verify: { command: string; sha256: string } | null;
} {
  if (platform === "linux") {
    if (update.installerName && update.installerSha256) {
      return {
        warning: update.signed === false ? "该软件包未签名，请务必核对下面的校验值。" : null,
        verify: { command: `sha256sum ${update.installerName}`, sha256: update.installerSha256 },
      };
    }
    return {
      warning:
        update.signed === false
          ? "该软件包未签名。请从发行页一并下载 SHA256SUMS，放在软件包同一目录后运行 sha256sum -c SHA256SUMS --ignore-missing 核对。"
          : null,
      verify: null,
    };
  }
  const name =
    update.installerName ??
    `${editionInstallerPrefix(edition) ?? "MetasequoiaIME-Full_Setup_v"}<版本>.exe`;
  // The shipped settings page's wording: an unsigned installer is not only a SmartScreen prompt, it also loses uiAccess, so the candidate window cannot float above elevated programs.
  const unsigned =
    "该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。";
  if (update.signed === false && !update.installerSha256)
    return {
      warning: `${unsigned}请从发行页一并下载 ${name}.sha256，用 Get-FileHash .\\${name} -Algorithm SHA256 核对。`,
      verify: null,
    };
  return {
    warning: update.signed === false ? `${unsigned}请务必核对下面的校验值。` : null,
    verify: update.installerSha256
      ? { command: `Get-FileHash .\\${name} -Algorithm SHA256`, sha256: update.installerSha256 }
      : null,
  };
}
