import { ManagementAction } from "./CandidateManagementAction";
import {
  TranslationCandidate,
  TranslationEntry,
  TranslationPolicy,
  TranslationQuery,
} from "./TranslationPolicy";
import { TranslationSensePolicy } from "./TranslationSensePolicy";
import { PASTEBOARD_BUSY_CODE, PASTEBOARD_PROHIBITED_CODE } from "../emoji/MascotStickerPolicy";

/**
 * 手机长按候选打开的「释义」卡片（#10915）：卡片上写什么、按钮交出什么、什么时候还要问在线服务。
 *
 * 译文只来自现有的候选翻译通路：离线释义词典，和用户已经配置好的在线翻译服务。卡片只为长按的这一个候选查，查询文档由共享层的 `translationQuery` 给出，这里只把候选换成这一个、把「仅当前高亮」拆出去的离线释义并回来；计划、缓存、失败冷却和隐私授权都照候选栏的走（`KeyboardSession.glossCardLookup`、`glossCardFetch`）。2in1 的候选窗不用这张卡片，长按仍是原来的菜单。
 */

export const GLOSS_CARD_TITLE: string = "释义";
export const GLOSS_CARD_EMPTY: string = "暂无译文";
/** 没有可用的在线翻译服务、离线释义也没有这一条时，跟在「暂无译文」后面。 */
export const GLOSS_CARD_EMPTY_HINT: string = "可以在设置里开启翻译服务";
export const GLOSS_CARD_PENDING: string = "正在查询译文…";
export const GLOSS_CARD_COMMIT: string = "上屏译文";
export const GLOSS_CARD_COPY: string = "复制译文";
export const GLOSS_CARD_CLOSE: string = "收起";
export const GLOSS_CARD_COPIED: string = "已复制译文";
/** 两个按钮不可用时读屏补的一句：它们只交第一目标语言的译文。 */
export const GLOSS_CARD_NO_PRIMARY: string = "第一目标语言还没有译文";

/** 离线释义的来源标识；在线服务用 `TranslationPolicy.provider` 的值。 */
export const GLOSS_SOURCE_OFFLINE: string = "offline";

const LANGUAGE_NAMES: Record<string, string> = {
  en: "英语",
  fr: "法语",
  ja: "日语",
  es: "西班牙语",
  ru: "俄语",
  de: "德语",
  ko: "韩语",
};

const SOURCE_NAMES: Record<string, string> = {
  offline: "离线释义",
  account: "水杉账号",
  tencent: "腾讯翻译",
  niutrans: "小牛翻译",
  custom: "自定义服务",
  ai: "AI 辅助",
};

/** 一种目标语言在本机已经有的答案，和还要不要问在线服务。 */
export interface GlossCardAnswer {
  readonly target: string;
  /** 随包的离线英文释义（只在目标语言是英语、查询开着 `english_gloss` 时有）。 */
  readonly english: string | null;
  /** 这种语言的离线释义词典给出的释义。 */
  readonly offline: string | null;
  /** 在线服务的译文：此前答过、还在缓存里的，或这次卡片请求到的。 */
  readonly online: string | null;
  /** 还要问在线服务：服务开着、离线英文释义没覆盖它、缓存里没有、也不在失败冷却期里。 */
  readonly wanted: boolean;
}

/** 卡片查到的全部答案，`answers` 按目标语言顺序排，第一项是第一目标语言。 */
export interface GlossCardLookup {
  /** 选中且可用的在线服务（`TranslationPolicy.provider`），没有时为空串。 */
  readonly provider: string;
  readonly answers: GlossCardAnswer[];
}

/** 卡片上的一段：一种目标语言的译文和它的来源。 */
export interface GlossCardSection {
  readonly language: string;
  readonly languageName: string;
  readonly source: string;
  readonly sourceName: string;
  /** 逐条列出的义项；在线译文是一整条。 */
  readonly senses: string[];
  /** 「目标语言名 · 来源名」。 */
  readonly sourceLine: string;
  readonly accessibilityText: string;
}

/** 卡片从哪里长按打开：候选栏按这一页的序号，展开面板按整份列表的序号。 */
export type GlossCardOrigin = "strip" | "expanded";

/** 卡片此刻要画的全部内容，交给 `CandidateGlossCard`。 */
export interface GlossCard {
  readonly origin: GlossCardOrigin;
  readonly index: number;
  /** 候选原文，按显示的样子（繁体输出时是繁体）。 */
  readonly candidate: string;
  /** 交给翻译通路的这个候选：Engine 的原文和来源。 */
  readonly subject: TranslationCandidate;
  readonly sections: GlossCardSection[];
  /** 还有在线请求没回来。 */
  readonly pending: boolean;
  /** 有选中且可用的在线服务；没有时空状态补一句去设置里开启。 */
  readonly serviceAvailable: boolean;
  /** 「上屏译文」「复制译文」交出的文字：第一目标语言的第一条译文，没有时为 null，两个按钮不可用。 */
  readonly translation: string | null;
  /** 原来长按菜单里的那些操作，并进卡片的次要操作行。 */
  readonly actions: ManagementAction[];
  /** 每次打开加一，晚到的在线答案按它认是不是还是这一张。 */
  readonly serial: number;
  /** 打开时的组字（候选栏上面那一行读音）。只有它变了卡片才关：同一段组字里候选会重排（停顿后的整句重解），那不算换了一件事。 */
  readonly composition: string;
}

export class CandidateGlossCardPolicy {
  static languageName(code: string): string {
    return LANGUAGE_NAMES[code] ?? code;
  }

  static sourceName(source: string): string {
    return SOURCE_NAMES[source] ?? source;
  }

  /**
   * 只问这一个候选的查询文档；候选的来源本来就不配释义（`sourceGetsNoGloss`）或原文为空时为 null。
   *
   * 「仅当前高亮」时共享层把整页的离线释义放在 `gloss_query` 里，主查询只带高亮那一个和翻译服务；卡片是用户对这一个候选的明确请求，所以两边的离线词典和英文释义开关并在一起，服务配置取主查询的。手机键盘只显示一种候选翻译（#8218），卡片和候选栏一样经过 `keepPrimaryOnly`，只查第一目标语言，与 Android、iOS 的卡片相同。返回副本，不改 `query`。
   */
  static cardQuery(
    query: TranslationQuery,
    subject: TranslationCandidate,
  ): TranslationQuery | null {
    if (subject.text.length === 0 || TranslationPolicy.sourceGetsNoGloss(subject.source)) {
      return null;
    }
    const gloss: TranslationQuery | null = query.gloss_query ?? null;
    const targets: string[] = TranslationPolicy.targets(query);
    const offline: string[] = (query.offline_gloss_languages ?? []).slice();
    if (gloss !== null) {
      for (const target of TranslationPolicy.targets(gloss)) {
        if (!targets.includes(target)) targets.push(target);
      }
      for (const language of gloss.offline_gloss_languages ?? []) {
        if (!offline.includes(language)) offline.push(language);
      }
    }
    const card: TranslationQuery = {
      ...query,
      target_language: targets.length > 0 ? targets[0] : query.target_language,
      target_languages: targets,
      candidates: [{ text: subject.text, source: subject.source }],
      english_gloss: query.english_gloss || (gloss !== null && gloss.english_gloss),
      resources: query.resources ?? gloss?.resources,
      sentence: false,
      gloss_query: null,
    };
    card.offline_gloss_languages = offline;
    TranslationPolicy.keepPrimaryOnly(card);
    // 共享层没有离线词典要查时不带这个字段，这里照样。
    if ((card.offline_gloss_languages ?? []).length === 0) {
      delete card.offline_gloss_languages;
    }
    return card;
  }

  /** 这种语言要不要问在线服务：有服务，而且不是离线英文释义已经答了的英语，与候选栏 `fetchTargetTranslations` 的规则相同。 */
  static asksProvider(
    provider: string,
    target: string,
    englishGloss: boolean,
    english: string | null,
  ): boolean {
    return provider.length > 0 && !(target === "en" && englishGloss && english !== null);
  }

  /** `entries` 里给 `text` 的那一条，没有时为 null。 */
  static translationOf(entries: TranslationEntry[], text: string): string | null {
    const found: TranslationEntry | undefined = entries.find(
      (entry: TranslationEntry): boolean => entry.text === text,
    );
    if (found === undefined) return null;
    const value: string = TranslationPolicy.normalise(found.translation);
    return value.length === 0 ? null : value;
  }

  /** 在线请求回来以后的这一种语言：不再等，有答案就记上。 */
  static answered(answer: GlossCardAnswer, online: string | null): GlossCardAnswer {
    return {
      target: answer.target,
      english: answer.english,
      offline: answer.offline,
      online: online ?? answer.online,
      wanted: false,
    };
  }

  /** 在线请求作废时不再等：各语言按本机已有的答案画。 */
  static settled(lookup: GlossCardLookup): GlossCardLookup {
    return {
      provider: lookup.provider,
      answers: lookup.answers.map((answer: GlossCardAnswer): GlossCardAnswer =>
        CandidateGlossCardPolicy.answered(answer, null),
      ),
    };
  }

  /** 离线释义拆成逐条的义项：同一条候选的几条释义由 `TranslationPolicy.append` 用「 / 」连着，每条里的义项再按分号分。 */
  static offlineSenses(gloss: string): string[] {
    const senses: string[] = [];
    for (const part of gloss.split(" / ")) {
      for (const sense of TranslationSensePolicy.split(part)) {
        if (!senses.includes(sense)) senses.push(sense);
        if (senses.length === TranslationSensePolicy.MAX_SENSES) return senses;
      }
    }
    return senses;
  }

  /**
   * 一种语言在卡片上的那一段，没有译文时为 null。
   *
   * 先后与候选栏相同：英语的离线英文释义最先（有它时候选栏也不问在线服务），然后是用户自己的在线服务，离线释义词典补在线服务没答的。一种语言只列一个来源。
   */
  static section(answer: GlossCardAnswer, provider: string): GlossCardSection | null {
    if (answer.english !== null) {
      return CandidateGlossCardPolicy.build(
        answer.target,
        GLOSS_SOURCE_OFFLINE,
        CandidateGlossCardPolicy.offlineSenses(answer.english),
      );
    }
    if (answer.online !== null && provider.length > 0) {
      return CandidateGlossCardPolicy.build(answer.target, provider, [answer.online]);
    }
    if (answer.offline !== null) {
      return CandidateGlossCardPolicy.build(
        answer.target,
        GLOSS_SOURCE_OFFLINE,
        CandidateGlossCardPolicy.offlineSenses(answer.offline),
      );
    }
    return null;
  }

  static sections(lookup: GlossCardLookup | null): GlossCardSection[] {
    if (lookup === null) return [];
    const result: GlossCardSection[] = [];
    for (const answer of lookup.answers) {
      const section: GlossCardSection | null = CandidateGlossCardPolicy.section(
        answer,
        lookup.provider,
      );
      if (section !== null) result.push(section);
    }
    return result;
  }

  static pending(lookup: GlossCardLookup | null): boolean {
    return (
      lookup !== null && lookup.answers.some((answer: GlossCardAnswer): boolean => answer.wanted)
    );
  }

  /** 两个按钮交出的文字：第一目标语言那一段的第一条，没有译文时为 null。 */
  static translation(lookup: GlossCardLookup | null, sections: GlossCardSection[]): string | null {
    if (lookup === null || lookup.answers.length === 0) return null;
    const primary: string = lookup.answers[0].target;
    const section: GlossCardSection | undefined = sections.find(
      (entry: GlossCardSection): boolean => entry.language === primary,
    );
    return section === undefined || section.senses.length === 0 ? null : section.senses[0];
  }

  static card(
    origin: GlossCardOrigin,
    index: number,
    candidate: string,
    subject: TranslationCandidate,
    lookup: GlossCardLookup | null,
    actions: ManagementAction[],
    serial: number,
    composition: string,
  ): GlossCard {
    const sections: GlossCardSection[] = CandidateGlossCardPolicy.sections(lookup);
    return {
      origin: origin,
      index: index,
      candidate: candidate,
      subject: subject,
      sections: sections,
      pending: CandidateGlossCardPolicy.pending(lookup),
      serviceAvailable: lookup !== null && lookup.provider.length > 0,
      translation: CandidateGlossCardPolicy.translation(lookup, sections),
      actions: actions,
      serial: serial,
      composition: composition,
    };
  }

  /** 同一张卡片换上新的次要操作（候选栏重画、释义晚到时那一格的菜单会变）。 */
  static withActions(card: GlossCard, actions: ManagementAction[], index: number): GlossCard {
    return { ...card, actions: actions, index: index };
  }

  /** 同一张卡片换上在线请求回来以后的答案。 */
  static withLookup(card: GlossCard, lookup: GlossCardLookup | null): GlossCard {
    return CandidateGlossCardPolicy.card(
      card.origin,
      card.index,
      card.candidate,
      card.subject,
      lookup,
      card.actions,
      card.serial,
      card.composition,
    );
  }

  /** 没有任何一段时卡片中间那一句。 */
  static emptyText(card: GlossCard): string {
    return card.pending ? GLOSS_CARD_PENDING : GLOSS_CARD_EMPTY;
  }

  /** 「暂无译文」后面要不要补「可以在设置里开启翻译服务」：只在确实没有可用的在线服务时补，开着服务却没答上来时这句话不对。 */
  static showsHint(card: GlossCard): boolean {
    return card.sections.length === 0 && !card.pending && !card.serviceAvailable;
  }

  /** 有义项时逐条编号，只有一条时不编号。 */
  static senseLines(section: GlossCardSection): string[] {
    if (section.senses.length <= 1) return section.senses.slice();
    return section.senses.map((sense: string, index: number): string => `${index + 1}. ${sense}`);
  }

  /** 卡片还开着吗：只看组字有没有变。上屏、继续打字、退格都会改组字，卡片关掉；停顿后的整句重解只给同一段组字换一代候选、重排顺序，卡片留着（Android 在设备上撞过：长按刚打开约 130 ms 重解就到了，卡片随即被关掉）。 */
  static stillComposing(card: GlossCard, composition: string): boolean {
    return card.composition === composition;
  }

  /** 卡片的候选此刻在 `texts`（候选栏这一页或展开列表）里的序号，按原文找：还在原来那一格就用原来的，换了位置用新位置，不在了为 -1。词库操作按这个序号做。 */
  static currentIndex(card: GlossCard, texts: string[]): number {
    if (card.index >= 0 && card.index < texts.length && texts[card.index] === card.candidate) {
      return card.index;
    }
    return texts.indexOf(card.candidate);
  }

  /** 工具条和按键在读屏里藏不藏：键盘调整时（读屏只走遮罩上的柄和按钮）和释义卡片盖着时整块藏起来，读屏不会落到被盖住的键上。 */
  static keysAccessibilityLevel(adjusting: boolean, cardOpen: boolean): string {
    return adjusting || cardOpen ? "no-hide-descendants" : "auto";
  }

  static candidateLabel(candidate: string): string {
    return `候选，${candidate}`;
  }

  static closeLabel(): string {
    return "收起释义";
  }

  static commitAnnouncement(text: string): string {
    return `已输入 ${text}`;
  }

  /** 写剪贴板失败时的提示，说清楚是哪一种拒绝。 */
  static copyFailureNotice(code: number | undefined): string {
    if (code === PASTEBOARD_BUSY_CODE) return "复制失败：剪贴板正忙，请重试";
    if (code === PASTEBOARD_PROHIBITED_CODE) return "复制失败：系统不允许写入剪贴板";
    return "复制失败，请重试";
  }

  private static build(
    language: string,
    source: string,
    senses: string[],
  ): GlossCardSection | null {
    if (senses.length === 0) return null;
    const languageName: string = CandidateGlossCardPolicy.languageName(language);
    const sourceName: string = CandidateGlossCardPolicy.sourceName(source);
    return {
      language: language,
      languageName: languageName,
      source: source,
      sourceName: sourceName,
      senses: senses,
      sourceLine: `${languageName} · ${sourceName}`,
      accessibilityText: `${languageName}：${senses.join("；")}。来源：${sourceName}`,
    };
  }
}
