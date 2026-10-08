import { KeyboardIconPaths } from '../KeyboardIconPaths';
import { OneHandedPolicy } from '../OneHandedPolicy';
import { KeyboardLineIcon } from './KeyboardLineIcon';

/**
 * 单手模式 放在缩窄后的按键旁边的侧栏，移植自 Android 的 `OneHandGutterView`，尺寸取自设计稿（`OneHandedPolicy`）。
 *
 * 两个 40vp 的按钮，相距 14vp，在侧栏宽度内居中；容纳侧栏的那一行让它相对按键高度居中。换到另一侧 是按键颜色的圆盘，带按键阴影，里面用按键文字颜色画一个指向按键将要移去那一侧的箭头。退出单手 没有填充，用次要文字颜色画展开箭头。两者按下时都缩小到 .9。
 */
@Component
export struct OneHandRail {
  /** 按键贴着右边缘，所以箭头指向左。 */
  @Prop keysOnRight: boolean = true;
  @Require @Prop keyBackground: string;
  @Require @Prop keyForeground: string;
  @Require @Prop secondary: string;
  @Prop shadowRadius: number = 0;
  @Prop shadowColor: string = '#00000000';
  @Prop shadowOffset: number = 0;
  @Require onSwap: () => void;
  @Require onExit: () => void;

  build() {
    Column({ space: OneHandedPolicy.BUTTON_GAP_VP }) {
      Stack() {
        KeyboardLineIcon({
          d: this.keysOnRight ? KeyboardIconPaths.SWAP_TO_LEFT : KeyboardIconPaths.SWAP_TO_RIGHT,
          iconSize: OneHandedPolicy.ICON_VP,
          stroke: OneHandedPolicy.ICON_STROKE,
          color: this.keyForeground
        })
      }
      .width(OneHandedPolicy.BUTTON_VP)
      .height(OneHandedPolicy.BUTTON_VP)
      .borderRadius(OneHandedPolicy.BUTTON_VP / 2)
      .backgroundColor(this.keyBackground)
      .shadow({ radius: this.shadowRadius, color: this.shadowColor, offsetX: 0, offsetY: this.shadowOffset })
      .stateStyles({
        normal: {
          .scale({ x: 1, y: 1 })
        },
        pressed: {
          .scale({ x: OneHandedPolicy.PRESSED_SCALE, y: OneHandedPolicy.PRESSED_SCALE })
        }
      })
      .accessibilityText(OneHandedPolicy.swapLabel())
      .onClick(() => this.onSwap())

      Stack() {
        KeyboardLineIcon({
          d: KeyboardIconPaths.EXIT_ONE_HAND,
          iconSize: OneHandedPolicy.ICON_VP,
          stroke: OneHandedPolicy.ICON_STROKE,
          color: this.secondary
        })
      }
      .width(OneHandedPolicy.BUTTON_VP)
      .height(OneHandedPolicy.BUTTON_VP)
      .borderRadius(OneHandedPolicy.BUTTON_VP / 2)
      .stateStyles({
        normal: {
          .scale({ x: 1, y: 1 })
        },
        pressed: {
          .scale({ x: OneHandedPolicy.PRESSED_SCALE, y: OneHandedPolicy.PRESSED_SCALE })
        }
      })
      .accessibilityText(OneHandedPolicy.exitLabel())
      .onClick(() => this.onExit())
    }
    .width('100%')
    .alignItems(HorizontalAlign.Center)
  }
}
