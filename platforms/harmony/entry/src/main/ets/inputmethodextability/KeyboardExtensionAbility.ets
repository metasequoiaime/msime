import { CandidateLayoutPolicy } from '../keyboard/candidate/CandidateSkinPolicy';
import { CandidateCanvasPolicy } from '../keyboard/candidate/CandidateCanvasPolicy';
import { CandidateCanvasDecoder } from '../keyboard/candidate/CandidateCanvasFrame';
import InputMethodExtensionAbility from '@ohos.InputMethodExtensionAbility';
import inputMethodEngine from '@ohos.inputMethodEngine';
import Want from '@ohos.app.ability.Want';
import { KeyboardSession, KeyboardPreferences } from '../keyboard/KeyboardSession';
import { CandidateAnchor, CandidateAnchorPolicy } from './CandidateAnchorPolicy';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import fs from '@ohos.file.fs';
import display from '@ohos.display';
import window from '@ohos.window';
import ConfigurationConstant from '@ohos.app.ability.ConfigurationConstant';
import { KeyboardMetrics } from '../keyboard/KeyboardMetrics';
import { TouchPanelShape } from '../keyboard/TouchLayoutStylePolicy';
import { KeyboardFormFactorPolicy } from '../keyboard/KeyboardFormFactorPolicy';
import { KeyboardGeometry } from '../keyboard/KeyboardGeometry';
import { KeyboardFormFactor } from '../keyboard/KeyboardFormFactor';
import { HardwareKeyRouter, HardwareKey, HardwareKeyAction, HardwareKeyDecision }
  from '../keyboard/HardwareKeyRouter';
import { InputModeRouting, ModeGesture, ModeKey } from '../keyboard/InputModeRouting';
import { VoiceKey } from '../keyboard/input/VoiceHotkeyPolicy';
import { CandidateTextEdge } from '../keyboard/input/CandidateTextPolicy';
import { FloatingToolbarLayout } from '../keyboard/FloatingToolbarLayout';
import { InputModeHudPolicy, InputModeHudSize } from '../keyboard/InputModeHudPolicy';
import { FloatingToolbarDragPolicy } from '../keyboard/FloatingToolbarDragPolicy';
import { FloatingKeyboardPolicy } from '../keyboard/FloatingKeyboardPolicy';
import { FeedbackSettings } from '../keyboard/KeyboardFeedback';
import { DesktopSurface } from '../keyboard/DesktopSurface';
import type { KeyEvent as InputKeyEvent } from '@ohos.multimodalInput.keyEvent';
import { Action as KeyAction } from '@ohos.multimodalInput.keyEvent';
import { StagedResources } from '../keyboard/StagedResources';
import { HardwareKeyDispatch } from '../keyboard/HardwareKeyDispatch';
import { SchemeTraits } from '../keyboard/SchemeTraits';
import { PanelShortcutKey, PanelShortcutPolicy, PanelSurfaceAction }
  from '../keyboard/input/PanelShortcutPolicy';
import { ToolbarButton } from '../keyboard/FloatingToolbarLayout';
import { AttachedInputDevice, DeviceChange, HardwareKeyboardPolicy, KeyRoutingTransition }
  from '../keyboard/input/HardwareKeyboardPolicy';
import { HarmonyHardwareKeyboards } from '../keyboard/input/HarmonyHardwareKeyboards';
import { HardwareBackspaceGuard } from '../keyboard/input/BackspaceHoldPolicy';
import { HeldKeys, KeyIdPolicy } from '../keyboard/KeyIdPolicy';
import { HarmonyTelemetry } from '../telemetry/Telemetry';
import { HarmonySharedSandboxPolicy } from '../keyboard/HarmonySharedSandboxPolicy';
import { SharedStateMirror } from '../keyboard/SharedStateMirror';
import { AccountSessionAdoption } from '../account/AccountSessionAdoption';
import { HarmonyAccountIdentity } from '../account/HarmonyAccountIdentity';
import { SharedStateSync } from '../keyboard/SharedStateSync';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';
import { ScreenCaptureProtection } from '../keyboard/settings/ScreenCaptureProtection';
import { HarmonyReadAloud } from '../speech/HarmonyReadAloud';

// Where the badge goes and how long it stays, from platforms/macos/src/input/InputModeHUDPanel.mm: shown long enough to read and gone before the next word. Its size is the floating toolbar's, from InputModeHudPolicy.sizeVp.
const HUD_SCREEN_MARGIN_VP: number = 8;
const HUD_CARET_GAP_VP: number = 10;
const HUD_VISIBLE_MS: number = 780;
const BUNDLE_NAME: string = 'app.msime.hmos';
// KEYCODE_DEL, the Backspace key.
const KEYCODE_DEL: number = 2055;
const TOOLBAR_MARGIN_VP: number = 20;
// Wide enough for the key rows to keep the proportions they have on a phone, rather than stretching
// a ten-key row across a desktop.
const DESKTOP_SURFACE_WIDTH_VP: number = 520;

KeyboardLog.error('module loaded');
const engine: inputMethodEngine.InputMethodAbility = inputMethodEngine.getInputMethodAbility();

/**
 * The HarmonyOS input method host. The system starts it when an editor takes focus, so the panel is
 * created here rather than by opening a window of our own.
 *
 * The same host serves every form factor rather than being duplicated per device. What it creates
 * differs: a phone gets a soft keyboard fixed to the bottom of the screen, a 2in1 gets a candidate
 * window, because there the keys are already under the user's hands.
 */
export default class KeyboardExtensionAbility extends InputMethodExtensionAbility {
  private panel: inputMethodEngine.Panel | undefined = undefined;
  // The editor that already has focus when the panel finishes being created. The system sends
  // inputStart as soon as the ability is up, which is well before createPanel and setUiContent have
  // resolved, so an editor that arrives in that window is remembered and served once there is a panel
  // to show. Subscribing after the awaits instead would simply drop it, and the keyboard would sit
  // there visible but inert until the user focused something else.
  private pending: inputMethodEngine.InputClient | undefined = undefined;
  // Where the caret last was, in px, so the candidate window can be put under it. Kept here because
  // the panel is here: the view has no handle on the window it is drawn into.
  private caret: [number, number, number] = [0, 0, 0];
  private candidateAnchor: CandidateAnchor | undefined = undefined;
  private density: number = 1;
  // The badge that says which mode a switch landed in. Its own window, because it has to be able to
  // appear when the candidate window is not up: a mode is switched between words, not during one.
  private hud: inputMethodEngine.Panel | undefined = undefined;
  private hudTimer: number = -1;
  private readonly modeKeys: InputModeRouting = new InputModeRouting();
  private readonly backspaceHold: HardwareBackspaceGuard = new HardwareBackspaceGuard();
  // Physical keys that are down, so the key heatmap counts a held key once rather than once per repeat.
  private readonly heldKeys: HeldKeys = new HeldKeys();
  // The bar that says what mode the keyboard is in. Another window of its own, because it outlives
  // every composition and every editor: it is on screen whenever the keyboard is running at all.
  private toolbar: inputMethodEngine.Panel | undefined = undefined;
  // Preferences can be reloaded again while a STATUS_BAR panel is still being created. The
  // generation fences those awaits so an obsolete request cannot leak a panel or tear down the
  // panel belonging to a newer request.
  private statusPanelGeneration: number = 0;
  private toolbarPosition: [number, number] = [0, 0];
  // Set while the toolbar is hidden because the focused editor's window fills the screen.
  private toolbarHiddenForFullscreen: boolean = false;
  private toolbarDragOrigin: [number, number] = [0, 0];
  private screenWidth: number = 0;
  private screenHeight: number = 0;
  private candidateCount: number = 0;
  private candidateWidthVp: number = KeyboardMetrics.CANDIDATE_WINDOW_WIDTH_VP;
  // The attached keyboards, and whether their keys are currently being routed. Tracked rather than
  // re-derived because a second on('keyEvent') is not idempotent: it would deliver every keystroke
  // twice, and the framework de-duplicates nothing.
  private attachedKeyboards: AttachedInputDevice[] = [];
  private routingKeys: boolean = false;
  /** Set once the ability has torn down, so a late device enumeration cannot route keys again. */
  private tornDown: boolean = false;
  // ---- 手机和平板的软键盘面板（2in1 不用） ----
  /** 面板此刻的标志是不是 `FLG_FLOATING`（浮动键盘，#10225）。只在 `createPanel` 和 `changeFlag` 成功之后改。 */
  private floating: boolean = false;
  /** 停靠时面板的宽度（px）：创建面板时的屏幕宽度，停靠路径一直按它 resize，与加入浮动键盘之前相同。 */
  private fixedWidth: number = 0;
  /** 视图最近一次要的键盘高度（vp，不含浮动拖动条）。 */
  private touchHeightVp: number = 0;
  /** 浮动面板的位置：可移动范围里的千分比。从存着的设置读，拖动松手后更新；写盘失败时只在这个进程里有效。 */
  private floatingFraction: [number, number] = [FloatingKeyboardPolicy.DEFAULT_X_FRACTION,
    FloatingKeyboardPolicy.DEFAULT_Y_FRACTION];
  /** 浮动面板左上角此刻在屏幕上的位置（px）。 */
  private floatingPosition: [number, number] = [0, 0];
  private floatingDragOrigin: [number, number] = [0, 0];
  private floatingDragging: boolean = false;
  /** 换标志要先隐藏、再换、再显示，几步都是异步的；连点两下磁贴时按顺序一次做完一个。 */
  private floatingChange: Promise<void> = Promise.resolve();
  /** 是否订阅了 `display.on('change')`：只有手机和平板订阅，转屏后浮动面板跟着重新定宽和位置。 */
  private watchingDisplay: boolean = false;

  /** The editor moved its caret; the candidate window goes with it if it is up. */
  private readonly caretMoved: (x: number, y: number, height: number) => void =
    (x: number, y: number, height: number): void => {
      this.caret = [x, y, height];
      KeyboardSession.shared.invalidateReplyContext();
      // When follow-cursor is disabled, the first composition position is the anchor for the
      // whole candidate window lifetime. Keep recording the latest caret while idle so the next
      // composition still starts beside the newly focused text.
      if (KeyboardSession.shared.composing()
        && KeyboardSession.shared.preferences().candidateFollowCursor) {
        this.placeCandidateWindow();
      }
    };

  /** The editor changed its selection, invalidating everything derived from the old caret. */
  private readonly selectionChanged: (oldBegin: number, oldEnd: number,
                                      newBegin: number, newEnd: number) => void =
    (_oldBegin: number, _oldEnd: number, newBegin: number, newEnd: number): void => {
      KeyboardSession.shared.editorSelectionChanged(newBegin === newEnd);
    };

  /** The editor changed its document, either as our edit echo or independently of the keyboard. */
  private readonly textChanged: (value: string) => void = (value: string): void => {
    KeyboardSession.shared.editorTextChanged(value.length);
  };

  /**
   * As wide as the buttons the user left on, scaled by what they asked for.
   *
   * The width is computed from the same layout the page draws with, so a hidden button narrows the
   * bar rather than leaving a gap where it was.
   */
  private async resizeToolbar(density: number,
                              panel: inputMethodEngine.Panel | undefined = this.toolbar): Promise<void> {
    if (panel === undefined) {
      return;
    }
    const session: KeyboardSession = KeyboardSession.shared;
    const size: number[] = session.toolbarSize();
    const scale: number = session.toolbarScale();
    await panel.resize(Math.max(1, Math.round(size[0] * scale * density)),
      Math.max(1, Math.round(size[1] * scale * density)));
  }

  /**
   * The toolbar or the mode badge, whichever the preferences ask for, in the one STATUS_BAR panel.
   *
   * Measured on a 2in1: the framework grants one STATUS_BAR panel, not two. The toolbar takes it when there is one, and the toolbar is the better readout anyway — it says what mode the keyboard is in all the time rather than for three quarters of a second after it changed. The badge is what a desktop falls back to when the toolbar is turned off, which is exactly when nothing else on screen would say the mode had changed at all. The badge is also the one piece of feedback the user can turn off from the shared settings, and a panel that would never be shown is not worth the one STATUS_BAR grant to hold open.
   *
   * Run at creation and again whenever the settings page changes the preferences, so turning the toolbar on or off takes effect without restarting the input method, as the Windows host's config sync does. Whatever is to be closed is closed before anything is opened, since the grant has to be free first.
   */
  private async reconcileStatusPanels(): Promise<void> {
    const generation: number = ++this.statusPanelGeneration;
    const session: KeyboardSession = KeyboardSession.shared;
    const wantToolbar: boolean = session.toolbarEnabled();
    const wantBadge: boolean = !wantToolbar && session.modeBadgeEnabled();
    if (!wantToolbar && this.toolbar !== undefined) {
      this.closeToolbar();
    }
    if (!wantBadge && this.hud !== undefined) {
      this.closeModeBadge();
    }
    if (wantToolbar && this.toolbar === undefined) {
      await this.openToolbar(generation);
      if (generation !== this.statusPanelGeneration) return;
    } else if (this.toolbar !== undefined) {
      await this.resizeToolbar(this.density);
      if (generation !== this.statusPanelGeneration) return;
      session.refreshToolbar();
    }
    if (wantBadge && this.hud === undefined) {
      await this.openModeBadge(generation);
    } else if (this.hud !== undefined) {
      // The toolbar's font size or scale may be what changed, and the badge is sized by them.
      await this.resizeModeBadge(this.hud);
    }
  }

  private async openToolbar(generation: number): Promise<void> {
    let toolbar: inputMethodEngine.Panel | undefined = undefined;
    try {
      toolbar = await engine.createPanel(this.context, {
        type: inputMethodEngine.PanelType.STATUS_BAR
      });
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        await engine.destroyPanel(toolbar);
        return;
      }
      await toolbar.setUiContent('keyboard/FloatingToolbar');
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        await engine.destroyPanel(toolbar);
        return;
      }
      this.toolbar = toolbar;
      ScreenCaptureProtection.protectPanel(toolbar, this.screenCaptureProtected(), 'toolbar');
      KeyboardSession.shared.onToolbarResize = async (): Promise<void> => {
        await this.resizeToolbar(this.density, toolbar);
      };
      KeyboardSession.shared.onToolbarDragStart = (): void => {
        this.beginToolbarDrag();
      };
      KeyboardSession.shared.onToolbarDrag = (offsetX: number, offsetY: number): void => {
        this.dragToolbar(offsetX, offsetY);
      };
      await this.resizeToolbar(this.density, toolbar);
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        // A newer reconciliation may have adopted this panel. Leave it to that call (or
        // teardown) rather than destroying the panel it now owns.
        if (this.toolbar !== toolbar) {
          await engine.destroyPanel(toolbar);
        }
        return;
      }
      KeyboardSession.shared.refreshToolbar();
      // Lower right by default, clear of the dock, where a status bar is looked for and where it is least likely to sit on top of what is being written. macOS picks the same corner.
      const size: number[] = KeyboardSession.shared.toolbarSize();
      const width: number = size[0] * KeyboardSession.shared.toolbarScale() * this.density;
      const height: number = size[1] * KeyboardSession.shared.toolbarScale() * this.density;
      const margin: number = TOOLBAR_MARGIN_VP * this.density;
      this.toolbarPosition = [Math.round(this.screenWidth - width - margin),
        Math.round(this.screenHeight - height - margin * 4)];
      await toolbar.moveTo(this.toolbarPosition[0], this.toolbarPosition[1]);
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        if (this.toolbar !== toolbar) {
          await engine.destroyPanel(toolbar);
        }
        return;
      }
      await toolbar.show();
    } catch (error) {
      KeyboardLog.error(`toolbar unavailable: ${describeError(error)}`);
      if (toolbar !== undefined
        && (generation === this.statusPanelGeneration || this.toolbar !== toolbar)) {
        if (this.toolbar === toolbar) {
          this.toolbar = undefined;
          KeyboardSession.shared.onToolbarResize = undefined;
          KeyboardSession.shared.onToolbarDragStart = undefined;
          KeyboardSession.shared.onToolbarDrag = undefined;
        }
        engine.destroyPanel(toolbar);
      }
    }
  }

  private closeToolbar(): void {
    if (this.toolbar === undefined) {
      return;
    }
    KeyboardSession.shared.onToolbarResize = undefined;
    KeyboardSession.shared.onToolbarDragStart = undefined;
    KeyboardSession.shared.onToolbarDrag = undefined;
    engine.destroyPanel(this.toolbar);
    this.toolbar = undefined;
    this.toolbarHiddenForFullscreen = false;
  }

  private async openModeBadge(generation: number): Promise<void> {
    let hud: inputMethodEngine.Panel | undefined = undefined;
    try {
      hud = await engine.createPanel(this.context, {
        type: inputMethodEngine.PanelType.STATUS_BAR
      });
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        await engine.destroyPanel(hud);
        return;
      }
      await hud.setUiContent('keyboard/InputModeHud');
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        await engine.destroyPanel(hud);
        return;
      }
      this.hud = hud;
      ScreenCaptureProtection.protectPanel(hud, this.screenCaptureProtected(), 'mode badge');
      await this.resizeModeBadge(hud);
      if (this.tornDown || generation !== this.statusPanelGeneration) {
        if (this.hud !== hud) {
          await engine.destroyPanel(hud);
        }
      }
    } catch (error) {
      // Worth saying but not worth failing over: the keyboard types perfectly well without a badge, and refusing to start because a piece of feedback would not build helps nobody.
      KeyboardLog.error(`mode badge unavailable: ${describeError(error)}`);
      if (hud !== undefined
        && (generation === this.statusPanelGeneration || this.hud !== hud)) {
        if (this.hud === hud) this.hud = undefined;
        engine.destroyPanel(hud);
      }
    }
  }

  /** The badge's size in vp: the floating toolbar's font size and scale, fitted to the mark and one character. */
  private modeBadgeSize(): InputModeHudSize {
    return InputModeHudPolicy.sizeVp(KeyboardSession.shared.toolbarFontSize(),
      KeyboardSession.shared.toolbarScale());
  }

  private async resizeModeBadge(hud: inputMethodEngine.Panel): Promise<void> {
    const size: InputModeHudSize = this.modeBadgeSize();
    await hud.resize(Math.round(size.widthVp * this.density), Math.round(size.heightVp * this.density));
  }

  private closeModeBadge(): void {
    if (this.hudTimer >= 0) {
      clearTimeout(this.hudTimer);
      this.hudTimer = -1;
    }
    if (this.hud !== undefined) {
      engine.destroyPanel(this.hud);
      this.hud = undefined;
    }
  }

  /** The settings page saved new preferences and the session has been rebuilt from them. */
  private preferencesReloaded(): void {
    this.modeKeys.use(KeyboardSession.shared.modeBindings());
    if (!KeyboardFormFactor.isDesktop() || this.panel === undefined) {
      return;
    }
    this.reconcileStatusPanels().catch((error: Object): void => {
      KeyboardLog.error(`status panels not reconciled: ${describeError(error)}`);
    });
  }

  private beginToolbarDrag(): void {
    this.toolbarDragOrigin = [this.toolbarPosition[0], this.toolbarPosition[1]];
  }

  private dragToolbar(offsetX: number, offsetY: number): void {
    if (this.toolbar === undefined) {
      return;
    }
    const session: KeyboardSession = KeyboardSession.shared;
    const size: number[] = session.toolbarSize();
    const scale: number = session.toolbarScale();
    const width: number = size[0] * scale * this.density;
    const height: number = size[1] * scale * this.density;
    const position: [number, number] = FloatingToolbarDragPolicy.position(
      this.toolbarDragOrigin, [offsetX, offsetY], this.density, this.screenWidth,
      this.screenHeight, width, height);
    this.toolbarPosition = position;
    this.toolbar.moveTo(position[0], position[1]).catch((error: Object): void => {
      KeyboardLog.error(`toolbar would not move: ${describeError(error)}`);
    });
  }

  /** 存着的浮动键盘开关此刻是否生效（`FloatingKeyboardPolicy.active`）。 */
  private floatingWanted(): boolean {
    return FloatingKeyboardPolicy.active(KeyboardSession.shared.feedbackSettings().floating,
      KeyboardFormFactor.isDesktop());
  }

  private storedFloatingFraction(): [number, number] {
    const feedback: FeedbackSettings = KeyboardSession.shared.feedbackSettings();
    return [feedback.floatingX, feedback.floatingY];
  }

  /** 浮动面板此刻的大小（px）：宽度按屏幕宽度（`FloatingKeyboardPolicy.widthVp`），高度是视图要的键盘高度加拖动条。 */
  private floatingSize(): [number, number] {
    return [
      Math.round(FloatingKeyboardPolicy.widthVp(this.screenWidth / this.density) * this.density),
      Math.round(FloatingKeyboardPolicy.panelHeightVp(this.touchHeightVp) * this.density)
    ];
  }

  /**
   * 按面板此刻的标志定大小。停靠时与加入浮动键盘之前一样，宽度是创建面板时的屏幕宽度；浮动时宽度按屏幕宽度取 80%，高度加上拖动条，再按存着的千分比摆进屏幕（拖动中不摆，免得和手指抢位置）。`moveTo` 对 `FLG_FIXED` 面板不可用，所以只在浮动时调用。
   */
  private async fitTouchPanel(panel: inputMethodEngine.Panel): Promise<void> {
    if (!this.floating) {
      await panel.resize(this.fixedWidth, Math.round(this.touchHeightVp * this.density));
      return;
    }
    const size: [number, number] = this.floatingSize();
    await panel.resize(size[0], size[1]);
    if (this.floatingDragging) {
      return;
    }
    this.floatingPosition = FloatingKeyboardPolicy.position(this.floatingFraction[0], this.floatingFraction[1],
      this.screenWidth, this.screenHeight, size[0], size[1]);
    await panel.moveTo(this.floatingPosition[0], this.floatingPosition[1]);
  }

  /** 功能面板的「浮动键盘」磁贴或拖动条的停靠键改了开关：按顺序排进 `floatingChange`，一次换完一个。 */
  private followFloatingSetting(panel: inputMethodEngine.Panel): void {
    this.floatingChange = this.floatingChange
      .then((): Promise<void> => this.applyFloatingSetting(panel))
      .catch((error: Object): void => {
        KeyboardLog.error(`floating keyboard not applied: ${describeError(error)}`);
      });
  }

  /**
   * 换面板标志：`changeFlag` 要求先隐藏面板，换好之后按新标志定大小和位置，再显示回来。换不成功也要显示回来，编辑框还在等键盘；这时面板保持原来的标志，`floating` 不变。
   */
  private async applyFloatingSetting(panel: inputMethodEngine.Panel): Promise<void> {
    const wanted: boolean = this.floatingWanted();
    if (this.panel !== panel || this.tornDown || wanted === this.floating) {
      return;
    }
    this.floatingDragging = false;
    await panel.hide();
    try {
      panel.changeFlag(wanted ? inputMethodEngine.PanelFlag.FLG_FLOATING : inputMethodEngine.PanelFlag.FLG_FIXED);
      this.floating = wanted;
      if (wanted) {
        this.floatingFraction = this.storedFloatingFraction();
      }
      await this.fitTouchPanel(panel);
    } finally {
      if (this.panel === panel && !this.tornDown) {
        await panel.show();
      }
    }
  }

  /** 按下拖动条：记下面板此刻的位置，之后的位移都从这里算。 */
  private beginFloatingDrag(): void {
    if (!this.floating) {
      return;
    }
    this.floatingDragging = true;
    this.floatingDragOrigin = [this.floatingPosition[0], this.floatingPosition[1]];
  }

  /** 拖动中：位移是相对按下那一刻的屏幕位移（vp），换成像素加到起点上，钳进屏幕（`FloatingKeyboardPolicy.dragged`）再移动面板。 */
  private dragFloating(offsetXVp: number, offsetYVp: number): void {
    const panel: inputMethodEngine.Panel | undefined = this.panel;
    if (!this.floatingDragging || panel === undefined) {
      return;
    }
    const size: [number, number] = this.floatingSize();
    this.floatingPosition = [
      Math.round(FloatingKeyboardPolicy.dragged(this.floatingDragOrigin[0], offsetXVp * this.density,
        this.screenWidth - size[0])),
      Math.round(FloatingKeyboardPolicy.dragged(this.floatingDragOrigin[1], offsetYVp * this.density,
        this.screenHeight - size[1]))
    ];
    panel.moveTo(this.floatingPosition[0], this.floatingPosition[1]).catch((error: Object): void => {
      KeyboardLog.error(`floating keyboard would not move: ${describeError(error)}`);
    });
  }

  /** 松手：位置换成千分比记下，并写进设备本地设置，下次显示、转屏或键盘变高时都按它摆。 */
  private endFloatingDrag(): void {
    if (!this.floatingDragging) {
      return;
    }
    this.floatingDragging = false;
    const size: [number, number] = this.floatingSize();
    this.floatingFraction = [
      FloatingKeyboardPolicy.fractionOf(this.floatingPosition[0], this.screenWidth - size[0]),
      FloatingKeyboardPolicy.fractionOf(this.floatingPosition[1], this.screenHeight - size[1])
    ];
    KeyboardSession.shared.saveFloatingPosition(this.floatingFraction[0], this.floatingFraction[1]);
  }

  /** 转屏或折叠屏开合：记下新的屏幕尺寸，浮动面板按存着的千分比重新定宽、定位置。停靠面板照旧。 */
  private readonly displayChanged: (displayId: number) => void = (_displayId: number): void => {
    const panel: inputMethodEngine.Panel | undefined = this.panel;
    if (panel === undefined) {
      return;
    }
    let shown: display.Display;
    try {
      shown = display.getDefaultDisplaySync();
    } catch (error) {
      KeyboardLog.error(`display unavailable: ${describeError(error)}`);
      return;
    }
    this.screenWidth = shown.width;
    this.screenHeight = shown.height;
    this.density = shown.densityPixels;
    if (!this.floating) {
      return;
    }
    this.fitTouchPanel(panel).catch((error: Object): void => {
      KeyboardLog.error(`floating keyboard would not follow the display: ${describeError(error)}`);
    });
  };

  /**
   * Open, close or swap the surface the candidate window is carrying.
   *
   * The same window, because there is one: a keyboard and a grid of emoji are both too big to sit
   * where the candidates do, so it is resized and moved rather than duplicated. Closing puts it back
   * to the size a candidate list needs, so the next composition does not open a keyboard-sized hole.
   */
  private showDesktopSurface(surface: DesktopSurface): void {
    const panel: inputMethodEngine.Panel | undefined = this.panel;
    if (panel === undefined) {
      return;
    }
    if (surface === DesktopSurface.NONE) {
      this.resizeCandidateWindow(surface).then((): void => {
        if (!KeyboardSession.shared.composing()) {
          panel.hide();
        }
      }).catch((error: Object): void => {
        KeyboardLog.error(`surface would not close: ${describeError(error)}`);
      });
      return;
    }
    // Shown before it is sized, not after: a panel that is not on screen takes a resize without
    // complaint and then appears at whatever size it had before, which on screen is a candidate
    // strip where a keyboard was asked for.
    panel.show().then((): Promise<void> => this.resizeCandidateWindow(surface)).then((): void => {
      this.centreCandidateWindow(surface);
    }).catch((error: Object): void => {
      KeyboardLog.error(`surface would not open: ${describeError(error)}`);
    });
  }

  /** A candidate list is one strip; the other two are as tall as the keyboard they stand in for. */
  private async resizeCandidateWindow(surface: DesktopSurface): Promise<void> {
    if (this.panel === undefined) {
      return;
    }
    const settings: KeyboardPreferences = KeyboardSession.shared.preferences();
    const tall: boolean = surface !== DesktopSurface.NONE;
    const heightVp: number = tall
      ? KeyboardMetrics.totalHeightVp(settings.rowSpacingTenths, settings.heightAdjustment,
        KeyboardSession.shared.glossRows(), settings.candidateFontSize,
        settings.candidatePreeditFontSize, true)
      : (KeyboardMetrics.candidateHeightVp(settings.candidateLayout, this.candidateCount,
        settings.candidateShowPreedit, settings.candidateDecorationTopVp,
        KeyboardSession.shared.glossRows(), settings.candidateFontSize,
        settings.candidatePreeditFontSize, CandidateLayoutPolicy.gap(settings.candidateSkinLayout, true,
        settings.candidateFontSize)) + CandidateCanvasPolicy.extraHeight(settings.candidateCanvas, settings.candidateShowPreedit));
    const widthVp: number = tall ? DESKTOP_SURFACE_WIDTH_VP : this.candidateWidthVp;
    await this.panel.resize(Math.round(widthVp * this.density),
      Math.round(heightVp * this.density));
  }

  /**
   * A surface that was asked for goes where it can be seen rather than where the caret is.
   *
   * The caret is the right place for candidates, which belong to the word being typed. A keyboard or
   * a grid of emoji belongs to the session, so it sits at the bottom of the screen the way one does
   * on a phone, out of the way of the text above it.
   */
  private centreCandidateWindow(surface: DesktopSurface): void {
    if (this.panel === undefined || this.screenWidth === 0) {
      return;
    }
    const settings: KeyboardPreferences = KeyboardSession.shared.preferences();
    const height: number = KeyboardMetrics.totalHeightVp(
      settings.rowSpacingTenths, settings.heightAdjustment,
      KeyboardSession.shared.glossRows(), settings.candidateFontSize,
      settings.candidatePreeditFontSize, true) * this.density;
    const width: number = DESKTOP_SURFACE_WIDTH_VP * this.density;
    this.panel.moveTo(Math.round((this.screenWidth - width) / 2),
      Math.round(this.screenHeight - height - TOOLBAR_MARGIN_VP * this.density))
      .catch((error: Object): void => {
        KeyboardLog.error(`surface would not move: ${describeError(error)}`);
      });
  }

  /**
   * 齿轮按钮打开应用，按键以外的一切都在应用里。
   *
   * `page` 是键盘按名称请求的设置页（功能面板的「词库」「反馈」和「关于」磁贴）；`EntryAbility` 从 want 的 `page` 参数读取它，并打开该页而不是根页面，冷启动和应用已在运行时都是如此。`target` 是那一页里的子目标（长按九键左侧符号栏时是那张表的共享偏好名），放在 want 的 `target` 参数里，只随 `page` 一起发。
   */
  private openSettings(page: string | undefined, requestMicrophone: boolean = false, target?: string): void {
    // 直接写明名称而不是从 context 读取：扩展的 context 不带 ability 信息，而 bundle 按构造就是本应用——面板和设置窗口属于同一个应用。
    const want: Want = { bundleName: BUNDLE_NAME, abilityName: 'EntryAbility' };
    if (page !== undefined) {
      const parameters: Record<string, Object> = { 'page': page };
      if (target !== undefined) {
        parameters['target'] = target;
      }
      want.parameters = parameters;
    }
    if (requestMicrophone) {
      want.parameters = { 'requestMicrophone': true };
    }
    this.context.startAbility(want).catch((error: Object): void => {
      KeyboardLog.error(`settings would not open: ${describeError(error)}`);
    });
  }

  /**
   * Flip the composing language and say so.
   *
   * The badge is the only feedback there is on a machine with no keyboard on screen to change shape:
   * nothing else on a desktop tells the user which mode the next letter will be in.
   */
  private switchInputMode(): void {
    this.modeKeys.disarm();
    const english: boolean = KeyboardSession.shared.toggleLanguage();
    KeyboardSession.shared.refreshToolbar();
    this.showModeBadge(english);
  }

  /**
   * Show the badge, then take it away again.
   *
   * Long enough to be read at a glance, short enough that it is gone before the next word is typed.
   * A switch arriving while one is still up restarts the wait rather than stacking a second timer.
   */
  private showModeBadge(english: boolean): void {
    if (this.hud === undefined) {
      return;
    }
    const panel: inputMethodEngine.Panel = this.hud;
    KeyboardSession.shared.setModeBadge(english);
    this.placeModeBadge();
    panel.show().catch((error: Object): void => {
      KeyboardLog.error(`mode badge would not show: ${describeError(error)}`);
    });
    if (this.hudTimer >= 0) {
      clearTimeout(this.hudTimer);
    }
    this.hudTimer = setTimeout((): void => {
      this.hudTimer = -1;
      panel.hide().catch((error: Object): void => {
        KeyboardLog.error(`mode badge would not hide: ${describeError(error)}`);
      });
    }, HUD_VISIBLE_MS);
  }

  /** Under the caret, clamped inside the screen; centred low when the editor reports no caret. */
  private placeModeBadge(): void {
    if (this.hud === undefined) {
      return;
    }
    const size: InputModeHudSize = this.modeBadgeSize();
    const width: number = size.widthVp * this.density;
    const height: number = size.heightVp * this.density;
    const margin: number = HUD_SCREEN_MARGIN_VP * this.density;
    let x: number = this.caret[2] > 0
      ? this.caret[0] + this.caret[2] / 2 - width / 2
      : this.screenWidth / 2 - width / 2;
    let y: number = this.caret[2] > 0
      ? this.caret[1] + this.caret[2] + HUD_CARET_GAP_VP * this.density
      : this.screenHeight * 2 / 3;
    x = KeyboardGeometry.bounded(x, margin, Math.max(margin, this.screenWidth - margin - width));
    y = KeyboardGeometry.bounded(y, margin, Math.max(margin, this.screenHeight - margin - height));
    this.hud.moveTo(Math.round(x), Math.round(y)).catch((error: Object): void => {
      KeyboardLog.error(`mode badge would not move: ${describeError(error)}`);
    });
  }

  /**
   * Put the candidate window just below the caret, kept inside the screen.
   *
   * Below by default, because that is where the eye goes next; above when there is no room beneath,
   * so the window never hangs off the bottom edge. The same clamp on the other axis keeps a caret
   * near the right edge from pushing the window off the side.
   */
  private placeCandidateWindow(): void {
    if (this.panel === undefined) {
      return;
    }
    const width: number = this.candidateWidthVp * this.density;
    const settings: KeyboardPreferences = KeyboardSession.shared.preferences();
    const height: number = (KeyboardMetrics.candidateHeightVp(settings.candidateLayout,
      this.candidateCount, settings.candidateShowPreedit,
      settings.candidateDecorationTopVp, KeyboardSession.shared.glossRows(),
      settings.candidateFontSize, settings.candidatePreeditFontSize, CandidateLayoutPolicy.gap(settings.candidateSkinLayout, true,
        settings.candidateFontSize)) + CandidateCanvasPolicy.extraHeight(settings.candidateCanvas, settings.candidateShowPreedit)) * this.density;
    const gap: number = KeyboardMetrics.CANDIDATE_WINDOW_GAP_VP * this.density;
    const anchor: CandidateAnchor = CandidateAnchorPolicy.position(settings.candidateFollowCursor,
      this.candidateAnchor, this.caret);
    // A caret of no height is one that was never reported: a web view mid-layout, a client that
    // answers with zeroes, or simply an editor that has not sent one yet. Believing it puts the
    // window in the top-left corner of the screen, which is where this went. Low and centred is
    // where a desktop input method puts a candidate list it cannot place precisely.
    if (anchor[2] <= 0) {
      this.panel.moveTo(Math.round(Math.max(0, (this.screenWidth - width) / 2)),
        Math.round(Math.max(0, this.screenHeight * 2 / 3)))
        .catch((error: Object): void => {
          KeyboardLog.error(`candidate window would not move: ${describeError(error)}`);
        });
      return;
    }
    let x: number = anchor[0];
    if (this.screenWidth > 0 && x + width > this.screenWidth) {
      x = this.screenWidth - width;
    }
    let y: number = anchor[1] + anchor[2] + gap;
    if (this.screenHeight > 0 && y + height > this.screenHeight) {
      y = anchor[1] - height - gap;
    }
    this.panel.moveTo(Math.round(Math.max(0, x)), Math.round(Math.max(0, y)))
      .catch((error: Object): void => {
        KeyboardLog.error(`candidate window would not move: ${describeError(error)}`);
      });
  }

  // off('inputStop') requires the same callback reference that on() received, unlike off('inputStart')
  // where it is optional. Holding both as fields keeps unsubscribing honest.
  private readonly started: (kbController: inputMethodEngine.KeyboardController,
                             inputClient: inputMethodEngine.InputClient) => void =
    (kbController: inputMethodEngine.KeyboardController,
     inputClient: inputMethodEngine.InputClient): void => {
      // A keyboard process can live for days; this sends the new day's active at most every few hours, off this thread.
      HarmonyTelemetry.keyboardShown();
      if (this.panel === undefined) {
        this.pending = inputClient;
        return;
      }
      this.pending = undefined;
      // A caret belongs to the editor that reported it. Carrying one across a focus change puts the
      // next composition beside the last editor's text, which is worse than not knowing at all.
      this.caret = [0, 0, 0];
      KeyboardSession.shared.attach(inputClient);
      this.applyScreenCaptureProtection();
      // A keyboard belongs on screen the moment an editor takes focus; a candidate window has
      // nothing to offer until something is being spelled, and waits for onComposition to say so.
      if (!KeyboardFormFactor.isDesktop()) {
        this.panel.show();
      } else {
        this.followCallingWindow(inputClient);
      }
    };

  /**
   * Keep the floating toolbar off an application that fills the screen, and bring it back when the focus is in a window that does not.
   *
   * The Windows host hides its toolbar while the foreground window is fullscreen (`OnWinEvent` in `window_hook.cpp`), so a game or a video is not covered by a bar nobody is using. HarmonyOS tells the keyboard the calling window's status, and only when asked, so it is asked each time an editor takes focus; a window that goes fullscreen while its editor keeps the focus is noticed at the next focus.
   */
  private followCallingWindow(inputClient: inputMethodEngine.InputClient): void {
    const toolbar: inputMethodEngine.Panel | undefined = this.toolbar;
    if (toolbar === undefined) {
      return;
    }
    inputClient.getCallingWindowInfo().then((info: inputMethodEngine.WindowInfo): Promise<void> | undefined => {
      const fullscreen: boolean = info.status === window.WindowStatusType.FULL_SCREEN;
      if (this.toolbar !== toolbar || fullscreen === this.toolbarHiddenForFullscreen) {
        return undefined;
      }
      this.toolbarHiddenForFullscreen = fullscreen;
      return fullscreen ? toolbar.hide() : toolbar.show();
    }).catch((error: Object): void => {
      KeyboardLog.error(`calling window unavailable: ${describeError(error)}`);
    });
  }

  private readonly keyboardShown: () => void = (): void => {
    KeyboardSession.shared.keyboardShown();
  };

  // 系统收起键盘（返回手势、点空白处）时编辑框不一定失去焦点，等不到 inputStop；会话在这里停掉键盘内的语音识别，把攒着的打字统计写盘，关掉缓存的释义词库。候选皮肤停放的位图也在这里释放，隐藏的键盘不留着它们。
  private readonly keyboardHidden: () => void = (): void => {
    KeyboardSession.shared.keyboardHidden();
    CandidateCanvasDecoder.trim();
  };

  private readonly stopped: () => void = (): void => {
    // An editor that stops while the panel is still being built must not be attached once it is.
    this.pending = undefined;
    this.backspaceHold.reset();
    this.heldKeys.reset();
    KeyboardSession.shared.detach();
    CandidateCanvasDecoder.trim();
    if (this.panel !== undefined) {
      this.panel.hide();
    }
  };

  /**
   * A key from a real keyboard, on a machine that has one.
   *
   * Returning false hands the key to the application untouched, which is what happens to everything
   * the router does not claim. Only key-down is acted on: a key that was claimed going down must be
   * claimed coming up too, or the application sees half of it.
   */
  /** Key codes whose key-down went back to the application, so their key-up must follow it. */
  private readonly releasedKeys: Set<number> = new Set<number>();
  /** Key codes whose key-down the emoji panel took, so their key-up is taken too. */
  private readonly panelKeys: Set<number> = new Set<number>();

  private readonly typed: (event: InputKeyEvent) => boolean = (event: InputKeyEvent): boolean => {
    const session: KeyboardSession = KeyboardSession.shared;
    // Whether this key-down is a new press rather than a repeat of a key still held, which OpenHarmony delivers as another key-down.
    let newPress: boolean = false;
    // Counted for the key heatmap before anything claims or releases the key, so a key the application types counts the same as one the Engine composes, and a modifier or shortcut chord counts as the presses it is. The session declines for a password field.
    if (event.action === KeyAction.DOWN) {
      newPress = this.heldKeys.press(event.key.code);
      if (newPress) {
        session.recordKey(KeyIdPolicy.hardware(event.key.code));
      }
    } else {
      this.heldKeys.release(event.key.code);
    }
    // 韩语用字母大小写区分字母本身（Shift+R 是 ㄲ），所以只看 Shift：忘了关的 Caps Lock 不能把每个 ㄱ 都变成 ㄲ。越南语和藏文（威利转写区分大小写）按输入时的大小写写字母，所以 Caps Lock 在那里就是大写，和任何编辑器一样。
    const key: HardwareKey = HardwareKeyRouter.normalizeLetterCase({
      keyCode: event.key.code, unicodeChar: event.unicodeChar, ctrlKey: event.ctrlKey,
      altKey: event.altKey, logoKey: event.logoKey, shiftKey: event.shiftKey
    }, event.capsLock && !session.isKorean() && session.rulesScheme() !== SchemeTraits.VIETNAMESE
      && session.rulesScheme() !== SchemeTraits.TIBETAN);
    session.setCapsLock(event.capsLock);
    const mode: ModeKey = {
      keyCode: event.key.code, down: event.action === KeyAction.DOWN, shiftKey: event.shiftKey,
      ctrlKey: event.ctrlKey, altKey: event.altKey, logoKey: event.logoKey,
      timestamp: Date.now()
    };
    // The voice shortcuts come first and apply whether or not anything is being composed: Windows
    // records into whatever has focus, and a hold key that only worked mid-composition would be a
    // shortcut nobody could start. Escape and Space are claimed only while recording, so they go on
    // meaning what they mean to a composition the rest of the time.
    const voiceKey: VoiceKey = {
      keyCode: event.key.code, down: event.action === KeyAction.DOWN, ctrlKey: event.ctrlKey,
      altKey: event.altKey, shiftKey: event.shiftKey, logoKey: event.logoKey
    };
    if (session.routeVoiceHotkey(voiceKey)) {
      return true;
    }
    // The panel chord, like the voice ones, applies whether or not anything is being composed: the
    // on-screen keyboard exists for the character the physical keyboard has no key for, and needing
    // to be mid-composition to ask for it would put it out of reach exactly when it is wanted.
    const panelKey: PanelShortcutKey = {
      keyCode: event.key.code, down: event.action === KeyAction.DOWN, ctrlKey: event.ctrlKey,
      shiftKey: event.shiftKey, altKey: event.altKey, logoKey: event.logoKey
    };
    if (PanelShortcutPolicy.claims(panelKey, KeyboardFormFactor.isDesktop())) {
      const surface: PanelSurfaceAction =
        PanelShortcutPolicy.action(PanelShortcutPolicy.shortcut(panelKey,
          KeyboardFormFactor.isDesktop()));
      if (surface !== PanelSurfaceAction.NONE) {
        // Through the same entry the toolbar button uses, not the window helper underneath it. That
        // helper shows and sizes a panel but never records which surface is open, so the view went
        // on drawing candidates and the window was sized around them: a thin empty bar where a
        // keyboard had been asked for. Asking for the open surface still closes it, because that is
        // what this entry does.
        session.runToolbarButton(surface as number as ToolbarButton);
      }
      // The release is claimed too, or a bare K lands in the editor after the panel has opened.
      return true;
    }
    // Switching mode is the one thing worth claiming in an editor this keyboard will not compose
    // for: the mode outlives the field, and a password box is exactly where one notices being in the
    // wrong one. Checked before anything else, so a chord meant for the mode never reaches the
    // Engine as the letter it is built on.
    const gesture: ModeGesture = this.modeKeys.accept(mode);
    if (gesture === ModeGesture.SWITCH_LANGUAGE) {
      this.switchInputMode();
      return true;
    }
    if (gesture === ModeGesture.TOGGLE_CHARACTER_SET) {
      // Windows only switches while composing Chinese, but claims the chord either way (`HandleImeKey`, `IsCharacterSetInputModeToggle`).
      if (!session.isEnglish()) {
        session.toggleCharacterSet();
        KeyboardSession.shared.refreshToolbar();
      }
      return true;
    }
    if (gesture === ModeGesture.ENGLISH_CANDIDATES) {
      this.modeKeys.disarm();
      this.showModeBadge(session.toggleEnglishCandidates());
      KeyboardSession.shared.refreshToolbar();
      return true;
    }
    if (gesture === ModeGesture.TOGGLE_WIDTH) {
      session.toggleCharacterWidth();
      return true;
    }
    if (gesture === ModeGesture.TOGGLE_PUNCTUATION) {
      session.togglePunctuation();
      KeyboardSession.shared.refreshToolbar();
      return true;
    }
    // The 2in1 emoji panel takes navigation, activation and search keys the way the focused Windows panel does (`EmojiPanel::OnKeyDown`); its key-up follows, so the editor never sees half of one.
    if (event.action !== KeyAction.DOWN && this.panelKeys.delete(event.key.code)) {
      return true;
    }
    if (event.action === KeyAction.DOWN && session.routePanelKey({
      keyCode: event.key.code, unicodeChar: event.unicodeChar, ctrlKey: event.ctrlKey,
      altKey: event.altKey, logoKey: event.logoKey
    })) {
      this.panelKeys.add(event.key.code);
      return true;
    }
    // A space with nothing being composed is the editor's, with one exception: it is also the key
    // that rewrites a just-committed Chinese mark as ASCII, and that arming only exists after a
    // commit — which is to say, only when nothing is being composed.
    if (event.action === KeyAction.DOWN && session.convertPunctuationSpace(event.unicodeChar)) {
      return true;
    }
    // The other half of a letter the Engine declined going down: the application saw that key-down, so it gets the key-up too.
    if (event.action !== KeyAction.DOWN && this.releasedKeys.delete(event.key.code)) {
      return false;
    }
    // A Backspace held from inside a composition stops where the composition did; see HardwareBackspaceGuard.
    if (event.key.code === KEYCODE_DEL) {
      const held: boolean = event.action === KeyAction.DOWN
        ? this.backspaceHold.down(session.composing()) : this.backspaceHold.up(session.composing());
      if (held) {
        return true;
      }
    } else if (event.action === KeyAction.DOWN) {
      this.backspaceHold.reset();
    }
    // 按键音属于这次敲击，与这个键最后被组字、交还还是放给应用无关，所以从这里起每次按下都出声。实体键只在 2in1 上出声（触屏键经 `KeyboardSession.softKeyPress` 出声），会话让密码框和英文模式保持安静。
    const sound = (): void => {
      if (event.action === KeyAction.DOWN && KeyboardFormFactor.isDesktop()) {
        session.hardwareKeySound(event.key.code, event.unicodeChar, event.ctrlKey, event.altKey,
          event.logoKey);
      }
    };
    if (!session.composes()) {
      sound();
      return false;
    }
    const decision: HardwareKeyDecision =
      HardwareKeyRouter.route(key, session.composing(), !session.isEnglish() || session.isEnglishCandidates(),
        KeyboardSession.shared.preferences().numberRowSelection,
      KeyboardSession.shared.preferences().navigation,
        session.hasHighlightedTranslation(), session.isJapanese(),
        session.wordCharacterBinding(), session.hasHighlightedCandidate(), session.hardwareSpelling(),
        session.isFullWidth(), session.punctuationLockedChinese(), session.isKorean(),
        session.candidateListOpen(), session.rulesScheme());
    if (decision.action === HardwareKeyAction.RELEASE) {
      // What the application types is still the user's typing, and Windows counts it (`stats_passthrough.h`).
      if (event.action === KeyAction.DOWN) {
        session.recordPassthrough(event.unicodeChar, event.ctrlKey, event.altKey, event.logoKey);
      }
      sound();
      return false;
    }
    if (event.action !== KeyAction.DOWN) {
      return true;
    }
    // A held Hanja key converts once rather than opening and closing the list with every repeat.
    if (decision.action === HardwareKeyAction.CONVERT_HANJA && !newPress) {
      return true;
    }
    const applied: boolean = HardwareKeyDispatch.apply(decision, key.shiftKey, session);
    // After the key has done its work, so a commit it caused is heard with it.
    sound();
    if (!applied) {
      // Declined rather than swallowed, the way TSF treats a key the Windows host did not eat: a capital letter with nothing composed is the application's to type.
      this.releasedKeys.add(event.key.code);
      session.recordPassthrough(event.unicodeChar, event.ctrlKey, event.altKey, event.logoKey);
      return false;
    }
    return true;
  };

  /**
   * Routes physical keys for as long as there are physical keys to route.
   *
   * The hot-plug listener is registered before the first enumeration so that a keyboard attached
   * during startup is not missed in the gap between the two, and the enumeration is applied on top
   * of whatever the listener has already recorded rather than replacing it.
   */
  private routeKeysWhenAttached(): void {
    if (KeyboardFormFactor.isDesktop()) {
      this.applyKeyRouting();
      return;
    }
    HarmonyHardwareKeyboards.watch((change: DeviceChange): void => {
      this.attachedKeyboards = HardwareKeyboardPolicy.applyChange(this.attachedKeyboards, change);
      this.applyKeyRouting();
    });
    HarmonyHardwareKeyboards.list((devices: AttachedInputDevice[]): void => {
      for (const device of devices) {
        this.attachedKeyboards = HardwareKeyboardPolicy.applyChange(this.attachedKeyboards,
          { type: 'add', deviceId: device.deviceId, keyboardType: device.keyboardType });
      }
      this.applyKeyRouting();
    });
  }

  /** Brings the `keyEvent` subscription in line with what is attached, and only when it differs. */
  private applyKeyRouting(): void {
    if (this.tornDown) {
      return;
    }
    const route: boolean =
      HardwareKeyboardPolicy.routes(KeyboardFormFactor.isDesktop(), this.attachedKeyboards);
    const transition: KeyRoutingTransition | null =
      HardwareKeyboardPolicy.transition(this.routingKeys, route);
    if (transition === null) {
      return;
    }
    try {
      if (transition === KeyRoutingTransition.SUBSCRIBE) {
        inputMethodEngine.getKeyboardDelegate().on('keyEvent', this.typed);
        this.routingKeys = true;
        KeyboardLog.info(`routing hardware keys: ${this.attachedKeyboards.length} device(s)`);
        return;
      }
      // off('keyEvent') wants the same callback reference on() was given, which is why `typed` is a
      // property and not a method.
      inputMethodEngine.getKeyboardDelegate().off('keyEvent', this.typed);
      this.routingKeys = false;
      KeyboardLog.info('hardware keys released: no keyboard attached');
    } catch (error) {
      KeyboardLog.error(`key routing ${transition} refused: ${describeError(error)}`);
    }
  }

  /** 设置应用里的「防止截屏」，读本进程 `files/state` 里经共享沙箱镜像过来的那一份；拿不到共享目录时没有这份文件，按关闭处理。 */
  private screenCaptureProtected(): boolean {
    return ScreenCaptureProtection.load(`${this.context.filesDir}/state`).enabled;
  }

  /**
   * 「防止截屏」落到键盘的每一块面板上：软键盘（2in1 上是候选窗）、浮动工具条和输入模式提示。每次编辑框获得焦点、`attach` 导入了设置应用的镜像之后调用，所以在设置应用里切换后，下一次弹出键盘就生效；面板新建时也各自设一次（`openToolbar`、`openModeBadge`、`onCreate`）。值没变的面板不再调用窗口管理（`ScreenCaptureProtection.protectPanel` 记着每块面板上次设的值），焦点路径上通常一次 IPC 也没有。
   */
  private applyScreenCaptureProtection(): void {
    const enabled: boolean = this.screenCaptureProtected();
    if (this.panel !== undefined) {
      ScreenCaptureProtection.protectPanel(this.panel, enabled,
        KeyboardFormFactor.isDesktop() ? 'candidate window' : 'soft keyboard');
    }
    if (this.toolbar !== undefined) ScreenCaptureProtection.protectPanel(this.toolbar, enabled, 'toolbar');
    if (this.hud !== undefined) ScreenCaptureProtection.protectPanel(this.hud, enabled, 'mode badge');
  }

  /** 用户是否给了键盘完整体验模式。基础访问模式下键盘对共享沙箱只读，只导入、不发布。 */
  private fullAccessMode(): boolean {
    try {
      return engine.getSecurityMode() === inputMethodEngine.SecurityMode.FULL;
    } catch (error) {
      KeyboardLog.warn(`security mode unavailable: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * Copies the packaged dictionary into the sandbox once, and only once.
   *
   * stage-resources.sh puts the pinned release under resfile/engine, which OpenHarmony extracts to
   * resourceDir at install time, and reading those files with @ohos.file.fs works. SQLite does not:
   * opening a database on that read-only bundle mount fails even for a read-only connection, because
   * the mount does not carry the advisory locks SQLite takes, and the Engine reports it as "Unable to
   * copy runtime dictionary". So the databases are copied to filesDir, where SQLite is on ordinary
   * storage, and the Engine derives its generation from there.
   *
   * The marker sits beside the directory rather than inside it: prepare_host verifies the resource
   * directory against the pinned lock and rejects any file the lock does not name.
   */
  async onCreate(want: Want): Promise<void> {
    try {
      const files: string = this.context.filesDir;
      const resources: string = `${files}/engine`;
      const state: string = `${files}/state`;
      if (!fs.accessSync(state)) {
        fs.mkdirSync(state, true);
      }
      // 设置应用经共享沙箱发布的偏好、皮肤、插件等，在会话第一次读它们之前导入；拿不到共享目录时照旧只用自己的 files/state。
      const fullMode = (): boolean => this.fullAccessMode();
      const resolving: Promise<SharedStateMirror | null> = SharedStateMirror.resolve(this.context)
        .then((groupDir: string | null): SharedStateMirror | null => {
          const status = HarmonySharedSandboxPolicy.status(groupDir);
          return status.available ? new SharedStateMirror(status.directory, files) : null;
        });
      // 在拿到共享目录之前就占住会话镜像的队列：设置应用发布的会话要先采用过来，本进程的认证请求（匿名注册、云剪贴板）都排在它后面（迁移差异 M2）。
      AccountSessionAdoption.adoptOnStart(resolving, state, 'settings', fullMode);
      const mirror: SharedStateMirror | null = await resolving;
      if (mirror !== null) {
        KeyboardSession.shared.useSharedSandbox(mirror, fullMode);
        SharedStateSync.keyboardSync(mirror, state, fullMode());
      }
      // 隐私政策的授权由设置应用经共享沙箱镜像过来，上面那次导入之后读。没有授权时下面的匿名注册和使用统计都等着，键盘里的云端功能也不发请求；之后每次获得焦点导入镜像时再读一次（`KeyboardSession.attach`）。
      HarmonyPrivacyConsent.load(state);
      // 覆盖从未打开应用就启用键盘的场景。完整体验模式下键盘获得焦点或编辑结束时发布这个匿名会话，设置应用第一次启动时本地还没有匿名会话，由 session_adopt 采用它，不再注册第二个；键盘发布之前就打开了设置应用、或者键盘在基础访问模式下时，设置应用仍会注册自己的那个。
      HarmonyAccountIdentity.ensureAnonymous(state);
      // 每个键盘进程只启动一个用量上报会话；在下方提前返回之前创建，让没有面板的进程也能上报。
      HarmonyTelemetry.keyboardStarted(state);
      if (!StagedResources.stage(`${this.context.resourceDir}/engine`, resources,
          `${files}/engine.ready`)) {
        return;
      }
      // 可选的非英文候选释义暂存到引擎资源旁的普通存储目录，满足 SQLite 要求；当前包不带释义时移除旧文件，不阻止键盘启动。
      const glosses: string = `${this.context.resourceDir}/offline-glosses`;
      if (fs.accessSync(glosses)) {
        StagedResources.stage(glosses, `${files}/offline-glosses`, `${files}/offline-glosses.ready`);
      } else {
        StagedResources.unstage(`${files}/offline-glosses`, `${files}/offline-glosses.ready`);
      }
      // 候选英文音标表同样暂存到引擎资源旁，共享层在 pronunciations/en-phonetic.db 找它；包里没带时移除旧表，「显示英文音标」就不预留那一行。
      const pronunciations: string = `${this.context.resourceDir}/pronunciations`;
      if (fs.accessSync(pronunciations)) {
        StagedResources.stage(pronunciations, `${files}/pronunciations`, `${files}/pronunciations.ready`);
      } else {
        StagedResources.unstage(`${files}/pronunciations`, `${files}/pronunciations.ready`);
      }
      StagedResources.stageLanguageDictionaries(this.context.resourceDir, files);
      StagedResources.stageHandwriting(this.context.resourceDir, files);
      // 扩展上下文提供系统颜色模式，视图本身没有可查询的上下文。
      const systemDark: boolean =
        this.context.config.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
      // 通过该上下文读取随包录音提示音，会话本身不持有上下文。
      KeyboardSession.shared.useVoiceContext(this.context);
      // 在该上下文的目录中读取内置音效包并写入变调音符。
      KeyboardSession.shared.useSoundContext(this.context);
      // 随包的水杉娘表情包也解压在 resourceDir 下。
      KeyboardSession.shared.mascotStickers.use(this.context.resourceDir);
      const started: boolean = KeyboardSession.shared.start(resources, state, systemDark);
      if (!started) {
        // 没有会话就不创建面板，避免系统显示无法响应的键盘。
        KeyboardLog.error('onCreate: no session, panel not created');
        return;
      }
      engine.on('inputStart', this.started);
      engine.on('inputStop', this.stopped);
      engine.on('keyboardShow', this.keyboardShown);
      engine.on('keyboardHide', this.keyboardHidden);
      inputMethodEngine.getKeyboardDelegate().on('selectionChange', this.selectionChanged);
      inputMethodEngine.getKeyboardDelegate().on('textChange', this.textChanged);
      // 手势绑定取自偏好文档，保证键盘与设置页一致；每种设备都在接收按键前完成绑定，兼容之后插入的实体键盘。
      this.modeKeys.use(KeyboardSession.shared.modeBindings());
      // 所有设备都绑定设置入口，手机也可以从工具面板打开设置应用。
      KeyboardSession.shared.onOpenSettings = (page: string | undefined, requestMicrophone?: boolean, target?: string): void => {
        this.openSettings(page, requestMicrophone === true, target);
      };
      KeyboardSession.shared.onPreferencesReloaded = (): void => {
        this.preferencesReloaded();
      };
      if (KeyboardFormFactor.isDesktop()) {
        // 候选窗口跟随编辑器报告的光标，显示在对应文本旁；固定软键盘始终位于屏幕底部，不随光标移动。
        inputMethodEngine.getKeyboardDelegate().on('cursorContextChange', this.caretMoved);
      }
      // 设备自带实体键盘时必须路由按键，否则没有软键可向引擎发送输入；手机外接键盘也需同样处理，避免系统直接向编辑器写入字母。随键盘接入与移除持续更新设备枚举结果。
      this.routeKeysWhenAttached();
      // 设备自带实体键盘时创建候选窗口；`FLAG_CANDIDATE` 面板由宿主在 `inputStart` 与 `inputStop` 中自行控制显隐。创建时确定标志，避免使用必须先隐藏面板的 `changeFlag`。
      const desktop: boolean = KeyboardFormFactor.isDesktop();
      // 手机和平板上浮动键盘开着时直接建浮动面板（`FLG_FLOATING`，#10225），第一次显示就在存着的位置上。
      const floating: boolean = !desktop && this.floatingWanted();
      const info: inputMethodEngine.PanelInfo = {
        type: inputMethodEngine.PanelType.SOFT_KEYBOARD,
        flag: desktop ? inputMethodEngine.PanelFlag.FLAG_CANDIDATE
          : floating ? inputMethodEngine.PanelFlag.FLG_FLOATING : inputMethodEngine.PanelFlag.FLG_FIXED
      };
      try {
        const panel: inputMethodEngine.Panel = await engine.createPanel(this.context, info);
        // 系统仍在创建或填充面板时也可能触发 `onDestroy`；销毁后不再挂载迟到的面板，避免窗口失去宿主清理回调而泄漏。
        if (this.tornDown) {
          await engine.destroyPanel(panel);
          return;
        }
        this.panel = panel;
        this.floating = floating;
        this.floatingFraction = this.storedFloatingFraction();
        // 挂上视图之前就设好隐私模式，面板第一次显示时已经截不到。
        ScreenCaptureProtection.protectPanel(panel, this.screenCaptureProtected(),
          desktop ? 'candidate window' : 'soft keyboard');
        if (!desktop) {
          // 挂上视图之前定下手机面板第一帧的形状：键面要等视图画出来才知道，按视图上次记下的键面和现在的偏好定（`TouchLayoutStylePolicy.firstPanel`），第一帧就带上数字行和长空格样式一直画着的工具条，不再先矮一行、过一帧再长高。视图出现时（`aboutToAppear`）从同一个形状出发核对高度，所以要在 `setUiContent` 之前。
          // 上一个面板的视图在释放之后发来的高度请求不属于这个面板，作废。
          KeyboardSession.shared.touchHeight.detach();
          const shown: display.Display = display.getDefaultDisplaySync();
          KeyboardSession.shared.decideFirstTouchPanel(KeyboardFormFactorPolicy.phoneLandscape(
            shown.width / shown.densityPixels, shown.height / shown.densityPixels));
        }
        await panel.setUiContent('keyboard/KeyboardView');
        if (this.tornDown) {
          if (this.panel === panel) {
            this.panel = undefined;
            await engine.destroyPanel(panel);
          }
          return;
        }
        // 面板必须明确设置尺寸；高度为候选行加四行按键，不绘制按键时只保留候选条。宽度通过面板窗口读取。
        // 扩展能力此处没有 UI 上下文，不能调用全局 `vp2px`，改用屏幕密度换算。
        const screen: display.Display = display.getDefaultDisplaySync();
        // 面板尺寸与视图布局使用同一组数值，避免裁掉最底行。
        const settings: KeyboardPreferences = KeyboardSession.shared.preferences();
        const opened: TouchPanelShape = KeyboardSession.shared.firstTouchPanel();
        // 视图若已在 `setUiContent` 期间出现并按真实键面请求了高度（记下的键面过时），第一帧直接用它；否则按记下的形状。
        const requested: number | null = desktop ? null : KeyboardSession.shared.touchHeight.takePending();
        const heightVp: number = desktop
          ? (KeyboardMetrics.candidateHeightVp(settings.candidateLayout, 0,
            settings.candidateShowPreedit, settings.candidateDecorationTopVp,
            KeyboardSession.shared.glossRows(), settings.candidateFontSize,
            settings.candidatePreeditFontSize, CandidateLayoutPolicy.gap(settings.candidateSkinLayout, true,
        settings.candidateFontSize)) + CandidateCanvasPolicy.extraHeight(settings.candidateCanvas, settings.candidateShowPreedit))
          // 刚创建时还没有组字也没有面板，工具条画不画看工具栏设置和上次的键面是不是长空格样式的中文 26 键，视图之后按同一条规则（`TouchToolbarPolicy`）改高度。
          : requested !== null ? requested : KeyboardMetrics.totalHeightVp(settings.rowSpacingTenths, settings.heightAdjustment,
            KeyboardSession.shared.glossRows(), settings.candidateFontSize,
            settings.candidatePreeditFontSize, false, opened.strip, opened.extraRows, settings.candidateScalePercent);
        // 软键盘占满屏幕宽度（浮动时按 `fitTouchPanel` 收窄），候选窗口只按内容取宽，避免在桌面上遮住候选所对应的文本。
        this.density = screen.densityPixels;
        this.screenWidth = screen.width;
        this.screenHeight = screen.height;
        if (desktop) {
          await panel.resize(Math.round(KeyboardMetrics.CANDIDATE_WINDOW_WIDTH_VP * screen.densityPixels),
            Math.round(heightVp * screen.densityPixels));
        } else {
          this.fixedWidth = screen.width;
          this.touchHeightVp = heightVp;
          await this.fitTouchPanel(panel);
        }
        if (this.tornDown) {
          if (this.panel === panel) {
            this.panel = undefined;
            await engine.destroyPanel(panel);
          }
          return;
        }
        // 视图发出请求，由持有面板的扩展能力执行。
        KeyboardSession.shared.onDismiss = (): void => {
          panel.hide();
        };
        if (desktop) {
          KeyboardSession.shared.onCandidateWidth = (widthVp: number): void => {
            this.candidateWidthVp = widthVp;
          };
          KeyboardSession.shared.onCandidateCount = (count: number): void => {
            this.candidateCount = count;
            if (KeyboardSession.shared.composing()
              && KeyboardSession.shared.openSurface() === DesktopSurface.NONE) {
              this.resizeCandidateWindow(DesktopSurface.NONE).then((): void => {
                this.placeCandidateWindow();
              }).catch((error: Object): void => {
                KeyboardLog.error(`candidate window would not resize: ${describeError(error)}`);
              });
            }
          };
          KeyboardSession.shared.onComposition = (active: boolean): void => {
            // 工具栏打开的面板在组合输入期间保持打开，便于向正在输入的文字中插入表情。
            if (KeyboardSession.shared.openSurface() !== DesktopSurface.NONE) {
              return;
            }
            if (active) {
              this.candidateAnchor = KeyboardSession.shared.preferences().candidateFollowCursor
                ? undefined : [this.caret[0], this.caret[1], this.caret[2]] as CandidateAnchor;
              this.resizeCandidateWindow(DesktopSurface.NONE);
              this.placeCandidateWindow();
              panel.show();
            } else {
              this.candidateAnchor = undefined;
              this.candidateWidthVp = KeyboardMetrics.CANDIDATE_WINDOW_WIDTH_VP;
              panel.hide();
            }
          };
          KeyboardSession.shared.onSurfaceRequested = (surface: DesktopSurface): void => {
            this.showDesktopSurface(surface);
          };
        } else {
          // 挂上回调时补发第一帧定高度之后、这里之前视图发来的请求（`TouchHeightRelay.attach`）。
          KeyboardSession.shared.touchHeight.attach((requestedHeightVp: number): void => {
            this.touchHeightVp = requestedHeightVp;
            this.fitTouchPanel(panel).catch((error: Object): void => {
              KeyboardLog.error(`touch keyboard would not resize: ${describeError(error)}`);
            });
          });
          KeyboardSession.shared.onFloatingKeyboardChanged = (): void => {
            this.followFloatingSetting(panel);
          };
          KeyboardSession.shared.onFloatingDragStart = (): void => {
            this.beginFloatingDrag();
          };
          KeyboardSession.shared.onFloatingDrag = (offsetX: number, offsetY: number): void => {
            this.dragFloating(offsetX, offsetY);
          };
          KeyboardSession.shared.onFloatingDragEnd = (): void => {
            this.endFloatingDrag();
          };
          display.on('change', this.displayChanged);
          this.watchingDisplay = true;
        }
        if (desktop) {
          await this.reconcileStatusPanels();
        }
        KeyboardLog.info(`panel ready: ${KeyboardFormFactor.describe()}, `
          + `${desktop ? 'candidate window' : 'soft keyboard'}`
          + `${this.toolbar !== undefined ? ', with toolbar' : ''}`
          + `${this.hud !== undefined ? ', with mode badge' : ''}`);
        // 面板创建完成后，服务创建期间获得焦点的编辑器。
        if (this.pending !== undefined) {
          KeyboardSession.shared.attach(this.pending);
          this.pending = undefined;
          this.applyScreenCaptureProtection();
          if (!desktop) {
            this.panel.show();
          }
        }
      } catch (error) {
        // 系统只允许一个软键盘面板；创建被拒绝时销毁会话，避免会话继续绑定到不存在的绘制区域。
        KeyboardLog.error(`panel creation failed: ${describeError(error)}`);
        this.teardown();
        return;
      }
    } catch (error) {
      KeyboardLog.error('inputmethodextability.KeyboardExtensionAbility.onCreate failed');
      throw error as Error;
    }
  }

  onDestroy(): void {
    this.teardown();
    // Here rather than in teardown, which a failed onCreate also runs: only the framework destroying the extension ends the session normally.
    HarmonyTelemetry.keyboardStopped();
  }

  /** Undoes everything onCreate installed. Safe to run twice: a failed onCreate and then onDestroy. */
  private teardown(): void {
    // 长按菜单「朗读」用的引擎：没建过时什么也不做。
    HarmonyReadAloud.shutdown();
    this.statusPanelGeneration += 1;
    this.tornDown = true;
    this.pending = undefined;
    engine.off('inputStart', this.started);
    engine.off('inputStop', this.stopped);
    engine.off('keyboardShow', this.keyboardShown);
    engine.off('keyboardHide', this.keyboardHidden);
    inputMethodEngine.getKeyboardDelegate().off('selectionChange', this.selectionChanged);
    inputMethodEngine.getKeyboardDelegate().off('textChange', this.textChanged);
    // Before releasing keyEvent, so a queued hot-plug change cannot subscribe it again.
    HarmonyHardwareKeyboards.unwatch();
    if (this.routingKeys) {
      inputMethodEngine.getKeyboardDelegate().off('keyEvent', this.typed);
      this.routingKeys = false;
    }
    this.attachedKeyboards = [];
    if (KeyboardFormFactor.isDesktop()) {
      inputMethodEngine.getKeyboardDelegate().off('cursorContextChange', this.caretMoved);
    }
    KeyboardSession.shared.shutdown();
    CandidateCanvasDecoder.trim();
    // These close over the panel being released.
    KeyboardSession.shared.touchHeight.detach();
    KeyboardSession.shared.onDismiss = undefined;
    KeyboardSession.shared.onFloatingKeyboardChanged = undefined;
    KeyboardSession.shared.onFloatingDragStart = undefined;
    KeyboardSession.shared.onFloatingDrag = undefined;
    KeyboardSession.shared.onFloatingDragEnd = undefined;
    if (this.watchingDisplay) {
      display.off('change', this.displayChanged);
      this.watchingDisplay = false;
    }
    KeyboardSession.shared.onCandidateWidth = undefined;
    KeyboardSession.shared.onCandidateCount = undefined;
    KeyboardSession.shared.onComposition = undefined;
    KeyboardSession.shared.onSurfaceRequested = undefined;
    KeyboardSession.shared.onOpenSettings = undefined;
    KeyboardSession.shared.onPreferencesReloaded = undefined;
    this.closeModeBadge();
    this.closeToolbar();
    const panel: inputMethodEngine.Panel | undefined = this.panel;
    this.panel = undefined;
    if (panel !== undefined) {
      engine.destroyPanel(panel).catch((error: Object): void => {
        KeyboardLog.error(`panel would not destroy: ${describeError(error)}`);
      });
    }
  }
}
