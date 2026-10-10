import { releasesPageUrl } from "./app-resources";

export type Version = { display: string; parts: number[] };

/** What the settings page asks its host: the release tag prefix of this platform, the running version, and the edition id and architecture when the host reports them. */
export type UpdateCheckRequest = {
  platform: string;
  currentVersion: string;
  edition?: string;
  arch?: string;
};

/** A release as `msime_client_core::update_check::ReleaseUpdate` serialises it. */
export type HostReleaseUpdate = {
  version: { display: string; parts: number[] };
  release_url: string;
  installer_name: string | null;
  installer_sha256: string | null;
  signed: boolean | null;
};

/** What the host's update check found (`msime_client_core::update_check::UpdateCheck`): a newer release, the running version already being the newest, or no release of this platform yet. */
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

/** Normalises the version string a host reports (`v1.2.0`, `1.2.0-beta`) to the dotted numbers the page shows. */
export function parseVersion(value: string): Version | null {
  const match = value.trim().match(/^v?(\d+(?:\.\d+)*)(?:[-+].*)?$/i);
  if (!match?.[1]) return null;
  return { display: match[1], parts: match[1].split(".").map(Number) };
}

const sha256Pattern = /^[0-9a-f]{64}$/;
// The name is shown inside a shell command the user may copy; the host only reports names of this shape.
const installerNamePattern = /^[A-Za-z0-9][\w.+~-]*$/;

/**
 * The host's release in the page's shape, or null when it is not one the page can show safely: the release page must be a tag page of the shared repository, and an installer name or digest that does not look like one is dropped. The host's Rust check already guarantees all of this; the page does not take a URL it will open on trust.
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
