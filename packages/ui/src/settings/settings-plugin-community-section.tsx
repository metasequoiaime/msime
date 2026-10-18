import { CommunityPluginsPage } from "../community/community-plugins";
import * as communityStyle from "../community/community-style";
import type { SettingsPageModel } from "./use-settings-page-model";
import type { useSettingsCommunityViews } from "./use-settings-community-views";
import type { SettingsShellView } from "./use-settings-shell-state";

export interface SettingsPluginCommunitySectionProps {
  model: Pick<SettingsPageModel, "client" | "page" | "openAccountLogin">;
  views: Pick<ReturnType<typeof useSettingsCommunityViews>, "pluginCommunityShown">;
  pluginView: SettingsShellView;
  onPluginViewChange: (view: SettingsShellView) => void;
}

/** 插件来源受外壳控制，社区图库继续位于偏好表单之外。 */
export function SettingsPluginCommunitySection({
  model,
  views,
  pluginView,
  onPluginViewChange,
}: SettingsPluginCommunitySectionProps) {
  const { client, page, openAccountLogin } = model;
  const { pluginCommunityShown } = views;
  return (
    <>
      {page === "plugins" && client.communityPlugins && (
        <div className={communityStyle.categoryTabsPair} role="tablist" aria-label="插件来源">
          <button
            type="button"
            role="tab"
            aria-selected={pluginView === "mine"}
            onClick={() => onPluginViewChange("mine")}
          >
            我的插件
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={pluginView === "community"}
            onClick={() => onPluginViewChange("community")}
          >
            社区插件
          </button>
        </div>
      )}
      {client.communityPlugins && pluginCommunityShown && (
        <div className="mt-4">
          <CommunityPluginsPage
            client={client.communityPlugins}
            localPlugins={client.plugins?.catalog}
            onLogin={openAccountLogin}
          />
        </div>
      )}
    </>
  );
}
