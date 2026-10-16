import { boundedGraphemes } from "../core/text";
import { resourceKindTitle } from "./community-helpers";
import { communityResourceSubtitle, type CommunityResource } from "./community-resource-model";
import * as style from "./community-style";

/** 行上的胶囊按钮提供什么：添加该条目、添加进行中，或因本次会话已添加而不提供。 */
export type CommunityResourcePillState = "add" | "adding" | "added";

const resourcePillLabels: Record<CommunityResourcePillState, string> = {
  add: "添加",
  adding: "添加中",
  added: "已添加",
};

export interface CommunityHarmonyResourceRowProps {
  item: CommunityResource;
  pill: CommunityResourcePillState;
  open: () => void;
  add: () => void;
}

/** HarmonyOS 手机上的一行词库或回复模板：色调底块上的首字、名称、元信息行和一个「添加」胶囊按钮。点按胶囊以外的任何地方都会打开详情。 */
export function CommunityHarmonyResourceRow({
  item,
  pill,
  open,
  add,
}: CommunityHarmonyResourceRowProps) {
  const subtitle = communityResourceSubtitle(item);
  return (
    // 整行是打开详情的指针目标；行内文字是可聚焦的按钮，为键盘和读屏用户做同样的事，该按钮的点击会像轻点一样传到整行。
    <div className={style.harmonyRow} onClick={open}>
      <span className={style.harmonyRowTile} aria-hidden="true">
        {boundedGraphemes(item.name.trim(), 1) || "?"}
      </span>
      <button
        type="button"
        className={style.harmonyRowText}
        aria-label={`查看${resourceKindTitle(item.kind)} ${item.name}`}
      >
        <span className={style.harmonyRowName}>{item.name}</span>
        {subtitle && <span className={style.harmonyRowMeta}>{subtitle}</span>}
        {item.kind === "reply" && item.description && (
          <span className={style.harmonyRowDescription}>{item.description}</span>
        )}
      </button>
      <button
        type="button"
        className={`${style.harmonyRowPill} ${pill === "added" ? style.harmonyPillDone : style.harmonyPillTonal}`}
        aria-label={`${resourcePillLabels[pill]}${resourceKindTitle(item.kind)} ${item.name}`}
        disabled={pill !== "add"}
        onClick={(event) => {
          // 胶囊按钮作用于本行的条目，不打开其详情。
          event.stopPropagation();
          add();
        }}
      >
        {resourcePillLabels[pill]}
      </button>
    </div>
  );
}
