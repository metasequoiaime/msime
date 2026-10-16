import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import { CommunityCard } from "./community-card";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCardMetrics } from "./community-card-metrics";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import type { CommunitySkin } from "./community-skin-model";
import * as style from "./community-style";

export interface CommunitySkinCardProps {
  skin: CommunitySkin;
  theme: "light" | "dark";
  open: () => void;
}

export function CommunitySkinCard({ skin, theme, open }: CommunitySkinCardProps) {
  return (
    <CommunityCard aria-label={`查看皮肤 ${skin.name}`} onClick={open}>
      <span className={style.cardStage}>
        <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={skin.design} compact />
      </span>
      <strong>{skin.name}</strong>
      <CommunityCardAuthor
        prefix={communitySkinCategoryLabel(skin.category) ?? undefined}
        author={skin.author}
        owned={skin.owned}
        removed={skin.moderation === "removed"}
      />
      <CommunityCardMetrics
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
      />
    </CommunityCard>
  );
}
