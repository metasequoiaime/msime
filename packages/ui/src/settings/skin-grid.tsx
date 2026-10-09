import { useId } from "react";
import type { Preferences } from "../index";
import { ActionButton } from "../core/action-button";
import { FluentIcon } from "../core/fluent-icons";
import * as controls from "../core/platform-controls-style";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import type { ExternalSkin } from "../skin/external-skins";
import {
  keyboardThemeId,
  themeEntry,
  type BaseGlobalTheme,
  type CustomTheme,
  type GlobalTheme,
  type ThemeCatalogEntry,
} from "../theme/global-theme";

/** 我的皮肤卡片：键盘编辑器的设计，仅在有编辑器的宿主上出现。 */
export type SkinGridCustomDesign = {
  design: TouchKeyboardSkinDesign;
  /** 自定义主题是否用这个设计绘制键盘。 */
  selected: boolean;
  onSelect: () => void;
};

export interface SkinGridProps {
  /** 本宿主提供的主题（`offeredThemeCatalog`），按目录顺序排成卡片。 */
  themes: ThemeCatalogEntry[];
  globalTheme: GlobalTheme;
  customTheme: CustomTheme | undefined;
  /** 扫描到的外部皮肤包，排在内置主题之后。 */
  packages: ExternalSkin[];
  /** 在有键盘皮肤编辑器（`customTouchKeyboardSkins`）的宿主上提供；它的卡片取代目录里的自定义。 */
  customDesign?: SkinGridCustomDesign;
  /** 键盘绘制所用的浅色或深色模式，供跟随模式的皮肤使用。 */
  keyboardTheme: "dark" | "light";
  /** 把卡片的选择应用到偏好设置草稿，与主题轮播的切换做的更新相同。 */
  onApply: (patch: Partial<Preferences>) => void;
  /** 告知点击的卡片应用了哪个皮肤，用于换装达人徽章：主题 id，我的皮肤或自定义主题为 `custom`，或者皮肤包 id，与 Android 皮肤页记录的 id 相同。 */
  onApplied?: (id: string) => void;
  /** 打开 AI 皮肤抽卡；只有提供它时才绘制虚线的 AI 卡片。 */
  onOpenAi?: () => void;
}

type Card = {
  key: string;
  title: string;
  selected: boolean;
  theme: "dark" | "light";
  skin: GlobalTheme;
  design?: TouchKeyboardSkinDesign;
  /** 换装达人徽章把这张卡片计为什么。 */
  statisticsId: string;
  select: () => void;
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

/** 按顺序排列的卡片：目录里的主题、我的皮肤，然后是外部皮肤包。最多选中一张：正在使用的皮肤包优先，其次是自定义主题正在绘制其设计时的我的皮肤，再次是选中的主题。在有编辑器的宿主上，既没有皮肤包也没有设计的自定义主题没有自己的卡片；它的颜色在更多选项里编辑。 */
function skinGridCards({
  themes,
  globalTheme,
  customTheme,
  packages,
  customDesign,
  keyboardTheme,
  onApply,
}: SkinGridProps): Card[] {
  // 皮肤包是自定义主题的一部分，所以自定义主题绘制它时，它的卡片就是正在使用的那张。
  const skinInUse = globalTheme === "custom" ? (customTheme?.candidate_skin ?? null) : null;
  const packageInUse = packages.some((skin) => skin.id === skinInUse);
  const designInUse = !packageInUse && customDesign !== undefined && customDesign.selected;
  const storedDesign = customTheme?.keyboard ?? undefined;
  // 自定义主题在 `base` 之上绘制的键盘：有设计时用设计，否则用该 base 的键盘。
  const customKeyboard = (base: BaseGlobalTheme | undefined) => ({
    skin: keyboardThemeId("custom", { ...customTheme, base }),
    design: storedDesign,
    theme: themeEntry(base ?? "system").appearance ?? keyboardTheme,
  });
  const cards: Card[] = [];
  for (const entry of themes) {
    const id = entry.id;
    if (id === "custom") {
      // 在有编辑器的宿主上，我的皮肤代表自定义主题，与 Android 皮肤页一致；再加一张卡片会绘制出相同的键盘。
      if (customDesign) continue;
      cards.push({
        key: `theme:${id}`,
        title: entry.title,
        selected: globalTheme === "custom" && !packageInUse,
        ...customKeyboard(customTheme?.base),
        statisticsId: "custom",
        // 选择自定义卡片本身会去掉皮肤包，保留自定义主题的其余部分，并在它自己的 base 上绘制。
        select: () =>
          onApply({
            global_theme: "custom",
            custom_theme: { ...customTheme, candidate_skin: null },
          }),
      });
      continue;
    }
    cards.push({
      key: `theme:${id}`,
      title: entry.title,
      selected: globalTheme === id,
      theme: entry.appearance ?? keyboardTheme,
      skin: id,
      statisticsId: id,
      select: () => onApply({ global_theme: id }),
    });
  }
  if (customDesign)
    cards.push({
      key: "design",
      title: "我的皮肤",
      selected: designInUse,
      theme: keyboardTheme,
      skin: "custom",
      design: customDesign.design,
      statisticsId: "custom",
      select: customDesign.onSelect,
    });
  for (const skin of packages)
    cards.push({
      key: `package:${skin.id}`,
      title: skin.name,
      selected: skin.id === skinInUse,
      ...customKeyboard(skin.base),
      statisticsId: skin.id,
      // 皮肤包 manifest 中的 base 成为自定义主题的 base，`resolve()` 在皮肤包之下绘制的就是它。
      select: () =>
        onApply({
          global_theme: "custom",
          custom_theme: { ...customTheme, base: skin.base, candidate_skin: skin.id },
        }),
    });
  return cards;
}

/** HarmonyOS 手机的皮肤网格：两列键盘缩略图，点击即应用，最后是一张进入 AI 皮肤抽卡的虚线卡片。 */
export function SkinGrid(props: SkinGridProps) {
  const cards = skinGridCards(props);
  const titleId = useId();
  return (
    <section className={controls.group} aria-labelledby={titleId}>
      <h3 id={titleId} className={controls.groupTitle} data-group-title="">
        皮肤
      </h3>
      <div className={grid} data-skin-grid="">
        {cards.map((card) => (
          <ActionButton
            key={card.key}
            // 点击已在使用的卡片不改变任何东西，所以不触碰草稿。
            action={
              card.selected
                ? () => undefined
                : () => {
                    card.select();
                    props.onApplied?.(card.statisticsId);
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
                </span>
              </>
            }
          />
        ))}
        {props.onOpenAi && (
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
      </div>
    </section>
  );
}
