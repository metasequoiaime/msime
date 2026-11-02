import { HarmonyMicrophonePermission } from '../keyboard/input/HarmonyMicrophonePermission';
import UIAbility from '@ohos.app.ability.UIAbility';
import Want from '@ohos.app.ability.Want';
import AbilityConstant from '@ohos.app.ability.AbilityConstant';
import ConfigurationConstant from '@ohos.app.ability.ConfigurationConstant';
import window from '@ohos.window';
import { KeyboardLog } from '../keyboard/KeyboardLog';
import { BusinessErrorPolicy } from '../keyboard/BusinessErrorPolicy';
import { describeError } from '../keyboard/KeyboardLog';
import { SettingsLinkRequest, SettingsPageLink } from '../keyboard/SettingsPageLink';
import { HarmonyTelemetry } from '../telemetry/Telemetry';
import { SharedStateMirror } from '../keyboard/SharedStateMirror';
import { HarmonySharedSandboxPolicy } from '../keyboard/HarmonySharedSandboxPolicy';
import { AccountSessionAdoption } from '../account/AccountSessionAdoption';
import { HarmonyAccountIdentity } from '../account/HarmonyAccountIdentity';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';
import { ScreenCaptureProtection } from '../keyboard/settings/ScreenCaptureProtection';

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
  /** 键盘请求过、设置页面还没取走的页面（和它的子目标）；设置页面加载完成后会取走它。 */
  static pendingPage: SettingsLinkRequest | null = null;
  /** 由设置页面在打开期间设置，让之后请求的页面能送达已经显示着的设置页面。 */
  static onOpenPage: ((link: SettingsLinkRequest) => void) | undefined = undefined;

  /**
   * 键盘请求的设置页面，没有则为 null。
   *
   * 键盘通过在 want 里传 `page` 直接打开词库、反馈、关于、皮肤和屏幕键盘（长按工具栏按钮），再带一个 `target` 直接打开那一页里的某一处（长按九键左侧符号栏：「按键布局」里那张表的编辑页）。只接受 `SettingsPageLink.PAGES` 中的页面和 `SettingsPageLink.targets(page)` 中的子目标：want 里的其他内容可能是别的应用发来的，不应让页面被引到一个没人打算开放的路由。
   */
  private static linkedPage(want: Want): SettingsLinkRequest | null {
    const parameters: Record<string, Object> | undefined = want.parameters;
    if (parameters === undefined) {
      return null;
    }
    return SettingsPageLink.fromParameters(parameters['page'], parameters['target']);
  }

  private telemetryGeneration: number = 0;
  private microphoneRequested: boolean = false;
  private microphoneRequestPending: boolean = false;

  private requestMicrophone(): void {
    if (this.microphoneRequestPending) return;
    this.microphoneRequested = false;
    this.microphoneRequestPending = true;
    void HarmonyMicrophonePermission.request(this.context).finally((): void => {
      this.microphoneRequestPending = false;
    });
  }

  onForeground(): void {
    if (this.microphoneRequested) this.requestMicrophone();
  }

  onCreate(want: Want, launch: AbilityConstant.LaunchParam): void {
    KeyboardLog.info('app created');
    this.microphoneRequested = want.parameters?.['requestMicrophone'] === true;
    EntryAbility.pendingPage = EntryAbility.linkedPage(want);
    // 先读隐私政策的授权：下面的使用统计发送和匿名账号注册都要联网，没同意之前它们各自等着，同意弹窗由设置页面在显示时打开。
    HarmonyPrivacyConsent.load(`${this.context.filesDir}/state`);
    // 键盘经共享沙箱发布的会话要在本进程第一个认证请求之前采用过来（迁移差异 M2），所以进程一创建就占住会话镜像的队列；设置页、反馈、匿名注册的请求都排在它后面。
    const files: string = this.context.filesDir;
    const generation: number = ++this.telemetryGeneration;
    const group: Promise<string | null> = SharedStateMirror.resolve(this.context);
    void group.then((groupDir: string | null): void => {
      if (generation !== this.telemetryGeneration) return;
      HarmonyTelemetry.configureAuthority(groupDir, true);
      HarmonyTelemetry.applicationStarted(`${files}/state`);
    });
    const mirror: Promise<SharedStateMirror | null> = group
      .then((groupDir: string | null): SharedStateMirror | null => {
        const status = HarmonySharedSandboxPolicy.status(groupDir);
        return status.available ? new SharedStateMirror(status.directory, files) : null;
      });
    AccountSessionAdoption.adoptOnStart(mirror, `${files}/state`, 'keyboard', (): boolean => true);
    // 本机匿名账号在同意隐私政策之后马上注册（首次启动时同意就是首次启动），而不是等到第一个要用它的功能；会话存下以后这里只读一次文件。没同意就不注册；失败时下一次启动或键盘启动再试。
    HarmonyAccountIdentity.ensureAnonymous(`${files}/state`);
  }

  onDestroy(): void {
    this.telemetryGeneration++;
    HarmonyTelemetry.applicationStopped();
  }

  /**
   * 应用已在运行时键盘请求某个页面，此时复用这个 ability，而不是重新创建。
   *
   * 已打开的设置页面原地导航；否则请求会等到页面加载后来取。
   */
  onNewWant(want: Want, launch: AbilityConstant.LaunchParam): void {
    if (want.parameters?.['requestMicrophone'] === true) this.requestMicrophone();
    const link: SettingsLinkRequest | null = EntryAbility.linkedPage(want);
    if (link === null) {
      return;
    }
    const open: ((link: SettingsLinkRequest) => void) | undefined = EntryAbility.onOpenPage;
    if (open === undefined) {
      EntryAbility.pendingPage = link;
      return;
    }
    open(link);
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
    // 「防止截屏」开着时，在页面载入之前给主窗口设上隐私模式，第一帧就截不到；之后的切换由设置页写入时当场更新（`SettingsBridge.screenCaptureProtectionSettings`）。
    try {
      if (ScreenCaptureProtection.load(`${this.context.filesDir}/state`).enabled) {
        ScreenCaptureProtection.protectWindow(stage.getMainWindowSync(), true);
      }
    } catch (error) {
      KeyboardLog.error(`main window unavailable for privacy mode: ${describeError(error)}`);
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
    try {
      const argb: number = this.context.resourceManager.getColorSync($r('app.color.start_window_background').id);
      return `#${(argb >>> 0).toString(16).padStart(8, '0')}`;
    } catch {
      KeyboardLog.error('launch window colour unavailable');
      return this.context.config.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK
        ? '#FF0A0B0A' : '#FFF1F3F5';
    }
  }
}
