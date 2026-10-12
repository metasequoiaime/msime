import UIAbility from '@ohos.app.ability.UIAbility';
import Want from '@ohos.app.ability.Want';
import AbilityConstant from '@ohos.app.ability.AbilityConstant';
import ConfigurationConstant from '@ohos.app.ability.ConfigurationConstant';
import window from '@ohos.window';
import { KeyboardLog } from '../keyboard/KeyboardLog';
import { BusinessErrorPolicy } from '../keyboard/BusinessErrorPolicy';
import { describeError } from '../keyboard/KeyboardLog';
import { SettingsPageLink } from '../keyboard/SettingsPageLink';
import { HarmonyTelemetry } from '../telemetry/Telemetry';
import client from 'libmsimeclient.so';

/**
 * The application the keyboard belongs to.
 *
 * An input method extension has no window of its own beyond its panel, and on a 2in1 that panel is a
 * candidate window with no room for the surfaces a phone reaches from its key faces. Everything that
 * is a setting rather than a keystroke lives here instead: one window, one page, and the preference
 * document the keyboard reads when it next starts.
 */
export default class EntryAbility extends UIAbility {
  /** Set by the settings page, which owns the bridge that can read and deliver the document. */
  static onActivated: (() => void) | undefined = undefined;
  /** 设置窗口退到后台时调用，设置页借它把改动立即发布给键盘。 */
  static onDeactivated: (() => void) | undefined = undefined;
  /** 键盘请求过、设置页面还没取走的页面；设置页面加载完成后会取走它。 */
  static pendingPage: string | null = null;
  /** 由设置页面在打开期间设置，让之后请求的页面能送达已经显示着的设置页面。 */
  static onOpenPage: ((page: string) => void) | undefined = undefined;

  /**
   * 键盘请求的设置页面，没有则为 null。
   *
   * 键盘工具栏通过在 want 里传 `page` 直接打开 词库、反馈 和 关于。只接受 `SettingsPageLink.PAGES` 中的页面：want 里的其他内容可能是别的应用发来的，不应让页面被引到一个没人打算开放的路由。
   */
  private static linkedPage(want: Want): string | null {
    const parameters: Record<string, Object> | undefined = want.parameters;
    if (parameters === undefined) {
      return null;
    }
    return SettingsPageLink.fromParameter(parameters['page']);
  }

  onCreate(want: Want, launch: AbilityConstant.LaunchParam): void {
    KeyboardLog.info('app created');
    EntryAbility.pendingPage = EntryAbility.linkedPage(want);
    // Sends what usage reporting has queued (and today's active) when the switch is on; the keyboard owns the sessions.
    HarmonyTelemetry.applicationStarted(`${this.context.filesDir}/state`);
    // The device's anonymous MSIME account is registered on first launch rather than on the first feature that needs it; once its session is saved this only reads a file. A failure is retried on the next launch or keyboard start.
    client.ensureAnonymousAccount(`${this.context.filesDir}/state`).catch((error: Error): void => {
      KeyboardLog.error(`anonymous account: ${describeError(error)}`);
    });
  }

  /**
   * 应用已在运行时键盘请求某个页面，此时复用这个 ability，而不是重新创建。
   *
   * 已打开的设置页面原地导航；否则请求会等到页面加载后来取。
   */
  onNewWant(want: Want, launch: AbilityConstant.LaunchParam): void {
    const page: string | null = EntryAbility.linkedPage(want);
    if (page === null) {
      return;
    }
    const open: ((page: string) => void) | undefined = EntryAbility.onOpenPage;
    if (open === undefined) {
      EntryAbility.pendingPage = page;
      return;
    }
    open(page);
  }

  onWindowStageCreate(stage: window.WindowStage): void {
    // The keyboard writes preferences of its own — the toolbar's 简繁, 标点 and 全半角 each save —
    // and the two are separate processes with no listener between them. A settings window that
    // missed one got a conflict on its next save, which is safe but tells the user only that
    // something clashed. Refreshing when the window comes back to the front is the moment a user
    // who has just used the keyboard is looking at this page again.
    try {
      stage.on('windowStageEvent', (event: window.WindowStageEventType): void => {
        if (event === window.WindowStageEventType.ACTIVE) {
          EntryAbility.onActivated?.();
        } else if (event === window.WindowStageEventType.HIDDEN) {
          EntryAbility.onDeactivated?.();
        }
      });
    } catch (error) {
      KeyboardLog.error(`window stage events unavailable: ${describeError(error)}`);
    }
    // An AsyncCallback is handed a BusinessError whether it succeeded or not, and on success that
    // object carries code 0. Testing the object itself reported a failure on every successful load,
    // which is also what made a real one impossible to spot among them.
    stage.loadContent('pages/Settings', (error: Object): void => {
      if (BusinessErrorPolicy.failed(error)) {
        KeyboardLog.error(`settings page would not load: ${BusinessErrorPolicy.describe(error)}`);
      }
    });
    // 系统栏先取启动窗的颜色（`start_window_background`，浅色和深色各一份，跟随系统外观）：页面要等桥接就绪才能调用 setSystemBars，在那之前状态栏是系统默认色，会在启动画面上下露出一条别的颜色。页面挂载后会换成它自己的颜色。
    const launchColour: string = this.launchColour();
    const launchContent: string =
      this.context.config.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK ? '#FFFFFF' : '#000000';
    const launchBars: window.SystemBarProperties = {
      statusBarColor: launchColour,
      statusBarContentColor: launchContent,
      navigationBarColor: launchColour,
      navigationBarContentColor: launchContent
    };
    stage.getMainWindow().then((main: window.Window): Promise<void> => {
      // 窗口底色也一样：底部手势条那一块露出的是它，见 Settings 的 setSystemBars。
      try {
        main.setWindowBackgroundColor(launchColour);
      } catch (error) {
        KeyboardLog.error(`launch window background not applied: ${describeError(error)}`);
      }
      return main.setWindowSystemBarProperties(launchBars);
    }).catch((error: Object): void => {
      KeyboardLog.error(`launch system bar colours not applied: ${describeError(error)}`);
    });
  }

  /** 启动窗的底色，`#AARRGGBB`。从资源读而不是写死：浅色和深色由资源目录各给一份，系统按当前外观取。 */
  private launchColour(): string {
    const argb: number = this.context.resourceManager.getColorSync($r('app.color.start_window_background').id);
    return `#${(argb >>> 0).toString(16).padStart(8, '0')}`;
  }
}
