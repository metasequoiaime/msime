import type { Preferences } from "../core/preferences-model";
import { ActionButton } from "../core/action-button";
import { deepEqual } from "../core/deep-equal";
import { FluentIcon } from "../core/fluent-icons";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type {
  SavedTouchKeyboardSkin,
  TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";
import type { ExternalSkin } from "../skin/external-skins";
import {
  applyCandidateSkin,
  keyboardThemeId,
  removeCandidateSkin,
  skinSlot,
  themeEntry,
  type BaseGlobalTheme,
  type CustomTheme,
  type GlobalTheme,
  type ThemeCatalogEntry,
} from "../theme/global-theme";

/** HarmonyOS 手机皮肤页顶部的分类，与 Android 皮肤页（`SkinsPage`）的「内置 / 我的 / 社区」一致：内置是目录里的主题和皮肤包，我的是 AI 设计入口、我的皮肤和皮肤库里的设计，社区直接是完整的社区皮肤图库。 */
export const skinGridTabs = [
  { id: "builtin", title: "内置" },
  { id: "mine", title: "我的" },
  { id: "community", title: "社区" },
] as const;

export type SkinGridTab = (typeof skinGridTabs)[number]["id"];

/** 我的皮肤卡片：键盘编辑器的设计，仅在有编辑器的宿主上出现。 */
export type SkinGridCustomDesign = {
  design: TouchKeyboardSkinDesign;
  /** 自定义主题是否用这个设计绘制键盘。 */
  selected: boolean;
  onSelect: () => void;
  /** 再点一次已在使用的我的皮肤卡片时打开编辑器；HarmonyOS 手机的皮肤页没有别的「设计我的皮肤」入口。 */
  onEdit?: () => void;
};

export interface SkinGridProps {
  /** 画哪个分类的卡片；选中状态仍按全部卡片判断，所以两个分类里同时最多选中同样的那几张。 */
  section: Exclude<SkinGridTab, "community">;
  /** 本宿主提供的主题（`offeredThemeCatalog`），按目录顺序排成卡片。 */
  themes: ThemeCatalogEntry[];
  globalTheme: GlobalTheme;
  customTheme: CustomTheme | undefined;
  /** 扫描到的外部皮肤包，排在内置主题之后。 */
  packages: ExternalSkin[];
  /** 在有键盘皮肤编辑器（`customTouchKeyboardSkins`）的宿主上提供；它的卡片取代目录里的自定义。 */
  customDesign?: SkinGridCustomDesign;
  /** 本机皮肤库里的设计：在「社区」里获取的皮肤和命名保存的设计，排在我的皮肤之后，与 Android 皮肤页「我的」里的设计一致。 */
  savedDesigns?: SavedTouchKeyboardSkin[];
  /** 选中皮肤库里的一个设计：把它设为自定义主题的键盘。 */
  onSelectSavedDesign?: (design: TouchKeyboardSkinDesign) => void;
  /** 键盘绘制所用的浅色或深色模式，供跟随模式的皮肤使用。 */
  keyboardTheme: "dark" | "light";
  /** 把卡片的选择应用到偏好设置草稿，与主题轮播的切换做的更新相同。 */
  onApply: (patch: Partial<Preferences>) => void;
  /** 告知点击的卡片应用了哪个皮肤，用于换装达人徽章：主题 id，我的皮肤或自定义主题为 `custom`，或者皮肤包 id，与 Android 皮肤页记录的 id 相同。 */
  onApplied?: (id: string) => void | Promise<void>;
  /** 打开 AI 皮肤抽卡；只有提供它时才在「我的」最前面绘制虚线的 AI 卡片。 */
  onOpenAi?: () => void;
}

type Card = {
  key: string;
  /** 卡片所在的分类：目录里的主题和皮肤包在「内置」，我的皮肤和皮肤库的设计在「我的」。 */
  section: SkinGridProps["section"];
  title: string;
  selected: boolean;
  theme: "dark" | "light";
  skin: GlobalTheme;
  design?: TouchKeyboardSkinDesign;
  /** 换装达人徽章把这张卡片计为什么。 */
  statisticsId: string;
  select: () => void;
  /** 再点一次已在使用的卡片时做什么；没有时什么也不改。皮肤包卡片把它从所在的槽位取下，我的皮肤卡片打开编辑器。 */
  deselect?: () => void;
  /** 再点一次是打开别处（编辑器），标题后画一个 ›。 */
  opens?: boolean;
};

const grid = "grid grid-cols-2 gap-x-3.5 gap-y-[18px] px-1 pt-1";
const cardButton =
  "flex min-w-0 cursor-pointer flex-col items-center gap-2 rounded-xl border-0 bg-transparent p-0 transition-transform duration-100 active:scale-[.97] focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-accent motion-reduce:transition-none";
/** 缩略图的边框：12px 圆角，选中时是一圈 2px 页面背景色间隙加 4px 强调色外环，否则是一条细线。 */
export const skinGridFrame = (selected: boolean) =>
  `block w-full min-w-0 overflow-hidden rounded-xl ${
    selected
      ? "[box-shadow:0_0_0_2px_var(--p-bg),0_0_0_4px_var(--accent-color)]"
      : "[box-shadow:0_0_0_1px_var(--p-hair)]"
  }`;
const caption = (selected: boolean) =>
  `flex max-w-full min-w-0 items-center gap-1 text-[15px] leading-tight whitespace-nowrap ${
    selected ? "font-semibold [color:var(--p-accent-text)]" : "font-normal [color:var(--p-text)]"
  }`;
const aiTile =
  "flex aspect-[390/292] w-full flex-col items-center justify-center gap-1.5 rounded-xl border-[1.5px] border-dashed border-[var(--accent-color)] bg-[var(--accent-soft)] [color:var(--p-accent-text)]";

function Check() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      data-skin-check=""
    >
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </svg>
  );
}

/** 按顺序排列的卡片：目录里的主题、我的皮肤、皮肤库的设计，然后是外部皮肤包；各自的 `section` 决定落在「内置」还是「我的」。皮肤包分浅色、深色两个槽位，所以最多同时选中两张皮肤包卡片；没有皮肤包在使用时，最多选中一张：自定义主题正在绘制其设计时的我的皮肤，否则是选中的主题。在有编辑器的宿主上，既没有皮肤包也没有设计的自定义主题没有自己的卡片。 */
function skinGridCards({
  themes,
  globalTheme,
  customTheme,
  packages,
  customDesign,
  savedDesigns = [],
  onSelectSavedDesign,
  keyboardTheme,
  onApply,
}: SkinGridProps): Card[] {
  // 皮肤包是自定义主题的一部分，放在浅色或深色槽位里的皮肤包都算正在使用。
  const skinsInUse =
    globalTheme === "custom"
      ? [customTheme?.candidate_skin, customTheme?.candidate_skin_dark].filter((id): id is string =>
          Boolean(id),
        )
      : [];
  const packageInUse = packages.some((skin) => skinsInUse.includes(skin.id));
  // 应用皮肤时用目录判断原来那款皮肤属于哪个槽位，见 `applyCandidateSkin`。
  const slotOf = (id: string) => {
    const skin = packages.find((entry) => entry.id === id);
    return skin ? skinSlot(skin.base) : undefined;
  };
  const storedDesign = customTheme?.keyboard ?? undefined;
  // 正在使用的设计就是皮肤库里的某一款时，选中的是那张有名字的卡片，我的皮肤不再同时打勾。
  const savedInUse =
    globalTheme === "custom" && !packageInUse
      ? savedDesigns.find((item) => deepEqual(item.design, storedDesign))
      : undefined;
  const designInUse =
    !packageInUse &&
    savedInUse === undefined &&
    customDesign !== undefined &&
    customDesign.selected;
  // 自定义主题在 `base` 之上绘制的键盘：有设计时用设计，否则用该 base 的键盘。
  const customKeyboard = (base: BaseGlobalTheme | undefined) => ({
    skin: keyboardThemeId("custom", { ...customTheme, base }),
    design: storedDesign,
    theme: themeEntry(base ?? "system").appearance ?? keyboardTheme,
  });
  const cards: Card[] = [];
  // 固定明暗的主题只在那种明暗下提供，否则键盘和周围的界面颜色反着、风格割裂，与键盘的皮肤面板（`GlobalTheme.fitsMode`）一致。所选主题属于另一种明暗时，键盘按「跟随系统」画，选中的也就是那张卡片。
  const dark = keyboardTheme === "dark";
  const chosen = themes.find((entry) => entry.id === globalTheme);
  const effectiveTheme =
    chosen && chosen.appearance !== null && (chosen.appearance === "dark") !== dark
      ? "system"
      : globalTheme;
  for (const entry of themes) {
    const id = entry.id;
    if (entry.appearance !== null && (entry.appearance === "dark") !== dark) continue;
    if (id === "custom") {
      // 在有编辑器的宿主上，我的皮肤代表自定义主题，与 Android 皮肤页一致；再加一张卡片会绘制出相同的键盘。
      if (customDesign) continue;
      cards.push({
        key: `theme:${id}`,
        section: "builtin",
        title: entry.title,
        selected: globalTheme === "custom" && !packageInUse,
        ...customKeyboard(customTheme?.base),
        statisticsId: "custom",
        // 选择自定义卡片本身会去掉两个槽位的皮肤包，保留自定义主题的其余部分，并在它自己的 base 上绘制。
        select: () =>
          onApply({
            global_theme: "custom",
            custom_theme: { ...customTheme, candidate_skin: null, candidate_skin_dark: null },
          }),
      });
      continue;
    }
    cards.push({
      key: `theme:${id}`,
      section: "builtin",
      title: entry.title,
      selected: effectiveTheme === id,
      theme: entry.appearance ?? keyboardTheme,
      skin: id,
      statisticsId: id,
      select: () => onApply({ global_theme: id }),
    });
  }
  if (customDesign)
    cards.push({
      key: "design",
      section: "mine",
      title: "我的皮肤",
      selected: designInUse,
      theme: keyboardTheme,
      skin: "custom",
      design: customDesign.design,
      statisticsId: "custom",
      select: customDesign.onSelect,
      deselect: customDesign.onEdit,
      opens: Boolean(customDesign.onEdit),
    });
  if (onSelectSavedDesign)
    for (const item of savedDesigns)
      cards.push({
        key: `saved:${item.id}`,
        section: "mine",
        title: item.name,
        selected: item === savedInUse,
        theme: keyboardTheme,
        skin: "custom",
        design: item.design,
        statisticsId: "custom",
        select: () => onSelectSavedDesign(item.design),
      });
  // 社区和本机的皮肤包是用户自己装的，两种明暗下都列出，不按 base 的明暗过滤；只有目录里的内置主题按明暗提供。
  for (const skin of packages) {
    cards.push({
      key: `package:${skin.id}`,
      section: "builtin",
      title: skin.name,
      selected: skinsInUse.includes(skin.id),
      ...customKeyboard(skin.base),
      statisticsId: skin.id,
      // 皮肤包放进它 base 所属的槽位（浅色或深色，`system` 底两个都放），另一个槽位不动；manifest 中的 base 成为自定义主题的 base，`resolve()` 在皮肤包之下绘制的就是它。
      select: () =>
        onApply({
          global_theme: "custom",
          custom_theme: applyCandidateSkin(customTheme, skin.id, skin.base, slotOf),
        }),
      deselect: () =>
        onApply({
          global_theme: "custom",
          custom_theme: removeCandidateSkin(customTheme, skin.id),
        }),
    });
  }
  return cards;
}

/** HarmonyOS 手机皮肤页一个分类里的皮肤网格：两列键盘缩略图，点击即应用。「我的」最前面是一张进入 AI 皮肤抽卡的虚线卡片，与 Android 皮肤页一致；分类名由所在的标签页给出，网格不再另画标题。 */
export function SkinGrid(props: SkinGridProps) {
  const cards = skinGridCards(props).filter((card) => card.section === props.section);
  return (
    <div className={grid} data-skin-grid={props.section}>
      {props.section === "mine" && props.onOpenAi && (
        <ActionButton
          action={props.onOpenAi}
          className={cardButton}
          ariaLabel="AI 设计皮肤，描述一句话生成"
          label={
            <>
              <span className={aiTile} data-skin-ai-tile="">
                <FluentIcon name="sparkle" size={26} />
                <span className="text-[13px] font-semibold">描述一句话生成</span>
              </span>
              <span className="text-[15px] leading-tight whitespace-nowrap [color:var(--p-accent-text)]">
                AI 设计皮肤
              </span>
            </>
          }
        />
      )}
      {cards.map((card) => (
        <ActionButton
          key={card.key}
          // 点击已在使用的卡片不改变任何东西，所以不触碰草稿；皮肤包卡片例外，再点一次把它从槽位取下。
          action={
            card.selected
              ? (card.deselect ?? (() => undefined))
              : () => {
                  card.select();
                  try {
                    const result = props.onApplied?.(card.statisticsId);
                    if (result) void result.catch(() => undefined);
                  } catch {
                    // 统计是后台徽章数据，宿主失败不应阻断换肤操作或冒泡成未处理异常。
                  }
                }
          }
          className={cardButton}
          ariaPressed={card.selected}
          label={
            <>
              <span className={skinGridFrame(card.selected)} data-skin-frame="">
                <ScreenKeyboardPreview
                  theme={card.theme}
                  skin={card.skin}
                  customDesign={card.design}
                  thumbnail
                />
              </span>
              <span className={caption(card.selected)}>
                {card.selected && <Check />}
                <span className="min-w-0 truncate">{card.title}</span>
                {card.selected && card.opens && (
                  <span aria-hidden="true" className="opacity-60">
                    ›
                  </span>
                )}
              </span>
            </>
          }
        />
      ))}
    </div>
  );
}
