import { NativeCleanup } from '../keyboard/NativeCleanup';
import bundleManager from '@ohos.bundle.bundleManager';
import hiAppEvent from '@ohos.hiviewdfx.hiAppEvent';
import errorManager from '@ohos.app.ability.errorManager';
import process from '@ohos.process';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { CrashReport, ReportingConsent, SystemCapture, TelemetryPolicy } from './TelemetryPolicy';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';

/** App 与键盘各自拥有私有队列；共享只读授权记录负责跨沙箱持续同意，不依赖焦点镜像。 */

const PLATFORM: string = 'harmony';
const WATCHER_NAME: string = 'msime_crash_reports';
/** 先等上一进程的系统故障投递，再建立新的输入会话。 */
const BEGIN_DELAY_MS: number = 2000;

interface ConsentPublication {
  policy_allowed: boolean;
  policy_identity: string;
}

interface ConsentQuery {
  directory?: string;
  authority_directory?: string;
  publication?: ConsentPublication;
}

interface TelemetryRequest {
  directory: string;
  platform: string;
  version: string;
  authority_directory?: string;
  capture_only?: boolean;
  component?: string;
  consent_epoch?: string;
}

interface DirectoryRequest {
  directory: string;
  authority_directory?: string;
  session?: string;
}

interface CrashRecordRequest {
  directory: string;
  message: string;
  stack: string;
  pid?: number;
  outside_session?: boolean;
  crash_record_header?: string;
  diagnostic_id?: string;
  diagnostic_capture?: string;
  diagnostic_begin_unix_ms?: number;
  diagnostic_end_unix_ms?: number;
  consent_epoch?: string;
  authority_directory?: string;
}

interface BeginValue {
  enabled?: boolean;
  session_id?: string;
  crash_record_header?: string;
  crash_record_path?: string;
}

interface RememberedProcess {
  pid: number;
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
  private static authority: string = '';
  private static publisher: boolean = true;
  private static blocked: boolean = false;
  private static began: boolean = false;
  private static session: string = '';
  private static epoch: string = '';
  private static enabled: boolean = false;
  private static publicationFailed: boolean = false;
  private static cleared: boolean = false;
  private static stopped: boolean = true;
  private static beginTimer: number = -1;
  private static observer: number = -1;
  private static lastFlush: number = 0;
  private static watcher: hiAppEvent.Watcher | undefined = undefined;
  private static startGeneration: number = 0;
  private static activeCapture: string = '';
  private static activeCrashPath: string = '';
  private static nativeCapture: string = '';
  private static errorHandlerInstalled: boolean = false;
  private static errorHandlerFailed: boolean = false;
  private static nativeCaptureFailed: boolean = false;
  private static desiredCapture: string = '';
  private static installedCapture: string | undefined = undefined;
  private static capturePending: boolean = false;
  private static capturePublicationFailed: boolean = false;
  private static previousKeyboardPid: number | null = null;

  /** 设置进程发布，键盘只读；拿不到共享目录时键盘不能自行退回私有授权。 */
  static configureAuthority(groupDirectory: string | null, publisher: boolean): void {
    HarmonyTelemetry.publisher = publisher;
    HarmonyTelemetry.authority = groupDirectory === null ? '' : `${groupDirectory}/msime/reporting-authority`;
    HarmonyTelemetry.blocked = !publisher && groupDirectory === null;
  }

  static directoryFor(stateDirectory: string): string {
    return `${stateDirectory}/telemetry`;
  }

  /** 设置应用只准备独立采集；首次未同意政策也要发布关闭，不能留下旧许可。 */
  static applicationStarted(stateDirectory: string): void {
    const generation: number = HarmonyTelemetry.start(stateDirectory);
    void HarmonyPrivacyConsent.whenNetworkAllowed().then((): void => {
      if (generation === HarmonyTelemetry.startGeneration && !HarmonyTelemetry.stopped) {
        HarmonyTelemetry.send(HarmonyTelemetry.request(stateDirectory));
        HarmonyTelemetry.refresh();
      }
    });
  }

  /** 无需等焦点导入偏好；只读的共享授权观察独立于网络与本地政策镜像。 */
  static keyboardStarted(stateDirectory: string): void {
    const generation: number = HarmonyTelemetry.start(stateDirectory);
    void HarmonyPrivacyConsent.whenNetworkAllowed().then((): void => {
      if (generation === HarmonyTelemetry.startGeneration && !HarmonyTelemetry.stopped) {
        HarmonyTelemetry.startKeyboardSession(stateDirectory);
      }
    });
  }

  private static startKeyboardSession(stateDirectory: string): void {
    HarmonyTelemetry.preferences = stateDirectory;
    HarmonyTelemetry.refresh();
  }

  private static start(stateDirectory: string): number {
    HarmonyTelemetry.stopped = false;
    HarmonyTelemetry.preferences = stateDirectory;
    HarmonyTelemetry.previousKeyboardPid = HarmonyTelemetry.readRememberedPid();
    HarmonyTelemetry.directory = HarmonyTelemetry.directoryFor(stateDirectory);
    if (HarmonyTelemetry.publisher && HarmonyTelemetry.authority === '') HarmonyTelemetry.authority = `${stateDirectory}/reporting-authority`;
    const generation: number = ++HarmonyTelemetry.startGeneration;
    HarmonyTelemetry.refresh();
    if (HarmonyTelemetry.observer === -1) {
      HarmonyTelemetry.observer = setInterval((): void => HarmonyTelemetry.refresh(), 1000);
    }
    return generation;
  }

  static keyboardShown(): void {
    HarmonyTelemetry.refresh();
  }

  static applicationStopped(): void {
    HarmonyTelemetry.stop();
  }

  static keyboardStopped(): void {
    HarmonyTelemetry.stop();
  }

  private static cancelBegin(): void {
    if (HarmonyTelemetry.beginTimer !== -1) clearTimeout(HarmonyTelemetry.beginTimer);
    HarmonyTelemetry.beginTimer = -1;
  }

  private static stop(): void {
    HarmonyTelemetry.stopped = true;
    HarmonyTelemetry.activeCapture = '';
    HarmonyTelemetry.publishCapture('');
    HarmonyTelemetry.startGeneration++;
    if (HarmonyTelemetry.observer !== -1) clearInterval(HarmonyTelemetry.observer);
    HarmonyTelemetry.observer = -1;
    HarmonyTelemetry.cancelBegin();
    if (HarmonyTelemetry.watcher !== undefined) {
      try { hiAppEvent.removeWatcher(HarmonyTelemetry.watcher); }
      catch (error) { KeyboardLog.error(`telemetry: crash watcher not removed: ${describeError(error)}`); }
      HarmonyTelemetry.watcher = undefined;
    }
    // 只结束本进程实际开始的会话，不动其他进程或上一轮故障的标记。
    if (HarmonyTelemetry.began && HarmonyTelemetry.session !== '') {
      const request: DirectoryRequest = { directory: HarmonyTelemetry.directory, session: HarmonyTelemetry.session };
      HarmonyTelemetry.logRefusal('end', client.telemetryEnd(JSON.stringify(request)));
    }
    HarmonyTelemetry.began = false;
    HarmonyTelemetry.session = '';
    HarmonyTelemetry.epoch = '';
    HarmonyTelemetry.enabled = false;
  }

  static preferencesChanged(stateDirectory: string): void {
    if (HarmonyTelemetry.preferences !== stateDirectory) return;
    HarmonyTelemetry.refresh();
  }

  static privacyChanged(stateDirectory: string): void {
    HarmonyTelemetry.preferencesChanged(stateDirectory);
  }

  private static consentQuery(): ConsentQuery {
    return { authority_directory: HarmonyTelemetry.blocked ? '' : HarmonyTelemetry.authority };
  }

  private static consentReply(): string {
    try { return client.telemetryConsent(JSON.stringify(HarmonyTelemetry.consentQuery())); }
    catch { return '{"ok":false}'; }
  }

  private static sharedConsent(stateDirectory: string): ReportingConsent | null {
    if (stateDirectory !== HarmonyTelemetry.preferences || HarmonyTelemetry.blocked || HarmonyTelemetry.stopped) return null;
    return TelemetryPolicy.sharedConsentReply(HarmonyTelemetry.consentReply());
  }

  private static request(stateDirectory: string, epoch?: string): TelemetryRequest {
    const request: TelemetryRequest = {
      directory: HarmonyTelemetry.directoryFor(stateDirectory), platform: PLATFORM, version: version(),
      component: HarmonyTelemetry.publisher ? 'application' : 'input',
    };
    request.authority_directory = HarmonyTelemetry.blocked ? '' : HarmonyTelemetry.authority;
    const consent: ReportingConsent | null = epoch === undefined ? HarmonyTelemetry.sharedConsent(stateDirectory) : null;
    // 授权错误必须明确拒绝，不能省略字段退回旧布尔协议。
    request.consent_epoch = epoch === undefined ? (consent === null ? '' : consent.epoch) : epoch;
    return request;
  }

  private static reportingAllowed(): boolean {
    if (HarmonyTelemetry.blocked || HarmonyTelemetry.stopped || !HarmonyTelemetry.enabled) return false;
    return HarmonyTelemetry.sharedConsent(HarmonyTelemetry.preferences) !== null;
  }

  private static clearDisabled(): void {
    if (!HarmonyTelemetry.cleared) {
      const request: DirectoryRequest = { directory: HarmonyTelemetry.directory, authority_directory: HarmonyTelemetry.authority };
      const reply: FlagReply | null = HarmonyTelemetry.flagReply('clear', client.telemetryClear(JSON.stringify(request)));
      HarmonyTelemetry.cleared = reply?.value === true;
    }
    HarmonyTelemetry.cancelBegin();
    HarmonyTelemetry.activeCapture = '';
    HarmonyTelemetry.publishCapture('');
    HarmonyTelemetry.enabled = false;
    HarmonyTelemetry.began = false;
    HarmonyTelemetry.session = '';
    HarmonyTelemetry.epoch = '';
  }

  private static publishConsent(): boolean {
    if (!HarmonyTelemetry.publisher || HarmonyTelemetry.authority === '') return true;
    const query: ConsentQuery = {
      directory: HarmonyTelemetry.preferences,
      authority_directory: HarmonyTelemetry.authority,
      publication: { policy_allowed: HarmonyPrivacyConsent.networkAllowed(), policy_identity: HarmonyPrivacyConsent.reportingIdentity() },
    };
    try {
      const reply: BeginReply = JSON.parse(client.telemetryConsent(JSON.stringify(query))) as BeginReply;
      return reply.ok === true;
    } catch { return false; }
  }

  /** 同步本地观察不等待任何 flush Promise；网络期间撤回仍可发布并被逐条发送核对。 */
  private static refresh(): void {
    if (HarmonyTelemetry.stopped || HarmonyTelemetry.blocked) return;
    if (!HarmonyTelemetry.publishConsent()) {
      if (!HarmonyTelemetry.publicationFailed) KeyboardLog.error('telemetry: reporting authorization publication failed');
      HarmonyTelemetry.publicationFailed = true;
      HarmonyTelemetry.enabled = false;
      HarmonyTelemetry.publishCapture('');
      return;
    }
    HarmonyTelemetry.publicationFailed = false;
    const reply: string = HarmonyTelemetry.consentReply();
    const consent: ReportingConsent | null = TelemetryPolicy.sharedConsentReply(reply);
    if (consent === null) {
      HarmonyTelemetry.enabled = false;
      HarmonyTelemetry.publishCapture('');
      if (TelemetryPolicy.sharedConsentDisabled(reply)) HarmonyTelemetry.clearDisabled();
      return;
    }
    HarmonyTelemetry.enabled = true;
    HarmonyTelemetry.cleared = false;
    if (consent.epoch === HarmonyTelemetry.epoch) {
      HarmonyTelemetry.publishCapture(HarmonyTelemetry.activeCapture);
      if (!HarmonyTelemetry.publisher && !HarmonyTelemetry.began && HarmonyTelemetry.beginTimer === -1) HarmonyTelemetry.scheduleBegin();
      if ((HarmonyTelemetry.publisher || HarmonyTelemetry.began) && TelemetryPolicy.flushDue(HarmonyTelemetry.lastFlush, Date.now()))
        HarmonyTelemetry.send(HarmonyTelemetry.request(HarmonyTelemetry.preferences, consent.epoch));
      return;
    }
    HarmonyTelemetry.cancelBegin();
    HarmonyTelemetry.began = false;
    HarmonyTelemetry.session = '';
    HarmonyTelemetry.activeCapture = '';
    HarmonyTelemetry.publishCapture('');
    const request: TelemetryRequest = HarmonyTelemetry.request(HarmonyTelemetry.preferences, consent.epoch);
    request.capture_only = true;
    let capture: string | null = null;
    try {
      const prepared: BeginReply = JSON.parse(client.telemetryBegin(JSON.stringify(request))) as BeginReply;
      if (prepared.ok !== true || prepared.value?.enabled !== true) {
        HarmonyTelemetry.enabled = false;
        return;
      }
      capture = TelemetryPolicy.systemCapture(prepared.value.session_id ?? '', prepared.value.crash_record_header ?? '', process.pid, Date.now(), consent);
      HarmonyTelemetry.activeCrashPath = prepared.value.crash_record_path ?? '';
    } catch { HarmonyTelemetry.enabled = false; return; }
    HarmonyTelemetry.activeCapture = capture ?? '';
    HarmonyTelemetry.publishCapture(HarmonyTelemetry.activeCapture);
    HarmonyTelemetry.epoch = consent.epoch;
    HarmonyTelemetry.lastFlush = 0;
    HarmonyTelemetry.watchCrashes();
    if (HarmonyTelemetry.publisher) HarmonyTelemetry.send(request);
    else HarmonyTelemetry.scheduleBegin();
  }

  private static scheduleBegin(): void {
    const epoch: string = HarmonyTelemetry.epoch;
    const generation: number = HarmonyTelemetry.startGeneration;
    HarmonyTelemetry.beginTimer = setTimeout((): void => {
      HarmonyTelemetry.beginTimer = -1;
      if (!HarmonyTelemetry.stopped && generation === HarmonyTelemetry.startGeneration && epoch === HarmonyTelemetry.epoch) HarmonyTelemetry.begin();
    }, BEGIN_DELAY_MS);
  }

  private static begin(): void {
    if (!HarmonyTelemetry.reportingAllowed()) return;
    const request: TelemetryRequest = HarmonyTelemetry.request(HarmonyTelemetry.preferences);
    if (request.consent_epoch !== HarmonyTelemetry.epoch) { HarmonyTelemetry.refresh(); return; }
    try {
      const parsed: BeginReply = JSON.parse(client.telemetryBegin(JSON.stringify(request))) as BeginReply;
      const session: string | undefined = parsed.value?.session_id;
      if (parsed.ok !== true || parsed.value?.enabled !== true || typeof session !== 'string' || session === '') return;
      HarmonyTelemetry.session = session;
      HarmonyTelemetry.began = true;
      if (!HarmonyTelemetry.publisher) HarmonyTelemetry.rememberPid();
      const consent: ReportingConsent | null = HarmonyTelemetry.sharedConsent(HarmonyTelemetry.preferences);
      HarmonyTelemetry.activeCapture = consent === null ? '' : TelemetryPolicy.systemCapture(session, parsed.value.crash_record_header ?? '', process.pid, Date.now(), consent) ?? '';
      HarmonyTelemetry.activeCrashPath = parsed.value.crash_record_path ?? '';
      HarmonyTelemetry.publishCapture(HarmonyTelemetry.activeCapture);
      HarmonyTelemetry.send(request);
    } catch (error) { KeyboardLog.error(`telemetry: begin refused: ${describeError(error)}`); }
  }

  private static send(request: TelemetryRequest): void {
    if (!HarmonyTelemetry.reportingAllowed()) return;
    HarmonyTelemetry.lastFlush = Date.now();
    if (!HarmonyPrivacyConsent.networkAllowed()) return;
    client.telemetryFlush(JSON.stringify(request)).then((reply: string): void => {
      HarmonyTelemetry.logRefusal('flush', reply);
    }).catch((error: Error): void => {
      KeyboardLog.error(`telemetry: flush failed: ${describeError(error)}`);
    });
  }

  private static watchCrashes(): void {
    if (HarmonyTelemetry.watcher !== undefined) return;
    const watcher: hiAppEvent.Watcher = {
      name: WATCHER_NAME,
      appEventFilters: [{ domain: hiAppEvent.domain.OS, names: [hiAppEvent.event.APP_CRASH, hiAppEvent.event.APP_FREEZE] }],
      onReceive: (domain: string, groups: Array<hiAppEvent.AppEventGroup>): void => {
        for (const group of groups) {
          for (const info of group.appEventInfos) {
            const report: CrashReport | null = info.name === hiAppEvent.event.APP_FREEZE
              ? TelemetryPolicy.freezeReport(JSON.stringify(info.params))
              : TelemetryPolicy.crashReport(JSON.stringify(info.params));
            if (report !== null) HarmonyTelemetry.recordCrash(report);
          }
        }
      }
    };
    try { hiAppEvent.addWatcher(watcher); HarmonyTelemetry.watcher = watcher; }
    catch (error) { KeyboardLog.error(`telemetry: crash watcher refused: ${describeError(error)}`); }
  }

  private static recordCrash(report: CrashReport): void {
    HarmonyTelemetry.refresh();
    HarmonyTelemetry.persistCrash(report, true);
  }

  /** 致命异常只同步保存，不刷新生命周期、不联网，随后仍执行系统原处理器。 */
  private static persistCrash(report: CrashReport, flush: boolean): void {
    if (!HarmonyTelemetry.reportingAllowed()) return;
    const consent: ReportingConsent | null = HarmonyTelemetry.sharedConsent(HarmonyTelemetry.preferences);
    if (consent === null) return;
    const original: SystemCapture | null = TelemetryPolicy.originalCapture(report, consent, Date.now());
    if (original === null) return;
    const request: CrashRecordRequest = {
      directory: HarmonyTelemetry.directory, message: report.message, stack: report.stack,
      crash_record_header: original.header, consent_epoch: consent.epoch,
      diagnostic_begin_unix_ms: report.time, diagnostic_end_unix_ms: report.time,
    };
    request.authority_directory = HarmonyTelemetry.authority;
    request.outside_session = true;
    if (report.outsideSession === true) request.diagnostic_id = TelemetryPolicy.standaloneRecordName(report);
    else request.diagnostic_capture = original.capture;
    const reply: FlagReply | null = HarmonyTelemetry.flagReply('record', client.telemetryRecordCrash(JSON.stringify(request)));
    if (reply !== null && reply.value !== true) {
      const path: string = `${HarmonyTelemetry.directory}/telemetry-crashes/${TelemetryPolicy.standaloneRecordName(report)}`;
      HarmonyTelemetry.atomicWrite(path, TelemetryPolicy.recordText(report), false);
    }
    if (reply !== null && flush) HarmonyTelemetry.send(HarmonyTelemetry.request(HarmonyTelemetry.preferences));
  }

  private static readRememberedPid(): number | null {
    const path: string = `${HarmonyTelemetry.directory}/harmony-keyboard-process.json`;
    let handle: fs.File | null = null;
    try {
      if (!HarmonyTelemetry.safeTelemetryDirectory()) return null;
      const expected: fs.Stat = fs.lstatSync(path);
      if (expected.isSymbolicLink() || !expected.isFile()) return null;
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > 256 || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      return TelemetryPolicy.rememberedPid(util.TextDecoder.create('utf-8').decodeToString(
        new Uint8Array(buffer, 0, count)));
    } catch (error) {
      try {
        if (fs.accessSync(path)) KeyboardLog.error(`telemetry: keyboard process unreadable: ${describeError(error)}`);
      } catch { }
      return null;
    } finally {
      if (handle !== null) NativeCleanup.close(handle);
    }
  }

  private static rememberPid(): void {
    const remembered: RememberedProcess = { pid: process.pid };
    const path: string = `${HarmonyTelemetry.directory}/harmony-keyboard-process.json`;
    if (!HarmonyTelemetry.prepareTelemetryDirectory()) return;
    HarmonyTelemetry.atomicWrite(path, JSON.stringify(remembered), true);
  }

  private static atomicWrite(path: string, text: string, replaceExisting: boolean): boolean {
    const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
    let renamed: boolean = false;
    try {
      const file: fs.File = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try { fs.writeSync(file.fd, text); } finally { fs.closeSync(file); }
      if (!replaceExisting && fs.accessSync(path)) return false;
      fs.renameSync(staging, path);
      renamed = true;
      return true;
    } finally {
      if (!renamed) {
        try { if (fs.accessSync(staging)) fs.unlinkSync(staging); } catch { }
      }
    }
  }

  private static prepareTelemetryDirectory(): boolean {
    try {
      if (!fs.accessSync(HarmonyTelemetry.directory)) fs.mkdirSync(HarmonyTelemetry.directory, true);
      return HarmonyTelemetry.safeTelemetryDirectory();
    } catch { return false; }
  }

  private static safeTelemetryDirectory(): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(HarmonyTelemetry.directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch { return false; }
  }

  private static installErrorHandler(): void {
    if (HarmonyTelemetry.errorHandlerInstalled) return;
    let previous: errorManager.ErrorHandler | undefined = undefined;
    const handler: errorManager.ErrorHandler = (error: Error): void => {
      try {
        if (!HarmonyTelemetry.stopped && HarmonyTelemetry.enabled && HarmonyTelemetry.activeCapture !== '') {
          const report: CrashReport = TelemetryPolicy.scriptCrash(process.pid, Date.now(),
            HarmonyTelemetry.activeCapture, error.name, error.stack ?? '');
          HarmonyTelemetry.persistCrash(report, false);
        }
      } catch { /* 采集失败不能阻断原异常处置，也不把异常消息写进日志。 */ }
      finally {
        if (previous) previous(error);
        else new process.ProcessManager().exit(1);
      }
    };
    try {
      previous = errorManager.setDefaultErrorHandler(handler);
      HarmonyTelemetry.errorHandlerInstalled = true;
      HarmonyTelemetry.errorHandlerFailed = false;
    } catch {
      if (!HarmonyTelemetry.errorHandlerFailed) KeyboardLog.error('telemetry: default error handler unavailable');
      HarmonyTelemetry.errorHandlerFailed = true;
    }
  }

  /** 原生武装同步完成；系统参数异步安装不影响 API 21 的本进程入口。 */
  private static prepareNativeCapture(identity: string): void {
    if (identity === HarmonyTelemetry.nativeCapture) return;
    try {
      if (identity === '') {
        client.telemetryNativeCapture('', '');
        HarmonyTelemetry.nativeCapture = '';
        HarmonyTelemetry.nativeCaptureFailed = false;
        return;
      }
      const capture: SystemCapture = JSON.parse(identity) as SystemCapture;
      if (client.telemetryNativeCapture(HarmonyTelemetry.activeCrashPath, capture.header)) {
        HarmonyTelemetry.nativeCapture = identity;
        HarmonyTelemetry.nativeCaptureFailed = false;
      } else {
        if (!HarmonyTelemetry.nativeCaptureFailed) KeyboardLog.error('telemetry: native capture unavailable');
        HarmonyTelemetry.nativeCaptureFailed = true;
      }
    } catch {
      if (!HarmonyTelemetry.nativeCaptureFailed) KeyboardLog.error('telemetry: native capture unavailable');
      HarmonyTelemetry.nativeCaptureFailed = true;
    }
  }

  /** 同一进程串行更新系统参数，迟到完成后只发布最新需求，绝不从异步回调启动遥测。 */
  private static publishCapture(identity: string): void {
    HarmonyTelemetry.prepareNativeCapture(identity);
    if (identity !== '') HarmonyTelemetry.installErrorHandler();
    HarmonyTelemetry.desiredCapture = identity;
    if (HarmonyTelemetry.capturePending || HarmonyTelemetry.installedCapture === identity) return;
    HarmonyTelemetry.capturePending = true;
    const parameters: Record<string, hiAppEvent.ParamType> = {};
    parameters['msime_capture'] = identity;
    let publishing: Promise<void>;
    try { publishing = hiAppEvent.setEventParam(parameters, hiAppEvent.domain.OS); }
    catch { HarmonyTelemetry.captureFinished(identity, false); return; }
    void publishing.then((): void => HarmonyTelemetry.captureFinished(identity, true))
      .catch((): void => HarmonyTelemetry.captureFinished(identity, false));
  }

  private static captureFinished(identity: string, succeeded: boolean): void {
    HarmonyTelemetry.capturePending = false;
    if (succeeded) {
      HarmonyTelemetry.installedCapture = identity;
      HarmonyTelemetry.capturePublicationFailed = false;
    } else {
      if (!HarmonyTelemetry.capturePublicationFailed) KeyboardLog.error('telemetry: system capture parameters unavailable');
      HarmonyTelemetry.capturePublicationFailed = true;
    }
    if (HarmonyTelemetry.desiredCapture !== identity) HarmonyTelemetry.publishCapture(HarmonyTelemetry.desiredCapture);
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
