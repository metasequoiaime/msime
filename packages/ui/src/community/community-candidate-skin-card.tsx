import { CommunityCard } from "./community-card";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCardMetrics } from "./community-card-metrics";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import type { CommunityCandidateSkin } from "./community-candidate-skin-model";
import {
  CandidateSkinPreviewImage,
  type CandidateSkinPreviewLoader,
} from "./community-candidate-skin-preview";
import * as style from "./community-style";

export interface CommunityCandidateSkinCardProps {
  skin: CommunityCandidateSkin;
  load: CandidateSkinPreviewLoader;
  open: () => void;
}

/** 候选窗口皮肤社区卡片；只负责展示预览与元数据，并把打开详情交给调用方。 */
export function CommunityCandidateSkinCard({ skin, load, open }: CommunityCandidateSkinCardProps) {
  return (
    <CommunityCard aria-label={`查看候选窗口皮肤 ${skin.name}`} onClick={open}>
      <CandidateSkinPreviewImage
        id={skin.id}
        revision={skin.updated_at}
        name={skin.name}
        load={load}
        className={style.cardStage}
      />
      <strong className={style.cardTitle}>{skin.name}</strong>
      <CommunityCardAuthor
        prefix={communitySkinCategoryLabel(skin.category) ?? undefined}
        author={skin.author}
        owned={skin.owned}
        private={skin.visibility === "private"}
        removed={skin.moderation === "removed"}
      />
      <CommunityCardMetrics
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
      />
      {skin.license.assets.trim() && (
        <span className={style.cardAuthor}>素材授权 {skin.license.assets.trim()}</span>
      )}
    </CommunityCard>
  );
}
