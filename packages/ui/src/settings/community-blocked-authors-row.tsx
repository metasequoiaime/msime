import { useEffect, useState } from "react";
import { LinkRow } from "../core/platform-controls";
import { useCommunityBlocks } from "../community/community-block";
import { communityBlockedAuthorsTitle } from "../community/community-block-model";
import { useSettingsForm } from "./settings-form-context";

/**
 * 隐私设置里的「已屏蔽的作者」一行，与 iOS、Android 的同一行措辞相同：登录了打开「已屏蔽的作者」页，没有登录时写「登录后可用」、点了去登录。宿主不提供屏蔽（没有 `CommunityBlocksContext`）时不画。每回到这一页重新问一次登录状态，从登录页回来就跟着变。
 */
export function CommunityBlockedAuthorsRow() {
  const blocks = useCommunityBlocks();
  const { page, selectPage, openAccountLogin } = useSettingsForm();
  const [signedIn, setSignedIn] = useState<boolean | null>(null);
  useEffect(() => {
    if (!blocks) return;
    let current = true;
    blocks.signedIn().then(
      (value) => {
        if (current) setSignedIn(value);
      },
      () => {
        if (current) setSignedIn(false);
      },
    );
    return () => {
      current = false;
    };
  }, [blocks, page]);
  if (!blocks) return null;
  return (
    <LinkRow
      title={communityBlockedAuthorsTitle}
      description={signedIn ? "社区里不显示这些作者的作品" : undefined}
      value={signedIn === false ? "登录后可用" : undefined}
      onClick={() => (signedIn === false ? openAccountLogin() : selectPage("blocked-authors"))}
    />
  );
}
