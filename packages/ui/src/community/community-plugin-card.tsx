import { kindLabels } from "../settings/plugin-catalog-helpers";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCard } from "./community-card";
import type { CommunityPlugin } from "./community-plugin-model";
import * as style from "./community-style";

export interface CommunityPluginCardProps {
  plugin: CommunityPlugin;
  open: () => void;
}

/** 社区插件图库卡片，打开详情的动作由调用方提供。 */
export function CommunityPluginCard({ plugin, open }: CommunityPluginCardProps) {
  return (
    <CommunityCard aria-label={`查看插件 ${plugin.name}`} onClick={open}>
      <strong className={style.cardTitle}>{plugin.name}</strong>
      <CommunityCardAuthor
        prefix={kindLabels[plugin.kind]}
        author={plugin.author}
        owned={plugin.owned}
        removed={plugin.moderation === "removed"}
      />
      {plugin.description && (
        <span className={style.resourceDescription}>{plugin.description}</span>
      )}
      <CommunityCardMetrics
        downloads={plugin.downloads}
        ratingCount={plugin.rating_count}
        ratingAverage={plugin.rating_average}
      />
    </CommunityCard>
  );
}
