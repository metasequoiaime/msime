import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import { CommunityCardMetrics } from "./community-card-metrics";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import type { CommunitySkinCardProps } from "./community-skin-card";
import type { CommunitySkinPillState } from "./use-community-skin-library";
import * as style from "./community-style";

export interface CommunityHarmonySkinCardProps extends CommunitySkinCardProps {
  pill: CommunitySkinPillState;
  act: () => void;
  touchRows?: PreviewKey[][];
}

const skinPillLabels: Record<CommunitySkinPillState, string> = {
  get: "获取",
  getting: "获取中",
  use: "使用",
  using: "使用",
  "in-use": "使用中",
};

/** HarmonyOS 手机上的皮肤卡片：预览图、名称、作者和使用次数，以及一个「获取 / 使用 / 使用中」胶囊按钮。点胶囊以外的任何位置打开详情，评分、举报和试用仍在详情里。 */
export function CommunityHarmonySkinCard({
  skin,
  theme,
  pill,
  open,
  act,
  touchRows,
}: CommunityHarmonySkinCardProps) {
  const author = [
    communitySkinCategoryLabel(skin.category),
    skin.owned ? "我的作品" : skin.author,
    skin.owned && skin.moderation === "removed" ? "已下架" : "",
  ]
    .filter(Boolean)
    .join(" · ");
  const settled = pill === "in-use";
  return (
    // 卡片本身是打开详情的指针目标；底部文字是可获得焦点的按钮，为键盘和读屏用户做同样的事，它的点击和轻点一样传到卡片上。
    <div className={style.harmonySkinCard} onClick={open}>
      <span className={style.harmonySkinStage}>
        <span className={style.harmonySkinStageClip}>
          <ScreenKeyboardPreview
            theme={theme}
            skin="custom"
            customDesign={skin.design}
            thumbnail
            touchRows={touchRows}
          />
        </span>
      </span>
      <div className={style.harmonySkinFooter}>
        <button
          type="button"
          className={style.harmonySkinText}
          aria-label={`查看皮肤 ${skin.name}`}
        >
          <span className={style.harmonySkinName}>{skin.name}</span>
          {author && <span className={style.harmonySkinMeta}>{author}</span>}
          <CommunityCardMetrics
            look="harmony"
            downloads={skin.downloads}
            ratingCount={skin.rating_count}
            ratingAverage={skin.rating_average}
          />
        </button>
        <button
          type="button"
          className={`${style.harmonySkinPill} ${settled ? style.harmonyPillDone : style.harmonyPillTonal}`}
          aria-label={`${skinPillLabels[pill]}皮肤 ${skin.name}`}
          disabled={pill !== "get" && pill !== "use"}
          onClick={(event) => {
            // 胶囊按钮直接作用于卡片的皮肤，不打开详情。
            event.stopPropagation();
            act();
          }}
        >
          {skinPillLabels[pill]}
        </button>
      </div>
    </div>
  );
}
