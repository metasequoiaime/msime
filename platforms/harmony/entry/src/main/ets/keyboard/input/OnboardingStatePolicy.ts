/**
 * Whether the settings window should open on the welcome flow instead of the settings page.
 *
 * A keyboard nobody has enabled is not a keyboard, and on HarmonyOS enabling one is two trips into
 * system screens the user has no reason to know about: turn the input method on, then pick it as
 * the current one. Android and iOS already walk people through exactly this with the shared welcome
 * flow; this host dropped them straight into a settings page for a keyboard that could not type.
 *
 * Two separate facts decide it, because they fail separately and the user fixes them in different
 * screens: the input method has to be enabled at all, and it has to be the one in use. Being
 * enabled but not current is the state someone lands in after doing half the setup, and it is worth
 * finishing the flow rather than showing settings that appear to have had no effect.
 */

/** What the framework reports, mirroring `inputMethod.EnabledState`. */
export enum ImeEnabledState {
  DISABLED = 0,
  BASIC_MODE = 1,
  FULL_EXPERIENCE_MODE = 2,
}

export interface OnboardingState {
  /** The framework's enablement answer, or null when it could not be asked. */
  readonly enabled: ImeEnabledState | null;
  /** The bundle currently serving input, or an empty string when it could not be read. */
  readonly currentBundle: string;
  /** This keyboard's own bundle. */
  readonly ownBundle: string;
}

/**
 * 单独的两个设置事实：设置页把它们显示为「启用」和「设为默认」两行，任一变化时也会再告诉设置页。
 *
 * 系统问不到时各自为 null，这样页面可以显示「未知」，而不是给出一个笃定的错误答案。
 */
export interface SetupState {
  /** 输入法是否在系统设置里开启。基础模式也算开启。 */
  readonly enabled: boolean | null;
  /** 这个键盘是否就是当前正在服务输入的那个。 */
  readonly current: boolean | null;
}

export class OnboardingStatePolicy {
  /**
   * Whether setup still has a step left in it.
   *
   * An unanswerable query is not treated as "needs onboarding". Someone who has been typing with
   * this keyboard for a month should not be sent back to a welcome screen because one system call
   * failed; the settings page is the safe thing to show when the answer is unknown, since it is
   * reachable from the welcome flow but not the other way round.
   */
  static required(state: OnboardingState): boolean {
    return OnboardingStatePolicy.requiredFor(OnboardingStatePolicy.setup(state));
  }

  /**
   * 把框架的回答归结为页面显示的两个事实。
   *
   * 任一名称读不到时比较就没有意义；而且自己的名称读不到时会与包括自己在内的所有键盘都不相等，所以两者都读作未知，而不是「不是当前」。
   */
  static setup(state: OnboardingState): SetupState {
    const enabled: boolean | null =
      state.enabled === null ? null : state.enabled !== ImeEnabledState.DISABLED;
    const current: boolean | null =
      state.currentBundle.length === 0 || state.ownBundle.length === 0
        ? null
        : state.currentBundle === state.ownBundle;
    const setup: SetupState = { enabled: enabled, current: current };
    return setup;
  }

  /**
   * 只凭这两个事实判断设置是否还剩步骤。
   *
   * 未启用时，无论当前键盘读成什么都已经决定。否则任一侧未知都打开设置，理由见 `required`。
   */
  static requiredFor(setup: SetupState): boolean {
    if (setup.enabled === null) {
      return false;
    }
    if (!setup.enabled) {
      return true;
    }
    if (setup.current === null) {
      return false;
    }
    return !setup.current;
  }

  /**
   * Which step the flow is on, for the log line that says why the window opened where it did.
   *
   * Setup that goes wrong here goes wrong silently — a keyboard that is enabled but not selected
   * looks exactly like one that is not installed — so the reason is recorded rather than inferred
   * afterwards from which screen appeared.
   */
  static describe(state: OnboardingState): string {
    if (state.enabled === null) {
      return "setup state unknown, opening settings";
    }
    if (state.enabled === ImeEnabledState.DISABLED) {
      return "not enabled, opening welcome flow";
    }
    if (state.currentBundle.length === 0 || state.ownBundle.length === 0) {
      return "enabled, keyboard identity unknown, opening settings";
    }
    if (state.currentBundle !== state.ownBundle) {
      return "enabled but not current, opening welcome flow";
    }
    return "enabled and current, opening settings";
  }
}
