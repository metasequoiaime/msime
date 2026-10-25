import { releasesPageUrl } from "./app-resources";

export type Version = { display: string; parts: number[] };

/** 桌面应用检查更新跟的发布通道（偏好 `update_channel`，见 `crates/client-core/src/preferences.rs` 的 `UpdateChannel`）：稳定版只认正式发布，测试版另外接受 develop 发出的 `-beta` 预发布，每日构建再接受滚动的 `nightly` 发布。 */
export type UpdateChannel = "stable" | "beta" | "nightly";

/** 设置页向宿主发出的请求：本平台的发布 tag 前缀、当前运行的版本号，以及宿主报告了时的版本 id 和架构；提供通道选择的宿主（macOS、Linux）还带上偏好里的通道，不带时宿主按稳定版检查。 */
export type UpdateCheckRequest = {
  platform: string;
  currentVersion: string;
  edition?: string;
  arch?: string;
  channel?: UpdateChannel;
};

/** `msime_client_services::update_check::ReleaseUpdate` 序列化后的发布。 */
export type HostReleaseUpdate = {
  version: { display: string; parts: number[] };
  release_url: string;
  installer_name: string | null;
  installer_sha256: string | null;
  signed: boolean | null;
  /** 这个发布属于哪个通道；以前的宿主不报这一项，按稳定版处理。 */
  channel?: UpdateChannel;
  /** 构建它的发布 workflow 运行 ID，每日构建在页面上显示为「构建 N」；没有标记的旧发布为 null。 */
  build_run?: number | null;
};

/** 宿主检查更新的结果（`msime_client_services::update_check::UpdateCheck`）：有更新的发布、当前运行的已是最新版本，或者本平台还没有发布。 */
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
  channel: UpdateChannel;
  buildRun: number | null;
};

const updateChannels: readonly UpdateChannel[] = ["stable", "beta", "nightly"];

/** 偏好或宿主给出的通道名；不认识的值（以后的版本加的通道、手改的文件）按稳定版，与 client-core 读偏好文档的规则一致。 */
export function updateChannelOf(value: unknown): UpdateChannel {
  return updateChannels.find((channel) => channel === value) ?? "stable";
}

/** 页面上称呼一个发布的方式：稳定版只写版本号，测试版写「测试版 v1.2.0」，每日构建写「每日构建 v1.2.0（构建 123）」。同一个版本号每天都有新的每日构建，只有带上构建号用户才分得清是不是又出了新的。 */
export function describeUpdateVersion(update: ValidatedUpdate): string {
  const version = `v${update.version.display}`;
  if (update.channel === "beta") return `测试版 ${version}`;
  if (update.channel === "nightly")
    return update.buildRun === null
      ? `每日构建 ${version}`
      : `每日构建 ${version}（构建 ${update.buildRun}）`;
  return version;
}

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
    channel: updateChannelOf(update.channel),
    buildRun:
      typeof update.build_run === "number" &&
      Number.isSafeInteger(update.build_run) &&
      update.build_run >= 0
        ? update.build_run
        : null,
  };
}

const editionIdPattern = /^[a-z][a-z0-9]*$/;

/** 版本的 Windows 安装包名前缀，例如 full（也是没有版本时）是 `MetasequoiaIME-Full_Setup_v`，五笔版是 `MetasequoiaIME-Wubi_Setup_v`；不是合法的版本 id 时为 null。与 `crates/client-services/src/update_check.rs` 的 `edition_installer_prefix` 相同。 */
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
