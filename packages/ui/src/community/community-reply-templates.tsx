import { GroupList, NavRow } from "../core/platform-controls";
import { communityResourceSubtitle } from "./community-resource-model";
import {
  useCommunityReplyTemplates,
  type UseCommunityReplyTemplatesOptions,
} from "./use-community-reply-templates";
import * as settings from "../settings/settings-style";

export interface CommunityReplyTemplatesProps extends UseCommunityReplyTemplatesOptions {
  onBrowseMore: () => void;
}

/** 社区回复模板预览和添加入口；宿主注入浏览完整图库的动作。 */
export function CommunityReplyTemplates({
  client,
  inlineNotice = true,
  onBrowseMore,
}: CommunityReplyTemplatesProps) {
  const { items, failure, notice, adding, added, add } = useCommunityReplyTemplates({
    client,
    inlineNotice,
  });
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
