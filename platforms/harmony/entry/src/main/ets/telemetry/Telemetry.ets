import bundleManager from '@ohos.bundle.bundleManager';
import hiAppEvent from '@ohos.hiviewdfx.hiAppEvent';
import process from '@ohos.process';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { CrashDestination, CrashReport, TelemetryPolicy } from './TelemetryPolicy';

/**
 * Anonymous usage reporting (the `usage_reporting` preference, on by default).
 *
 * The queue, the random install id, the daily `active` and the session bookkeeping live in client-core under `files/state/telemetry`, which the settings application and the keyboard extension share; client-core locks it across the two processes. A session is one keyboard process: the keyboard begins it when the extension is created and ends it when the extension is destroyed normally. The settings application only sends what is queued.
 *
 * Crashes are HarmonyOS's own reports. HiAppEvent hands a watcher the previous run's `APP_CRASH` on the next start, JavaScript and native alike, so this installs no handler and changes nothing about how a crash ends the process. A crash of the keyboard process that is reported before this keyboard begins its own session is written as that unfinished session's crash record, which makes it a `session_crash`; anything else is a standalone record. Records are sent as `crash` events on the next start.
 */

const PLATFORM: string = 'harmony';
const WATCHER_NAME: string = 'msime_crash_reports';
const CRASH_DIRECTORY: string = 'telemetry-crashes';
/** The keyboard's process id, remembered so the next start can tell its crash from the settings application's. */
const KEYBOARD_PROCESS_FILE: string = 'harmony-keyboard-process.json';
/**
 * How long the keyboard waits for the system's crash report before it closes the previous session.
 *
 * Closing it first would turn a crashed session into one that left only its marker, which counts as no session at all. A report that comes later is still sent, as a standalone crash.
 */
const BEGIN_DELAY_MS: number = 2000;

interface TelemetryRequest {
  directory: string;
  platform: string;
  version: string;
  preferences_directory: string;
}

interface DirectoryRequest {
  directory: string;
}

interface CrashRecordRequest {
  directory: string;
  message: string;
  stack: string;
}

interface BeginValue {
  enabled?: boolean;
}

interface BeginReply {
  ok: boolean;
  value?: BeginValue;
  error?: string;
}

interface FlagReply {
  ok: boolean;
  value?: boolean;
  error?: string;
}

interface RememberedProcess {
  pid: number;
}

function version(): string {
  try {
    return bundleManager.getBundleInfoForSelfSync(
      bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT).versionName;
  } catch (error) {
    KeyboardLog.error(`telemetry: version unreadable: ${describeError(error)}`);
    return '';
  }
}

export class HarmonyTelemetry {
  private static directory: string = '';
  private static preferences: string = '';
  private static began: boolean = false;
  private static enabled: boolean = false;
  private static previousKeyboardPid: number | null = null;
  private static beginTimer: number = -1;
  private static lastFlush: number = 0;
  private static watcher: hiAppEvent.Watcher | undefined = undefined;

  /** The telemetry directory under a process's shared state directory. */
  static directoryFor(stateDirectory: string): string {
    return `${stateDirectory}/telemetry`;
  }

  /** The settings application started: send what is queued, which also counts today's `active`. No session: that is the keyboard's. */
  static applicationStarted(stateDirectory: string): void {
    HarmonyTelemetry.send(HarmonyTelemetry.request(stateDirectory));
  }

  /** The keyboard extension was created: watch for the system's crash reports, then begin this process's session. */
  static keyboardStarted(stateDirectory: string): void {
    HarmonyTelemetry.directory = HarmonyTelemetry.directoryFor(stateDirectory);
    HarmonyTelemetry.preferences = stateDirectory;
    HarmonyTelemetry.began = false;
    HarmonyTelemetry.enabled = false;
    HarmonyTelemetry.previousKeyboardPid = HarmonyTelemetry.readRememberedPid();
    HarmonyTelemetry.watchCrashes();
    HarmonyTelemetry.beginTimer = setTimeout((): void => {
      HarmonyTelemetry.beginTimer = -1;
      HarmonyTelemetry.begin();
    }, BEGIN_DELAY_MS);
  }

  /** A text field took the keyboard: a process that stays up for days still sends each day's `active`. */
  static keyboardShown(): void {
    if (!HarmonyTelemetry.began || !TelemetryPolicy.flushDue(HarmonyTelemetry.lastFlush, Date.now())) {
      return;
    }
    HarmonyTelemetry.send(HarmonyTelemetry.request(HarmonyTelemetry.preferences));
  }

  /** The keyboard extension is being destroyed normally: its session ended without a crash. */
  static keyboardStopped(): void {
    if (HarmonyTelemetry.beginTimer !== -1) {
      clearTimeout(HarmonyTelemetry.beginTimer);
      HarmonyTelemetry.beginTimer = -1;
    }
    if (HarmonyTelemetry.watcher !== undefined) {
      try {
        hiAppEvent.removeWatcher(HarmonyTelemetry.watcher);
      } catch (error) {
        KeyboardLog.error(`telemetry: crash watcher not removed: ${describeError(error)}`);
      }
      HarmonyTelemetry.watcher = undefined;
    }
    // Only a session this process began is this process's to end: ending before it began would close the crashed previous session as a normal one.
    if (!HarmonyTelemetry.began) {
      return;
    }
    HarmonyTelemetry.began = false;
    const request: DirectoryRequest = { directory: HarmonyTelemetry.directory };
    HarmonyTelemetry.logRefusal('end', client.telemetryEnd(JSON.stringify(request)));
  }

  /** The user just turned usage reporting off in settings: nothing queued is sent, now or later. */
  static reportingTurnedOff(stateDirectory: string): void {
    const request: DirectoryRequest = { directory: HarmonyTelemetry.directoryFor(stateDirectory) };
    HarmonyTelemetry.logRefusal('clear', client.telemetryClear(JSON.stringify(request)));
  }

  private static request(stateDirectory: string): TelemetryRequest {
    return {
      directory: HarmonyTelemetry.directoryFor(stateDirectory),
      platform: PLATFORM,
      version: version(),
      preferences_directory: stateDirectory,
    };
  }

  private static begin(): void {
    const request: TelemetryRequest = HarmonyTelemetry.request(HarmonyTelemetry.preferences);
    const reply: string = client.telemetryBegin(JSON.stringify(request));
    let parsed: BeginReply;
    try {
      parsed = JSON.parse(reply) as BeginReply;
    } catch (error) {
      KeyboardLog.error(`telemetry: begin answer unreadable: ${describeError(error)}`);
      return;
    }
    if (parsed.ok !== true) {
      KeyboardLog.error(`telemetry: begin refused: ${parsed.error ?? 'unknown'}`);
      return;
    }
    HarmonyTelemetry.began = true;
    HarmonyTelemetry.enabled = parsed.value?.enabled === true;
    if (!HarmonyTelemetry.enabled) {
      return;
    }
    HarmonyTelemetry.rememberPid();
    HarmonyTelemetry.send(request);
  }

  private static send(request: TelemetryRequest): void {
    HarmonyTelemetry.lastFlush = Date.now();
    client.telemetryFlush(JSON.stringify(request)).then((reply: string): void => {
      HarmonyTelemetry.logRefusal('flush', reply);
    }).catch((error: Error): void => {
      KeyboardLog.error(`telemetry: flush failed: ${describeError(error)}`);
    });
  }

  private static watchCrashes(): void {
    const watcher: hiAppEvent.Watcher = {
      name: WATCHER_NAME,
      appEventFilters: [{ domain: hiAppEvent.domain.OS, names: [hiAppEvent.event.APP_CRASH] }],
      onReceive: (domain: string, groups: Array<hiAppEvent.AppEventGroup>): void => {
        for (const group of groups) {
          for (const info of group.appEventInfos) {
            const report: CrashReport | null = TelemetryPolicy.crashReport(JSON.stringify(info.params));
            if (report !== null) {
              HarmonyTelemetry.recordCrash(report);
            }
          }
        }
      }
    };
    try {
      hiAppEvent.addWatcher(watcher);
      HarmonyTelemetry.watcher = watcher;
    } catch (error) {
      KeyboardLog.error(`telemetry: crash watcher refused: ${describeError(error)}`);
    }
  }

  private static recordCrash(report: CrashReport): void {
    // After begin, an off switch means the crash is not kept at all; before it, begin clears what is written when reporting is off.
    if (HarmonyTelemetry.began && !HarmonyTelemetry.enabled) {
      return;
    }
    const destination: CrashDestination =
      TelemetryPolicy.destination(report, HarmonyTelemetry.previousKeyboardPid, HarmonyTelemetry.began);
    if (destination === CrashDestination.Session) {
      // The marker on disk is still the crashed session's, so client-core writes the record under that session's id.
      const request: CrashRecordRequest = {
        directory: HarmonyTelemetry.directory, message: report.message, stack: report.stack
      };
      const reply: FlagReply | null =
        HarmonyTelemetry.flagReply('record', client.telemetryRecordCrash(JSON.stringify(request)));
      if (reply !== null && reply.value === true) {
        return;
      }
    }
    HarmonyTelemetry.writeStandalone(report);
  }

  private static writeStandalone(report: CrashReport): void {
    const directory: string = `${HarmonyTelemetry.directory}/${CRASH_DIRECTORY}`;
    const path: string = `${directory}/${TelemetryPolicy.standaloneRecordName(report)}`;
    try {
      if (!HarmonyTelemetry.prepareTelemetryDirectory()) {
        return;
      }
      if (fs.accessSync(directory)) {
        const directoryStat: fs.Stat = fs.lstatSync(directory);
        if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
          return;
        }
      } else {
        fs.mkdirSync(directory, true);
        const directoryStat: fs.Stat = fs.lstatSync(directory);
        if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
          return;
        }
      }
      if (fs.accessSync(path)) {
        return;
      }
      HarmonyTelemetry.atomicWrite(path, TelemetryPolicy.recordText(report), false);
    } catch (error) {
      KeyboardLog.error(`telemetry: crash record not written: ${describeError(error)}`);
    }
  }

  private static readRememberedPid(): number | null {
    const path: string = `${HarmonyTelemetry.directory}/${KEYBOARD_PROCESS_FILE}`;
    let handle: fs.File | null = null;
    try {
      if (!HarmonyTelemetry.safeTelemetryDirectory()) {
        return null;
      }
      const expected: fs.Stat = fs.lstatSync(path);
      if (expected.isSymbolicLink() || !expected.isFile()) {
        return null;
      }
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > 256 || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      const text: string = util.TextDecoder.create('utf-8').decode(
        new Uint8Array(buffer, 0, count));
      return TelemetryPolicy.rememberedPid(text);
    } catch (error) {
      // An absent marker is the normal first-run state.
      try {
        if (fs.accessSync(path)) {
          KeyboardLog.error(`telemetry: keyboard process unreadable: ${describeError(error)}`);
        }
      } catch {
        // The directory or marker is unavailable.
      }
      return null;
    } finally {
      if (handle !== null) fs.closeSync(handle);
    }
  }

  private static rememberPid(): void {
    const remembered: RememberedProcess = { pid: process.pid };
    const path: string = `${HarmonyTelemetry.directory}/${KEYBOARD_PROCESS_FILE}`;
    try {
      if (!HarmonyTelemetry.prepareTelemetryDirectory()) {
        return;
      }
      HarmonyTelemetry.atomicWrite(path, JSON.stringify(remembered), true);
    } catch (error) {
      KeyboardLog.error(`telemetry: keyboard process not remembered: ${describeError(error)}`);
    }
  }

  private static atomicWrite(path: string, text: string, replaceExisting: boolean): boolean {
    const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
    let renamed: boolean = false;
    try {
      const file: fs.File = fs.openSync(staging,
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(file.fd, text);
      } finally {
        fs.closeSync(file);
      }
      if (!replaceExisting && fs.accessSync(path)) return false;
      fs.renameSync(staging, path);
      renamed = true;
      return true;
    } finally {
      if (!renamed) {
        try {
          if (fs.accessSync(staging)) fs.unlinkSync(staging);
        } catch {
          // A failed telemetry write leaves at most its uniquely named staging file.
        }
      }
    }
  }

  /** Ensure the shared telemetry directory itself is a real directory before following it. */
  private static prepareTelemetryDirectory(): boolean {
    try {
      if (!fs.accessSync(HarmonyTelemetry.directory)) {
        fs.mkdirSync(HarmonyTelemetry.directory, true);
      }
      return HarmonyTelemetry.safeTelemetryDirectory();
    } catch {
      return false;
    }
  }

  private static safeTelemetryDirectory(): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(HarmonyTelemetry.directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch {
      return false;
    }
  }

  /** The answer to a call whose value is a flag, or null (logged) when it was refused or unreadable. */
  private static flagReply(step: string, reply: string): FlagReply | null {
    try {
      const parsed: FlagReply = JSON.parse(reply) as FlagReply;
      if (parsed.ok === true) {
        return parsed;
      }
      KeyboardLog.error(`telemetry: ${step} refused: ${parsed.error ?? 'unknown'}`);
    } catch (error) {
      KeyboardLog.error(`telemetry: ${step} answer unreadable: ${describeError(error)}`);
    }
    return null;
  }

  private static logRefusal(step: string, reply: string): void {
    HarmonyTelemetry.flagReply(step, reply);
  }
}
