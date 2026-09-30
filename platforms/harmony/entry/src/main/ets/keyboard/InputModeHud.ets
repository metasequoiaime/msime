import { KeyboardSession } from './KeyboardSession';
import { GlobalTheme } from './skin/GlobalTheme';
import { InputModeHudPolicy } from './InputModeHudPolicy';
import { ToolbarSkin, ToolbarSkinPolicy } from './ToolbarSkinPolicy';

/**
 * The badge that says which mode a switch landed in, ported from platforms/macos/src/input/InputModeHUDPanel.mm.
 *
 * Wider than it is tall, because the logo and the character sit side by side the way the badge on a toolbar carries them. It appears under the caret, close enough to be read without looking away from what is being typed, and it goes again on its own: it never takes focus and never takes a click. The host owns the window and the timing; this is only its face.
 *
 * It is the floating toolbar's size and wears the toolbar's palette: the same font size and scale (InputModeHudPolicy), and the toolbar theme's surface, border and text, so the two read as the same piece of the input method whichever of them the desktop is showing.
 */

@Entry
@Component
struct InputModeHud {
  @State english: boolean = false;
  /** The toolbar's font size and scale, which the session sends with every switch; the toolbar's defaults until the first one. */
  @State fontSize: number = 24;
  @State toolbarScale: number = 1;
  /** The toolbar theme's palette, resolved as the floating toolbar resolves it; the light candidate palette until the first switch. */
  @State toolbarSkin: ToolbarSkin = ToolbarSkinPolicy.base(GlobalTheme.candidateColors(null, false));

  aboutToAppear(): void {
    KeyboardSession.shared.onModeBadge = (english: boolean, fontSize: number, scale: number,
                                          toolbarSkin: ToolbarSkin): void => {
      this.english = english;
      this.fontSize = fontSize;
      this.toolbarScale = scale;
      this.toolbarSkin = toolbarSkin;
    };
  }

  aboutToDisappear(): void {
    KeyboardSession.shared.onModeBadge = undefined;
  }

  build() {
    // The colour goes on a container that fills the panel, as on the toolbar: a panel is transparent where nothing is drawn.
    Row({ space: InputModeHudPolicy.spacingVp(this.toolbarScale) }) {
      // The full-colour application mark, as the macOS badge and the 2in1 candidate window carry it, not the single-colour keyboard_brand path the touch strip tints.
      Image($r('app.media.app_icon'))
        .width(InputModeHudPolicy.logoSideVp(this.toolbarScale))
        .height(InputModeHudPolicy.logoSideVp(this.toolbarScale))
        .objectFit(ImageFit.Contain)
      // 中 or 英: the state being switched to, not the pair being switched between.
      Text(this.english ? '英' : '中')
        .fontSize(InputModeHudPolicy.glyphFontSize(this.fontSize, this.toolbarScale))
        .fontWeight(FontWeight.Medium)
        .fontColor(this.toolbarSkin.buttonColor)
        .fontFamily(this.english ? this.toolbarSkin.englishFontFamily : this.toolbarSkin.fontFamily)
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .padding({ left: InputModeHudPolicy.insetVp(this.toolbarScale),
      right: InputModeHudPolicy.insetVp(this.toolbarScale) })
    .backgroundColor(this.toolbarSkin.backgroundColor)
    .borderRadius(InputModeHudPolicy.cornerRadiusVp(this.toolbarScale))
    // Its own edge for the same reason the toolbar has one: it floats over whatever the desktop happens to be.
    .borderWidth(1)
    .borderColor(this.toolbarSkin.borderColor)
  }
}
