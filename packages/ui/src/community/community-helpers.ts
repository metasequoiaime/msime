import { errorCode } from "../core/error-code";
import { runAsyncAction } from "../core/async-action";
import { pluginErrorMessage } from "../settings/plugins-section";
import type { CommunityResourceKind, CommunityResourceScope } from "./community-resource-model";

export type Identified = { id: string };

/** The sentences for the server's refusals the user has to act on, the same on every community page and on every host: the content screening rejected the text (`422 blocked_content`, the service itself is up), the screening could not run (`503 screening_unavailable`) or the account is banned (`403 account_banned`). */
export function communityModerationMessage(error: unknown): string | undefined {
  switch (errorCode(error)) {
    case "community_blocked_content":
    case "account_blocked_content":
      return "内容包含不允许发布的词语，请修改后再提交";
    case "community_screening_unavailable":
    case "account_screening_unavailable":
      return "审核服务暂时不可用，请稍后重试";
    case "community_account_banned":
    case "account_banned":
      return "该账号已被封禁，暂时无法使用账号相关功能";
  }
  return undefined;
}

type CommunityStatusCode =
  | "community_forbidden"
  | "community_unauthorized"
  | "community_not_found"
  | "community_cancelled"
  | "community_storage"
  | "community_conflict"
  | "community_rate_limited"
  | "community_invalid";

const communityStatusMessages: Record<CommunityStatusCode, string> = {
  community_forbidden: "没有权限执行此操作。",
  community_unauthorized: "请先登录后执行此操作。",
  community_not_found: "作品不存在或已下架。",
  community_cancelled: "账号状态已变化，请重新加载。",
  community_storage: "无法安全读取登录状态，请检查设备安全设置。",
  community_conflict: "作品状态已变化或已达到发布上限，请刷新后重试。",
  community_rate_limited: "请求过于频繁，请稍后再试。",
  community_invalid: "内容无效，请修改后重试。",
};

function communityStatusMessage(
  error: unknown,
  overrides: Partial<Record<CommunityStatusCode, string>> = {},
): string | undefined {
  const code = errorCode(error);
  if (!code || !Object.prototype.hasOwnProperty.call(communityStatusMessages, code))
    return undefined;
  const statusCode = code as CommunityStatusCode;
  return overrides[statusCode] ?? communityStatusMessages[statusCode];
}

export function resourceMessage(error: unknown): string {
  const moderation = communityModerationMessage(error);
  const status = communityStatusMessage(error, {
    community_storage: "无法安全保存本地模板，请稍后重试。",
  });
  if (moderation) return moderation;
  if (status) return status;
  switch (errorCode(error)) {
    case "community_resource_library_format":
      return "本地模板库无法读取，请检查后重试。";
  }
  return "社区暂时不可用，请稍后重试。";
}

export function resourceKindTitle(kind: CommunityResourceKind): string {
  return kind === "dictionary" ? "词库" : "回复模板";
}

export function resourceScopeTitle(scope: CommunityResourceScope): string {
  return scope === "mine" ? "我的作品" : scope === "saved" ? "收藏" : "全部";
}

export function communitySkinMessage(error: unknown): string {
  const moderation = communityModerationMessage(error);
  const status = communityStatusMessage(error, {
    community_unauthorized: "登录已失效；仍可退出后匿名浏览。",
    community_forbidden: "没有权限执行此操作；自己的作品不能评分或下架。",
    community_invalid: "搜索内容无效，请修改后重试。",
  });
  if (moderation) return moderation;
  if (status) return status;
  switch (errorCode(error)) {
    case "community_skin_library_full":
      return "最多保存 12 套皮肤，请先删除不需要的设计。";
    case "community_skin_invalid_name":
      return "无法保存这款皮肤：名称无效。";
    case "community_skin_duplicate_name":
      return "无法保存这款皮肤：名称重复。";
    case "community_trial_format":
      return "无法安全保存试用状态，请稍后重试。";
  }
  return "社区暂时不可用，请稍后重试。";
}

export function communitySkinPublishMessage(error: unknown): string {
  const moderation = communityModerationMessage(error);
  if (moderation) return moderation;
  switch (errorCode(error)) {
    case "community_unauthorized":
      return "请先登录后再发布皮肤。";
    case "community_forbidden":
      return "当前账号没有权限执行发布操作。";
    case "community_conflict":
      return "作品状态已变化或已达到发布上限，请刷新后重试。";
    case "community_invalid":
      return "名称、说明或皮肤设计不符合发布要求。";
    case "community_rate_limited":
      return "发布操作过于频繁，请稍后再试。";
    case "community_not_found":
      return "账号或作品不存在，请重新加载。";
    case "community_cancelled":
      return "账号状态已变化，请重新登录后重试。";
  }
  return "暂时无法发布皮肤，请稍后重试。";
}

/**
 * Fixed sentences for the candidate-skin gallery and its publish dialog. The host maps every server rejection to an HTTP-status code (`community_invalid`, `community_conflict`, …), so the specific wording comes from the local package codes the host checks before anything is uploaded; backend text is never shown. `publishing` picks the sentence for a `community_invalid` publish, where the server has rejected something only it can check.
 */
export function candidateSkinMessage(error: unknown, publishing = false): string {
  const moderation = communityModerationMessage(error);
  const status = communityStatusMessage(error, {
    community_forbidden: "下载后才能评分，且不能给自己的作品评分。",
    community_conflict: "发布信息已变更，或皮肤库已达到数量上限。",
    community_rate_limited: "发布太频繁，请稍后再试。",
    ...(publishing && {
      community_invalid:
        "服务器未接受这款皮肤：请确认图片每边不超过 2048 像素、能被正常解码，且皮肤 ID 不与内置主题重名。",
    }),
  });
  if (moderation) return moderation;
  if (status) return status;
  switch (errorCode(error)) {
    case "candidate_skin_package":
      return "皮肤包未通过校验，无法分享或安装。";
    case "candidate_skin_file_type":
      return "资源支持 PNG/JPEG/APNG/GIF/WebP，预览须为 PNG/JPEG，不能包含样式表。";
    case "candidate_skin_file_path":
      return "皮肤文件名或数量不符合要求。";
    case "candidate_skin_too_large":
      return "单个图片不超过 1 MB、合计不超过 2 MB，预览图不超过 256 KB。";
    case "candidate_skin_image_invalid":
      return "图片已损坏或格式与扩展名不符。";
    case "candidate_skin_license_required":
      return "发布前请在 skin.toml 的 [license] 中填写素材授权 assets。";
    case "candidate_skin_preview_required":
      return "请在 skin.toml 中用 preview 指定一张 PNG/JPEG 预览图。";
    case "candidate_skin_exists":
      return "已存在同名皮肤，安装会整体替换它。";
    case "storage":
      return "无法写入皮肤目录。";
  }
  return "社区暂时不可用，请稍后重试。";
}

/**
 * Fixed sentences for the plugin gallery and its publish dialog. Pack failures come back with client-services' `plugin_*` codes, which read as they do on the 插件 page, and the community ones with the host's HTTP-status `community_*` codes; backend text is never shown. `publishing` picks the sentence for a `community_invalid` or `community_conflict` publish, where the server has rejected something only it can check.
 */
export function communityPluginMessage(error: unknown, publishing = false): string {
  const moderation = communityModerationMessage(error);
  const status = communityStatusMessage(error, {
    community_forbidden: "下载后才能评分，且不能给自己的作品评分。",
    community_conflict: publishing
      ? "发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。"
      : "作品状态已变化，请刷新后重试。",
    community_rate_limited: publishing
      ? "每小时最多发布 10 次，请稍后再试。"
      : communityStatusMessages.community_rate_limited,
    ...(publishing && {
      community_invalid: "服务器未接受这个插件：请确认名称、说明和包内容符合发布要求。",
    }),
  });
  if (moderation) return moderation;
  if (status) return status;
  switch (errorCode(error)) {
    case "plugin_community_too_large":
      return "插件压缩后不能超过 8 MB。";
    case "plugin_community_checksum":
      return "下载的插件已损坏，请重试。";
    case "plugin_community_mismatch":
      return "下载的插件与作品信息不符，已停止安装。";
    case "storage":
      return "无法读写插件目录，请检查数据目录的权限。";
  }
  return pluginErrorMessage(error, "社区暂时不可用，请稍后重试。");
}

/** A package size in megabytes with one decimal, as the publish dialog states it against the 2 MB limit. */
export function candidateSkinMegabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Formats the optional license fields shown for candidate-skin packages. */
export function communityLicenseLine(license: {
  assets: string | null;
  code: string | null;
  source: string | null;
}): string {
  return [
    license.assets?.trim() ? `素材授权 ${license.assets.trim()}` : "",
    license.code?.trim() ? `代码授权 ${license.code.trim()}` : "",
    license.source?.trim() ? `来源 ${license.source.trim()}` : "",
  ]
    .filter(Boolean)
    .join(" / ");
}

export function communityNeedsSignIn(error: unknown): boolean {
  return errorCode(error) === "community_unauthorized";
}

/** Creates the shared publication-login action, closing the dialog before navigation. */
export function communityPublishLoginAction(
  onClose: () => void,
  onLogin?: () => void,
): (() => void) | undefined {
  if (!onLogin) return undefined;
  return () => {
    onClose();
    onLogin();
  };
}

type CurrentGeneration = { current: number };
type RunningAction = { current: boolean };

export interface CommunityActionOptions {
  busy: boolean;
  generation: number;
  clientGeneration: CurrentGeneration;
  actionRunning: RunningAction;
  setBusy: (busy: boolean) => void;
  setError: (message: string) => void;
  setNotice?: (message: string) => void;
  setSignInRequired?: (value: boolean) => void;
  formatError: (error: unknown) => string;
  isCurrent?: () => boolean;
  operation: (isCurrent: () => boolean) => Promise<void>;
}

/** Runs a guarded community action with shared busy, generation, and error handling. */
export async function runCommunityAction({
  busy,
  generation,
  clientGeneration,
  actionRunning,
  setBusy,
  setError,
  setNotice,
  setSignInRequired,
  formatError,
  isCurrent,
  operation,
}: CommunityActionOptions): Promise<void> {
  actionRunning.current = true;
  setSignInRequired?.(false);
  const current = isCurrent ?? (() => generation === clientGeneration.current);
  try {
    await runAsyncAction(
      {
        busy,
        isCurrent: current,
        setBusy,
        setError,
        setNotice,
      },
      operation,
      {
        formatError,
        onError: (error) => setSignInRequired?.(communityNeedsSignIn(error)),
      },
    );
  } finally {
    if (generation === clientGeneration.current) actionRunning.current = false;
  }
}

export type CommunityPublishActionOptions = CommunityActionOptions & {
  setSignInRequired: (value: boolean) => void;
};

/** Runs a guarded community publish action with sign-in handling. */
export function runCommunityPublishAction(options: CommunityPublishActionOptions): Promise<void> {
  return runCommunityAction(options);
}

/** Append only items whose ids are not already present, preserving source order. */
export function appendUniqueById<T extends Identified>(current: T[], incoming: T[]): T[] {
  const ids = new Set(current.map((item) => item.id));
  const additions = incoming.filter((item) => {
    if (ids.has(item.id)) return false;
    ids.add(item.id);
    return true;
  });
  return [...current, ...additions];
}

export function communityRating(count: number, average: number): string {
  return count === 0 ? "暂无评分" : `${average.toFixed(1)} 分`;
}
