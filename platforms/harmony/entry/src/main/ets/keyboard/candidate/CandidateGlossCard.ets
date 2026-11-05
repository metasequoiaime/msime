import {
  CandidateGlossCardPolicy, GlossCard, GlossCardSection, GLOSS_CARD_CLOSE, GLOSS_CARD_COMMIT, GLOSS_CARD_COPY,
  GLOSS_CARD_EMPTY_HINT, GLOSS_CARD_NO_PRIMARY, GLOSS_CARD_PENDING, GLOSS_CARD_TITLE
} from './CandidateGlossCardPolicy';
import { ManagementAction } from './CandidateManagementAction';

const CARD_PADDING_VP: number = 10;
const CARD_RADIUS_VP: number = 10;
const TITLE_FONT_SIZE: number = 12;
const CLOSE_FONT_SIZE: number = 14;
const CANDIDATE_FONT_SIZE: number = 22;
const SENSE_FONT_SIZE: number = 15;
const SOURCE_FONT_SIZE: number = 12;
const ACTION_FONT_SIZE: number = 13;
const BUTTON_HEIGHT_VP: number = 36;
const BUTTON_FONT_SIZE: number = 14;
const DISABLED_OPACITY: number = 0.4;

/**
 * 手机长按候选打开的「释义」卡片（#10915），盖住整块按键区，候选栏留在上面。
 *
 * 从上到下：标题「释义」和「收起」，候选原文（大字），每种目标语言一段译文和它的来源，原来长按菜单里的操作（横向滚动的一行），最后是「上屏译文」「复制译文」两个按钮。写什么由 `CandidateGlossCardPolicy` 定，这里只画；按钮和操作的效果由调用方负责，卡片关不关也由调用方决定。
 */
@Component
export struct CandidateGlossCard {
  @Require @Prop card: GlossCard;
  @Require @Prop foreground: string;
  @Require @Prop secondary: string;
  @Require @Prop accent: string;
  @Require @Prop surface: string;
  @Require @Prop hair: string;
  @Require @Prop primaryBackground: string;
  @Require @Prop primaryForeground: string;
  @Require @Prop buttonBackground: string;
  @Require @Prop buttonForeground: string;
  @Require onCommit: () => void;
  @Require onCopy: () => void;
  @Require onClose: () => void;
  @Require onAction: (action: ManagementAction) => void;

  @Builder
  section(section: GlossCardSection) {
    Column({ space: 2 }) {
      ForEach(CandidateGlossCardPolicy.senseLines(section), (line: string) => {
        Text(line)
          .fontSize(SENSE_FONT_SIZE)
          .fontColor(this.foreground)
          .width('100%')
      }, (line: string, index: number): string => `${index}:${line}`)
      Text(section.sourceLine)
        .fontSize(SOURCE_FONT_SIZE)
        .fontColor(this.secondary)
        .margin({ top: 2 })
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .accessibilityGroup(true)
    .accessibilityText(section.accessibilityText)
  }

  /** 底部两个按钮之一：`commit` 为真是「上屏译文」，否则是「复制译文」。两个都只交第一目标语言的译文，没有时一起不可用。 */
  @Builder
  button(commit: boolean) {
    Button(commit ? GLOSS_CARD_COMMIT : GLOSS_CARD_COPY, { type: ButtonType.Capsule, stateEffect: true })
      .fontSize(BUTTON_FONT_SIZE)
      .fontColor(commit ? this.primaryForeground : this.buttonForeground)
      .backgroundColor(commit ? this.primaryBackground : this.buttonBackground)
      .height(BUTTON_HEIGHT_VP)
      .layoutWeight(1)
      .enabled(this.card.translation !== null)
      .opacity(this.card.translation !== null ? 1 : DISABLED_OPACITY)
      .accessibilityDescription(this.card.translation !== null ? '' : GLOSS_CARD_NO_PRIMARY)
      .onClick(() => {
        if (commit) {
          this.onCommit();
        } else {
          this.onCopy();
        }
      })
  }

  build() {
    Column({ space: 6 }) {
      Row() {
        Text(GLOSS_CARD_TITLE)
          .fontSize(TITLE_FONT_SIZE)
          .fontColor(this.secondary)
          .layoutWeight(1)
        Text(GLOSS_CARD_CLOSE)
          .fontSize(CLOSE_FONT_SIZE)
          .fontColor(this.accent)
          .padding({ left: 12, top: 2, bottom: 2 })
          .accessibilityText(CandidateGlossCardPolicy.closeLabel())
          .onClick(() => this.onClose())
      }
      .width('100%')
      Text(this.card.candidate)
        .fontSize(CANDIDATE_FONT_SIZE)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.foreground)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .width('100%')
        .accessibilityText(CandidateGlossCardPolicy.candidateLabel(this.card.candidate))
      Scroll() {
        Column({ space: 8 }) {
          if (this.card.sections.length === 0) {
            // 按键区不高，「暂无译文」和设置提示挨着放，两行都看得见。
            Column({ space: 2 }) {
              Text(CandidateGlossCardPolicy.emptyText(this.card))
                .fontSize(SENSE_FONT_SIZE)
                .fontColor(this.secondary)
              if (CandidateGlossCardPolicy.showsHint(this.card)) {
                Text(GLOSS_CARD_EMPTY_HINT)
                  .fontSize(SOURCE_FONT_SIZE)
                  .fontColor(this.secondary)
              }
            }
            .alignItems(HorizontalAlign.Start)
          } else {
            ForEach(this.card.sections, (section: GlossCardSection) => {
              this.section(section)
            }, (section: GlossCardSection): string =>
              `${section.language}:${section.source}:${section.senses.join('|')}`)
            if (this.card.pending) {
              Text(GLOSS_CARD_PENDING)
                .fontSize(SOURCE_FONT_SIZE)
                .fontColor(this.secondary)
            }
          }
        }
        .width('100%')
        .alignItems(HorizontalAlign.Start)
      }
      .layoutWeight(1)
      .width('100%')
      .align(Alignment.TopStart)
      .scrollBar(BarState.Off)
      .edgeEffect(EdgeEffect.Spring)
      if (this.card.actions.length > 0) {
        // 原来长按菜单里的操作，按原来的顺序；比屏幕宽时横向滚动。
        Scroll() {
          Row() {
            ForEach(this.card.actions, (action: ManagementAction) => {
              Text(action.title)
                .fontSize(ACTION_FONT_SIZE)
                .fontColor(action.available === false || action.checked === true ? this.foreground : this.accent)
                .opacity(action.available === false ? DISABLED_OPACITY : 1)
                .padding({ top: 4, bottom: 4, right: 16 })
                .onClick(() => this.onAction(action))
            }, (action: ManagementAction): string => `${action.id}:${action.menuItemId}:${action.title}`)
          }
        }
        .width('100%')
        .scrollable(ScrollDirection.Horizontal)
        .scrollBar(BarState.Off)
        .align(Alignment.Start)
        .borderWidth({ top: 1 })
        .borderColor(this.hair)
      }
      Row({ space: 8 }) {
        this.button(true)
        this.button(false)
      }
      .width('100%')
    }
    .width('100%')
    .height('100%')
    .padding(CARD_PADDING_VP)
    .backgroundColor(this.surface)
    .borderRadius(CARD_RADIUS_VP)
  }
}
