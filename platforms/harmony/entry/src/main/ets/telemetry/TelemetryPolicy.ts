/**
 * The decisions behind anonymous usage reporting on this host, kept apart from the ArkTS that performs them so they run under node.
 *
 * The queue, the install id, the daily `active` and the session bookkeeping are all client-core's (`msime_client_telemetry_*`); this host only decides when to call it and turns the system's crash reports into crash records. A crash is reported by HarmonyOS itself: HiAppEvent hands the previous run's `APP_CRASH` (JavaScript or native) to a watcher on the next start. Nothing here installs a handler of its own, so a crash still ends the process exactly as it did before reporting existed.
 */

/** One crash HiAppEvent reported, reduced to what a crash record holds. */
export interface CrashReport {
  pid: number;
  /** When the crash happened, in milliseconds since the epoch. */
  time: number;
  /** The exception or signal summary, one line. */
  message: string;
  /** Frames, one per line: `file+0xpc symbol` for native code, the JavaScript stack as the engine wrote it. */
  stack: string;
}

/** What to do with a crash report. */
export enum CrashDestination {
  /** The keyboard session that crashed has not been closed yet: record it as that session's crash, so it counts as a `session_crash`. */
  Session,
  /** Any other crash (the settings application, or a keyboard session already closed): a standalone crash record, sent as a `crash` only. */
  Standalone,
}

/** How often a long-running keyboard sends what it has queued, so a process that lives for days still reports each day. */
const FLUSH_INTERVAL_MS: number = 6 * 60 * 60 * 1000;
const MAX_FRAMES: number = 64;

const SIGNALS: Map<number, string> = new Map<number, string>([
  [4, 'SIGILL'],
  [5, 'SIGTRAP'],
  [6, 'SIGABRT'],
  [7, 'SIGBUS'],
  [8, 'SIGFPE'],
  [11, 'SIGSEGV'],
  [31, 'SIGSYS'],
]);

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function integer(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function firstLine(value: string): string {
  const line: string = value.split('\n')[0] ?? '';
  return line.trim();
}

/** One native frame as `libmsimeclient.so+0x1a2b symbol`: the file name only, so no sandbox directory leaves the device. */
function nativeFrame(value: unknown): string | null {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return null;
  const frame = value as Record<string, unknown>;
  const file: string = text(frame.file).split('/').pop() ?? '';
  const pc: string = text(frame.pc).replace(/^0x/i, '').replace(/^0+(?=.)/, '');
  const symbol: string = text(frame.symbol).trim();
  if (file === '' && pc === '' && symbol === '') return null;
  let line: string = file === '' ? '<unknown>' : file;
  if (pc !== '') line += `+0x${pc}`;
  if (symbol !== '') line += ` ${symbol}`;
  return line;
}

export class TelemetryPolicy {
  /**
   * The crash in one HiAppEvent `APP_CRASH` event's parameters (as JSON), or null when they are not a crash this can describe.
   *
   * A JavaScript crash carries `exception: {name, message, stack}`; a native one `exception: {message, signal: {signo, code}, frames: [{file, pc, symbol}]}`. The faulting address is left out on purpose: it differs between two runs of the same bug and would split one crash group into many.
   */
  static crashReport(paramsJson: string): CrashReport | null {
    let parsed: unknown;
    try {
      parsed = JSON.parse(paramsJson);
    } catch {
      return null;
    }
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    const params = parsed as Record<string, unknown>;
    const pid: number | null = integer(params.pid);
    const time: number | null = integer(params.time);
    const exception: unknown = params.exception;
    if (pid === null || time === null || exception === null || typeof exception !== 'object') {
      return null;
    }
    const details = exception as Record<string, unknown>;
    const crashType: string = text(params.crash_type);
    if (crashType === 'JsError') {
      const name: string = firstLine(text(details.name));
      const message: string = firstLine(text(details.message));
      const summary: string = name !== '' && message !== '' ? `${name}: ${message}` : name + message;
      return {
        pid,
        time,
        message: summary === '' ? 'JavaScript error' : summary,
        stack: text(details.stack).trim(),
      };
    }
    if (crashType === 'NativeCrash') {
      const signal: unknown = details.signal;
      let summary: string = firstLine(text(details.message));
      if (signal !== null && typeof signal === 'object' && !Array.isArray(signal)) {
        const fields = signal as Record<string, unknown>;
        const number: number | null = integer(fields.signo);
        const code: number | null = integer(fields.code);
        if (number !== null) {
          const name: string = SIGNALS.get(number) ?? `signal ${number}`;
          const described: string = code === null ? name : `${name} (code ${code})`;
          summary = summary === '' ? described : `${described}: ${summary}`;
        }
      }
      const frames: string[] = [];
      if (Array.isArray(details.frames)) {
        for (const frame of details.frames as unknown[]) {
          if (frames.length >= MAX_FRAMES) break;
          const line: string | null = nativeFrame(frame);
          if (line !== null) frames.push(line);
        }
      }
      return {
        pid,
        time,
        message: summary === '' ? 'native crash' : summary,
        stack: frames.join('\n'),
      };
    }
    return null;
  }

  /**
   * Where a crash belongs.
   *
   * It is the unfinished keyboard session's crash only when it is that session's process (`previousKeyboardPid`, remembered when that session began) and this keyboard has not begun its own session yet: until then the session marker on disk is still the crashed one, and once begun it is this run's. Everything else is a standalone crash.
   */
  static destination(report: CrashReport, previousKeyboardPid: number | null, begun: boolean): CrashDestination {
    if (!begun && previousKeyboardPid !== null && report.pid === previousKeyboardPid) {
      return CrashDestination.Session;
    }
    return CrashDestination.Standalone;
  }

  /** The file name of a standalone crash record: unique per crash, so a report delivered twice is written once. Not a UUID, so client-core gives the event an id of its own. */
  static standaloneRecordName(report: CrashReport): string {
    return `harmony-${report.pid}-${report.time}.crash`;
  }

  /** A crash record's contents: the summary line, then the frames. */
  static recordText(report: CrashReport): string {
    return `${report.message}\n${report.stack}`;
  }

  /**
   * Whether the preferences snapshot the settings page saved, with `saveReply` the store's answer, turned usage reporting off.
   *
   * `usage_reporting` is omitted while it is on, so only an explicit `false` turns it off; a refused save (a conflict with the keyboard's own write) changed nothing.
   */
  static reportingTurnedOff(snapshotJson: string, saveReply: string): boolean {
    try {
      const reply: unknown = JSON.parse(saveReply);
      if (reply === null || typeof reply !== 'object' || (reply as Record<string, unknown>).ok !== true) {
        return false;
      }
      const snapshot: unknown = JSON.parse(snapshotJson);
      if (snapshot === null || typeof snapshot !== 'object') return false;
      const preferences: unknown = (snapshot as Record<string, unknown>).preferences;
      if (preferences === null || typeof preferences !== 'object') return false;
      return (preferences as Record<string, unknown>).usage_reporting === false;
    } catch {
      return false;
    }
  }

  /** Whether a running keyboard should send its queue again. */
  static flushDue(lastFlushMs: number, nowMs: number): boolean {
    return lastFlushMs <= 0 || nowMs - lastFlushMs >= FLUSH_INTERVAL_MS || nowMs < lastFlushMs;
  }

  /** The keyboard process's id as remembered between runs, or null when the file is absent or unreadable. */
  static rememberedPid(document: string): number | null {
    try {
      const value: unknown = JSON.parse(document);
      if (value === null || typeof value !== 'object') return null;
      return integer((value as Record<string, unknown>).pid);
    } catch {
      return null;
    }
  }
}
