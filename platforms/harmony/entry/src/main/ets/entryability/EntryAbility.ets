import UIAbility from '@ohos.app.ability.UIAbility';
import Want from '@ohos.app.ability.Want';
import AbilityConstant from '@ohos.app.ability.AbilityConstant';
import window from '@ohos.window';
import { KeyboardLog } from '../keyboard/KeyboardLog';
import { BusinessErrorPolicy } from '../keyboard/BusinessErrorPolicy';
import { describeError } from '../keyboard/KeyboardLog';
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

  onCreate(want: Want, launch: AbilityConstant.LaunchParam): void {
    KeyboardLog.info('app created');
    // Sends what usage reporting has queued (and today's active) when the switch is on; the keyboard owns the sessions.
    HarmonyTelemetry.applicationStarted(`${this.context.filesDir}/state`);
    // The device's anonymous MSIME account is registered on first launch rather than on the first feature that needs it; once its session is saved this only reads a file. A failure is retried on the next launch or keyboard start.
    client.ensureAnonymousAccount(`${this.context.filesDir}/state`).catch((error: Error): void => {
      KeyboardLog.error(`anonymous account: ${describeError(error)}`);
    });
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
  }
}
