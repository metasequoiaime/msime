import { errorCode } from "../core/error-code";
import type { CommunityResourceKind, CommunityResourceScope } from "./community-resources";

export type Identified = { id: string };

export function resourceMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "community_invalid":
      return "内容无效，请修改后重试。";
    case "community_unauthorized":
      return "请先登录后执行此操作。";
    case "community_forbidden":
      return "没有权限执行此操作。";
    case "community_conflict":
      return "作品状态已变化或已达到发布上限，请刷新后重试。";
    case "community_not_found":
      return "作品不存在或已下架。";
    case "community_rate_limited":
      return "请求过于频繁，请稍后再试。";
    case "community_cancelled":
      return "账号状态已变化，请重新加载。";
    case "community_storage":
      return "无法安全保存本地模板，请稍后重试。";
    case "community_resource_library_format":
      return "本地模板库无法读取，请检查后重试。";
  }
  return "社区暂时不可用，请稍后重试。";
}

export function resourceKindTitle(kind: CommunityResourceKind): string {
  return kind === "dictionary" ? "词库" : "回复";
}

export function resourceScopeTitle(scope: CommunityResourceScope): string {
  return scope === "mine" ? "我的作品" : scope === "saved" ? "收藏" : "全部";
}

export function communitySkinMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "community_invalid":
      return "搜索内容无效，请修改后重试。";
    case "community_unauthorized":
      return "登录已失效；仍可退出后匿名浏览。";
    case "community_forbidden":
      return "没有权限执行此操作；自己的作品不能评分或下架。";
    case "community_conflict":
      return "作品状态已变化或已达到发布上限，请刷新后重试。";
    case "community_not_found":
      return "作品不存在或已下架。";
    case "community_rate_limited":
      return "请求过于频繁，请稍后再试。";
    case "community_cancelled":
      return "账号状态已变化，请重新加载。";
    case "community_storage":
      return "无法安全读取登录状态，请检查设备安全设置。";
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

export function communityNeedsSignIn(error: unknown): boolean {
  return errorCode(error) === "community_unauthorized";
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
