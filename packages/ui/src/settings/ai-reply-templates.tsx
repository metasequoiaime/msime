import { useEffect, useState } from "react";
import { GroupList, NavRow } from "../core/platform-controls";
import { useToast } from "../core/toast";
import { resourceMessage } from "../community/community-helpers";
import {
  addCommunityReply,
  communityResourceSubtitle,
  type CommunityResource,
  type CommunityResourceClient,
} from "../community/community-resources";
import { useSettingsForm } from "./settings-form-context";
import { useAsyncGeneration } from "./use-async-generation";
import * as settings from "./settings-style";

/** 「回复模板」一组列出的社区回复模板数。AI 辅助页上还有别的设置，列得比「发现词库」的 8 个少。 */
const REPLY_PREVIEW_LIMIT = 5;

/**
 * 触屏宿主「AI 辅助」页上的「回复模板」：社区里的前几个回复模板，「添加」后存进本机的高情商回复键盘（和社区图库里的「添加」同一条路，见 `addCommunityReply`）；最后一行「浏览更多回复模板」在本页上打开完整的回复模板图库。鸿蒙原先「社区」标签页里叫「短语」的那一段就是这些模板。桌面不显示：桌面没有高情商回复键盘，回复模板仍在「社区」页。
 */
export function AiReplyTemplates() {
  const { client, mobilePlatform, harmonyPlatform, page, openCommunityGallery } = useSettingsForm();
  const community = client.communityResources;
  // 只在本页可见时挂载，每次进来都读一次社区列表。
  if (!mobilePlatform || !community || page !== "ai") return null;
  return (
    <ReplyTemplatesGroup
      community={community}
      // 鸿蒙手机上「添加」的结果用 toast 提示；其他触屏宿主没有 toast，写在组里。
      inlineNotice={!harmonyPlatform}
      onBrowseMore={() =>
        openCommunityGallery({ category: "reply", scope: "", initialMine: false })
      }
    />
  );
}

function ReplyTemplatesGroup({
  community,
  inlineNotice,
  onBrowseMore,
}: {
  community: CommunityResourceClient;
  inlineNotice: boolean;
  onBrowseMore: () => void;
}) {
  const toast = useToast();
  const generation = useAsyncGeneration(community);
  const [items, setItems] = useState<CommunityResource[] | null>(null);
  const [failure, setFailure] = useState("");
  const [notice, setNotice] = useState("");
  const [adding, setAdding] = useState<string | null>(null);
  // 本次进入页面后添加过的模板。宿主不报告哪些模板已在键盘里，与社区图库一样每行起初都是「添加」，再添加一次只会保留同一个模板。
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => {
    const current = generation.current;
    community
      .list("reply", "", "", 0)
      .then(
        (page) =>
          generation.current === current && setItems(page.items.slice(0, REPLY_PREVIEW_LIMIT)),
      )
      .catch(
        () => generation.current === current && setFailure("社区回复模板暂时读不到，请稍后再试。"),
      );
  }, [community]);
  const add = async (item: CommunityResource) => {
    if (adding) return;
    setAdding(item.id);
    setNotice("");
    try {
      const message = await addCommunityReply(community, item);
      setAdded((current) => new Set(current).add(item.id));
      toast(message);
      if (inlineNotice) setNotice(message);
    } catch (error) {
      const message = resourceMessage(error);
      toast(message);
      if (inlineNotice) setNotice(message);
    } finally {
      setAdding(null);
    }
  };
  return (
    <GroupList title="回复模板">
      {items === null || items.length === 0 ? (
        <p className={settings.groupNote} role={failure ? "alert" : undefined}>
          {failure || (items === null ? "正在读取社区回复模板…" : "社区里还没有回复模板。")}
        </p>
      ) : (
        items.map((item) => {
          const done = added.has(item.id);
          const pending = adding === item.id;
          return (
            <NavRow
              key={item.id}
              variant="me"
              title={item.name}
              description={item.description || communityResourceSubtitle(item)}
              trailing={
                <button
                  type="button"
                  className="secondary"
                  aria-label={`添加回复模板 ${item.name}`}
                  disabled={done || pending || adding !== null}
                  onClick={() => void add(item)}
                >
                  {done ? "已添加" : pending ? "添加中" : "添加"}
                </button>
              }
            />
          );
        })
      )}
      {notice && (
        <p className={settings.groupNote} role="status">
          {notice}
        </p>
      )}
      <NavRow variant="me" title="浏览更多回复模板" onClick={onBrowseMore} />
    </GroupList>
  );
}
