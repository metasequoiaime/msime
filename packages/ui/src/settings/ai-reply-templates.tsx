import { CommunityReplyTemplates } from "../community/community-reply-templates";
import { useSettingsForm } from "./settings-form-context";

/**
 * 触屏宿主「AI 辅助」页上的「回复模板」：社区里的前几个回复模板，「添加」后存进本机的高情商回复键盘（和社区图库里的「添加」同一条路，见 `addCommunityReply`）；最后一行「浏览更多回复模板」在本页上打开完整的回复模板图库。鸿蒙原先「社区」标签页里叫「短语」的那一段就是这些模板。桌面不显示：桌面没有高情商回复键盘，回复模板仍在「社区」页。
 */
export function AiReplyTemplates() {
  const { client, mobilePlatform, harmonyPlatform, page, openCommunityGallery } = useSettingsForm();
  const community = client.communityResources;
  // 只在本页可见时挂载，每次进来都读一次社区列表。
  if (!mobilePlatform || !community || page !== "ai") return null;
  return (
    <CommunityReplyTemplates
      client={community}
      // 鸿蒙手机上「添加」的结果用 toast 提示；其他触屏宿主没有 toast，写在组里。
      inlineNotice={!harmonyPlatform}
      onBrowseMore={() =>
        openCommunityGallery({ category: "reply", scope: "", initialMine: false })
      }
    />
  );
}
