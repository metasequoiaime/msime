import { CommunityBlockedAuthors, useCommunityBlocks } from "../../community/community-block";
import { communityBlockedAuthorsTitle } from "../../community/community-block-model";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { useSettingsForm } from "../settings-form-context";

/** 「已屏蔽的作者」：触屏宿主从「我的 › 隐私」打开，桌面从「关于 › 许可与隐私」打开。只在打开着时渲染，每次进来都重读名单。 */
export function BlockedAuthorsSettingsPage() {
  const { busy, page } = useSettingsForm();
  const blocks = useCommunityBlocks();
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "blocked-authors"}
      ariaLabel={communityBlockedAuthorsTitle}
    >
      {blocks && <CommunityBlockedAuthors blocks={blocks} />}
    </SettingsPageFieldset>
  );
}
