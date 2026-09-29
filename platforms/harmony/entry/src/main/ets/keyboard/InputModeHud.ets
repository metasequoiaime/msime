import { KeyboardSession } from './KeyboardSession';
import { GlobalTheme } from './skin/GlobalTheme';

/**
 * The badge that says which mode a switch landed in, ported from
 * platforms/macos/src/InputModeHUDPanel.mm.
 *
 * A capsule rather than a square, because the logo and the character sit side by side the way the
 * badge on a toolbar carries them. It appears under the caret, close enough to be read without
 * looking away from what is being typed, and it goes again on its own: it never takes focus and
 * never takes a click. The host owns the window and the timing; this is only its face.
 */

const CORNER_RADIUS_VP: number = 16;
const LOGO_SIDE_VP: number = 30;
const CONTENT_SPACING_VP: number = 6;
const MODE_FONT_SIZE: number = 26;

@Entry
@Component
struct InputModeHud {
  @State english: boolean = false;
  /** The global theme's accent and its paired ink, which the session sends with every switch; the platform accent until the first one. */
  @State fill: string = GlobalTheme.accent(false);
  @State ink: string = '#FFFFFF';

  aboutToAppear(): void {
    KeyboardSession.shared.onModeBadge = (english: boolean, fill: string, ink: string): void => {
      this.english = english;
      this.fill = fill;
      this.ink = ink;
    };
  }

  aboutToDisappear(): void {
    KeyboardSession.shared.onModeBadge = undefined;
  }

  build() {
    Row({ space: CONTENT_SPACING_VP }) {
      Image($r('app.media.keyboard_brand'))
        .width(LOGO_SIDE_VP)
        .height(LOGO_SIDE_VP)
        .fillColor(this.ink)
      // 中 or 英: the state being switched to, not the pair being switched between.
      Text(this.english ? '英' : '中')
        .fontSize(MODE_FONT_SIZE)
        .fontColor(this.ink)
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .backgroundColor(this.fill)
    .borderRadius(CORNER_RADIUS_VP)
  }
}
