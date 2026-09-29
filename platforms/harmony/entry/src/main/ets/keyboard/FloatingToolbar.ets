import { KeyboardSession } from './KeyboardSession';
import { ToolbarState, ToolbarButton, FloatingToolbarLayout } from './FloatingToolbarLayout';
import { GlobalTheme } from './skin/GlobalTheme';
import { ToolbarSkin, ToolbarSkinPolicy } from './ToolbarSkinPolicy';

/**
 * The bar that says what mode the keyboard is in and lets it be changed, ported from
 * platforms/macos/src/FloatingToolbarPanel.mm.
 *
 * A desktop has no keyboard on screen to carry this, so without it the only evidence of the mode is
 * the badge that flashes when it changes. Each button shows the state it is in rather than the one
 * it would switch to — 中 while composing Chinese, not 英 — which is the rule the Apple toolbar
 * follows and the opposite of what a button normally does.
 *
 * The gear is never hidden. Hiding the toolbar and opening the settings are both behind it, so
 * turning off the last of the others still has to leave a way back out.
 */

@Entry
@Component
struct FloatingToolbar {
  @State state: ToolbarState = FloatingToolbarLayout.idleState();
  @State buttons: ToolbarButton[] = [];
  @State hoveredButton: ToolbarButton | undefined = undefined;
  @State fontSize: number = 24;
  @State toolbarScale: number = 1;
  // The candidate palette of the global theme, so a bar floating beside the candidates does not look like another app's.
  @State toolbarSkin: ToolbarSkin = ToolbarSkinPolicy.base(GlobalTheme.candidateColors(null, false));

  aboutToAppear(): void {
    KeyboardSession.shared.onToolbar = (state: ToolbarState, buttons: ToolbarButton[],
                                        fontSize: number, scale: number,
                                        toolbarSkin: ToolbarSkin): void => {
      this.state = state;
      this.buttons = buttons;
      this.fontSize = fontSize;
      this.toolbarScale = scale;
      this.toolbarSkin = toolbarSkin;
    };
  }

  aboutToDisappear(): void {
    KeyboardSession.shared.onToolbar = undefined;
  }

  /**
   * Every measurement the bar is drawn from, as methods rather than locals.
   *
   * A build method holds the UI description and nothing else, so the values it needs have to be
   * named outside it. They all come from the one scale the shared preference sets.
   */
  private scaleFactor(): number {
    return FloatingToolbarLayout.scale(this.toolbarScale);
  }

  private toolbarHeight(): number {
    return FloatingToolbarLayout.heightVp(this.scaleFactor());
  }

  private handleHeight(): number {
    return 14 * this.scaleFactor();
  }

  private dividerHeight(): number {
    return Math.max(1, this.toolbarHeight() - 2 * this.scaleFactor());
  }

  build() {
    // The colour goes on a container that fills the panel rather than on the row of buttons: a panel
    // is transparent where nothing is drawn, and a row only covers the buttons it holds.
    Column() {
      Row({ space: FloatingToolbarLayout.buttonGapVp(this.scaleFactor()) }) {
        Column()
          .width(2.5 * this.scaleFactor())
          .height(this.handleHeight())
          .backgroundColor(this.toolbarSkin.dragHandleColor)
          .borderRadius(2 * this.scaleFactor())
          .gesture(
            PanGesture({ direction: PanDirection.All, distance: 1 })
              .onActionStart(() => {
                KeyboardSession.shared.beginToolbarDrag();
              })
              .onActionUpdate((event: GestureEvent) => {
                KeyboardSession.shared.dragToolbar(event.offsetX, event.offsetY);
              })
          )
        Column()
          .width(1.2 * this.scaleFactor())
          .height(this.dividerHeight())
          .backgroundColor(this.toolbarSkin.dividerColor)
        ForEach(this.buttons, (button: ToolbarButton) => {
          Text(FloatingToolbarLayout.face(button, this.state))
            .fontSize((button === ToolbarButton.SETTINGS ? this.fontSize - 2 : this.fontSize) * this.scaleFactor())
          .fontColor(button === ToolbarButton.SETTINGS
            ? this.toolbarSkin.settingsColor : this.toolbarSkin.buttonColor)
          .fontFamily(button === ToolbarButton.INPUT_MODE
            && (this.state.english || this.state.temporaryEnglish)
            ? this.toolbarSkin.englishFontFamily : this.toolbarSkin.fontFamily)
          .borderWidth({ bottom: button === ToolbarButton.INPUT_MODE && this.state.temporaryEnglish ? 1 : 0 })
          .borderColor(this.toolbarSkin.buttonColor)
          .textAlign(TextAlign.Center)
          .width(FloatingToolbarLayout.buttonWidthVp(this.scaleFactor()))
          .height(this.toolbarHeight() - FloatingToolbarLayout.edgeInsetVp(this.scaleFactor()))
          .borderRadius(6 * this.scaleFactor())
          .backgroundColor(this.hoveredButton === button
            ? this.toolbarSkin.buttonHoverColor : '#00000000')
          .onHover((isHover: boolean): void => {
            this.hoveredButton = isHover ? button : undefined;
          })
          .onClick(() => KeyboardSession.shared.runToolbarButton(button))
      }, (button: ToolbarButton) => `${button}:${FloatingToolbarLayout.face(button, this.state)}`)
      }
      .justifyContent(FlexAlign.Center)
      .width('100%')
      .height('100%')
    }
    .width('100%')
    .height('100%')
    .backgroundColor(this.toolbarSkin.backgroundColor)
    .borderRadius(this.toolbarSkin.cornerRadiusVp * this.scaleFactor())
    // A bar with nothing behind it needs its own edge: it floats over whatever the desktop happens
    // to be, and on a pale one a pale surface with no border is a row of letters in mid-air.
    .borderWidth(1)
    .borderColor(this.toolbarSkin.borderColor)
    .shadow({ radius: 12, color: '#33000000', offsetY: 2 })
  }
}
