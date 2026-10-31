/**
 * 社区投稿类功能的总闸，设置桥一侧。
 *
 * 投稿类功能指把用户的内容公开到社区、或对他人作品公开表态：发布键盘皮肤、词包、短语、回复模板和插件（含发布前的打包预检），修改已发布作品的分类，给他人作品评分，以及举报。华为应用市场（中国大陆）要求「信息发布」类服务先做实名认证，鸿蒙版的邮箱和华为账号登录都不核验手机号，所以实名认证到位之前鸿蒙版不提供它们，见 `.agents/notes/implemented/feature/2026-10-28-harmony-withhold-community-contribution.md`。浏览、搜索、详情、获取和安装、收藏、导入、下架或删除自己的作品不归这里管；屏蔽作者也不归这里管：它不公开任何内容，只让本账号看不到那位作者的作品，走的是三类之外的 `community_block` 请求。
 *
 * 开关本身不在这里：它是 Rust 的 `HostCapabilities.community_contribution`（`crates/client-core/src/host_surface.rs`），设置页的 React 一侧和 `apps/harmony` 的社区客户端读同一个字段。设置进程启动时把 `hostCapabilities` 的回答交给 `adopt`；实名认证到位后只改 Rust 那一行。键盘进程不经社区接口联网，不用这道闸。
 *
 * 在 `adopt` 之前、或回答读不懂时按关闭处理：宁可少一个入口，也不在实名认证没到位时把内容发布出去。
 */
export class CommunityContributionGate {
  /** 设置桥上的三类社区请求。 */
  static readonly COMMUNITY_KINDS: readonly string[] = [
    "community_skin",
    "community_resource",
    "community_plugin",
  ];
  /** 这三类请求里属于投稿的操作，名字与 `msime_client_community` 相同。 */
  static readonly CONTRIBUTION_OPERATIONS: readonly string[] = [
    "keyboard_skin_publish",
    "keyboard_skin_rate",
    "keyboard_skin_set_category",
    "resource_publish",
    "resource_rate",
    "plugin_pack_preview",
    "plugin_publish",
    "plugin_rate",
    "report",
  ];
  /** 设置桥拒绝时给页面的错误码，与 `apps/harmony` 页面一侧的拒绝相同。 */
  static readonly REFUSAL: string = "community_contribution_unavailable";

  private static open: boolean = false;

  /** `hostCapabilities` 的回答（`{ok, value}` 信封）里是否打开了社区投稿类功能。只有明确为 true 才算打开。 */
  static fromCapabilities(document: string): boolean {
    try {
      const reply: Record<string, Object> = JSON.parse(document) as Record<string, Object>;
      if (reply === null || typeof reply !== "object" || reply["ok"] !== true) return false;
      const value: Object | undefined = reply["value"];
      if (value === null || typeof value !== "object") return false;
      return (value as Record<string, Object>)["community_contribution"] === true;
    } catch {
      return false;
    }
  }

  /** 进程启动时读一次宿主能力；返回采用后的结果。 */
  static adopt(document: string): boolean {
    CommunityContributionGate.open = CommunityContributionGate.fromCapabilities(document);
    return CommunityContributionGate.open;
  }

  /** 设置桥是否放行这次请求：总闸关着时拒绝三类社区请求里的发布、打包预检、改分类、评分和举报，读不出操作名的社区请求也拒绝；其余照常。 */
  static allowsSettingsRequest(kind: string, payload: string): boolean {
    if (CommunityContributionGate.open) return true;
    if (!CommunityContributionGate.COMMUNITY_KINDS.includes(kind)) return true;
    const operation: string | null = CommunityContributionGate.operation(payload);
    return (
      operation !== null && !CommunityContributionGate.CONTRIBUTION_OPERATIONS.includes(operation)
    );
  }

  /** 设置桥拒绝时交给页面的回答，与其他请求失败同一个信封。 */
  static refusal(): string {
    return JSON.stringify({ ok: false, error: CommunityContributionGate.REFUSAL });
  }

  private static operation(payload: string): string | null {
    try {
      const document: Record<string, Object> = JSON.parse(payload) as Record<string, Object>;
      if (document === null || typeof document !== "object") return null;
      const value: Object | undefined = document["operation"];
      return typeof value === "string" ? value : null;
    } catch {
      return null;
    }
  }
}
