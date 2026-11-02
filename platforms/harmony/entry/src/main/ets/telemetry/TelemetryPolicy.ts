/** 本宿主只适配系统诊断与生命周期；队列、去重和持续授权由共享层持有。系统参数固定故障前的身份，升级补报不得重建原构建。 */

/** One crash HiAppEvent reported, reduced to what a crash record holds. */
export interface CrashReport {
  pid: number;
  /** 系统在原进程生命周期保存的采集身份，缺失时不能追认原构建。 */
  capture?: string;
  /** 卡死等系统处置独立记录，不记作输入会话崩溃。 */
  outsideSession?: boolean;
  /** 系统诊断里的原始版本，缺失时不冒用当前安装版本。 */
  version?: string;
  /** When the crash happened, in milliseconds since the epoch. */
  time: number;
  /** The exception or signal summary, one line. */
  message: string;
  /** Frames, one per line: `file+0xpc symbol` for native code, the JavaScript stack as the engine wrote it. */
  stack: string;
}

/** 故障前固定的捕获归属；只交给本机系统诊断，不直接作为事件字段上传。 */
export interface SystemCapture {
  format: number;
  capture: string;
  header: string;
  pid: number;
  since: number;
}

/** Rust 权威来源产生的授权；宿主不能自行生成。 */
export interface ReportingConsent {
  epoch: string;
  since: number;
}

/** How often a long-running keyboard sends what it has queued, so a process that lives for days still reports each day. */
const FLUSH_INTERVAL_MS: number = 6 * 60 * 60 * 1000;
const MAX_FRAMES: number = 64;

const SIGNALS: Map<number, string> = new Map<number, string>([
  [4, "SIGILL"],
  [5, "SIGTRAP"],
  [6, "SIGABRT"],
  [7, "SIGBUS"],
  [8, "SIGFPE"],
  [11, "SIGSEGV"],
  [31, "SIGSYS"],
]);

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function integer(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function firstLine(value: string): string {
  const line: string = value.split("\n")[0] ?? "";
  return line.trim();
}

/** 原生帧只留模块名、偏移、合法 ELF build-id 与符号，不上传沙箱路径。 */
function nativeFrame(value: unknown): string | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const frame = value as Record<string, unknown>;
  const file: string = text(frame.file).split("/").pop() ?? "";
  const pc: string = text(frame.pc)
    .replace(/^0x/i, "")
    .replace(/^0+(?=.)/, "");
  const symbol: string = text(frame.symbol).trim();
  if (file === "" && pc === "" && symbol === "") return null;
  let line: string = file === "" ? "<unknown>" : file;
  if (/^[0-9a-fA-F]{1,16}$/.test(pc)) line += `+0x${pc}`;
  const buildId: string = text(frame.buildId);
  if (/^(?:[0-9a-fA-F]{2}){1,64}$/.test(buildId)) line += ` [id 0x${buildId.toLowerCase()}]`;
  if (symbol !== "") line += ` ${symbol}`;
  return line;
}

export class TelemetryPolicy {
  /** 系统补录与 API 21 默认处理器共用裁剪规则，绝不读取异常消息。 */
  static scriptCrash(
    pid: number,
    time: number,
    capture: string,
    name: string,
    stack: string,
  ): CrashReport {
    const kind: string = firstLine(name);
    return {
      pid,
      time,
      capture,
      message: /^(Error|TypeError|ReferenceError|SyntaxError|RangeError|URIError|EvalError)$/.test(
        kind,
      )
        ? kind
        : "JavaScript error",
      stack: stack
        .split("\n")
        .map((line: string): string => line.trim())
        .filter((line: string): boolean => line.startsWith("at "))
        .slice(0, MAX_FRAMES)
        .join("\n"),
    };
  }

  /** 响应须为明确开启、规范 UUID 和正整数起点；缺失或错误不能回退旧协议。 */
  static sharedConsentReply(document: string): ReportingConsent | null {
    try {
      const reply: unknown = JSON.parse(document);
      if (reply === null || typeof reply !== "object" || Array.isArray(reply)) return null;
      const envelope = reply as Record<string, unknown>;
      const value: unknown = envelope.value;
      if (
        envelope.ok !== true ||
        value === null ||
        typeof value !== "object" ||
        Array.isArray(value)
      )
        return null;
      const fields = value as Record<string, unknown>;
      const epoch: string = text(fields.epoch);
      const since: number | null = integer(fields.since_unix_ms);
      if (
        fields.enabled !== true ||
        since === null ||
        since <= 0 ||
        !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(epoch) ||
        epoch === "00000000-0000-0000-0000-000000000000"
      )
        return null;
      return { epoch, since };
    } catch {
      return null;
    }
  }

  /** 只有明确的成功关闭响应允许清理；错误和损坏响应只停采。 */
  static sharedConsentDisabled(document: string): boolean {
    try {
      const reply: unknown = JSON.parse(document);
      if (reply === null || typeof reply !== "object" || Array.isArray(reply)) return false;
      const envelope = reply as Record<string, unknown>;
      const value: unknown = envelope.value;
      return (
        envelope.ok === true &&
        value !== null &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        (value as Record<string, unknown>).enabled === false &&
        ((value as Record<string, unknown>).suspended === undefined ||
          (value as Record<string, unknown>).suspended === false)
      );
    } catch {
      return false;
    }
  }

  /** 延迟诊断只认领本轮持续同意之后发生的事件，不能用今天的开关追认过去的故障。 */
  static diagnosticAllowed(report: CrashReport, consentSince: number | null, now: number): boolean {
    return (
      consentSince !== null &&
      Number.isSafeInteger(consentSince) &&
      consentSince > 0 &&
      report.time >= consentSince &&
      report.time <= now
    );
  }

  /** 卡死只保留标准原因，不读取系统日志、页面历史或异常消息。 */
  static freezeReport(paramsJson: string): CrashReport | null {
    let parsed: unknown;
    try {
      parsed = JSON.parse(paramsJson);
    } catch {
      return null;
    }
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    const params = parsed as Record<string, unknown>;
    const pid: number | null = integer(params.pid);
    const time: number | null = integer(params.time);
    if (pid === null || time === null) return null;
    const exception: unknown = params.exception;
    const name: string =
      exception !== null && typeof exception === "object" && !Array.isArray(exception)
        ? text((exception as Record<string, unknown>).name)
        : "";
    const safeName: string =
      /^(THREAD_BLOCK_6S|APP_INPUT_BLOCK|INPUT_BLOCK|LIFECYCLE_TIMEOUT|BUSSINESS_THREAD_BLOCK_6S)$/.test(
        name,
      )
        ? name
        : "unknown";
    return {
      pid,
      time,
      version: text(params.bundle_version),
      capture: text(params.msime_capture),
      outsideSession: true,
      message: `Hang ${safeName}`,
      stack: "",
    };
  }

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
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    const params = parsed as Record<string, unknown>;
    const pid: number | null = integer(params.pid);
    const time: number | null = integer(params.time);
    const exception: unknown = params.exception;
    if (pid === null || time === null || exception === null || typeof exception !== "object") {
      return null;
    }
    const details = exception as Record<string, unknown>;
    const crashType: string = text(params.crash_type);
    if (crashType === "JsError") {
      const report: CrashReport = TelemetryPolicy.scriptCrash(
        pid,
        time,
        text(params.msime_capture),
        text(details.name),
        text(details.stack),
      );
      report.version = text(params.bundle_version);
      return report;
    }
    if (crashType === "NativeCrash") {
      const signal: unknown = details.signal;
      let summary: string = "";
      if (signal !== null && typeof signal === "object" && !Array.isArray(signal)) {
        const fields = signal as Record<string, unknown>;
        const number: number | null = integer(fields.signo);
        const code: number | null = integer(fields.code);
        if (number !== null) {
          const name: string = SIGNALS.get(number) ?? `signal ${number}`;
          const described: string = code === null ? name : `${name} (code ${code})`;
          summary = described;
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
        version: text(params.bundle_version),
        capture: text(params.msime_capture),
        message: summary === "" ? "native crash" : summary,
        stack: frames.join("\n"),
      };
    }
    return null;
  }

  /** The file name of a standalone crash record: unique per crash, so a report delivered twice is written once. Not a UUID, so client-core gives the event an id of its own. */
  static standaloneRecordName(report: CrashReport): string {
    return `harmony-${report.outsideSession === true ? "freeze-" : ""}${report.pid}-${report.time}.crash`;
  }

  /** 保留共享层产生的原构建头；长度限制来自系统参数协议，不能截断身份。 */
  static systemCapture(
    capture: string,
    header: string,
    pid: number,
    now: number,
    consent: ReportingConsent,
  ): string | null {
    const identity: SystemCapture = { format: 1, capture, header, pid, since: now };
    const encoded: string = JSON.stringify(identity);
    const report: CrashReport = { pid, time: now, message: "", stack: "", capture: encoded };
    return TelemetryPolicy.originalCapture(report, consent, now) === null ? null : encoded;
  }

  /** 只认系统保存的原进程、原版本和本轮授权，补报时不从当前包重建头。 */
  static originalCapture(
    report: CrashReport,
    consent: ReportingConsent,
    now: number,
  ): SystemCapture | null {
    const encoded: string = report.capture ?? "";
    if (
      encoded.length === 0 ||
      encoded.length > 1024 ||
      !TelemetryPolicy.diagnosticAllowed(report, consent.since, now)
    )
      return null;
    try {
      const parsed: unknown = JSON.parse(encoded);
      if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
      const fields = parsed as Record<string, unknown>;
      const header: string = text(fields.header);
      const capture: string = text(fields.capture);
      const pid: number | null = integer(fields.pid);
      const since: number | null = integer(fields.since);
      if (
        fields.format !== 1 ||
        pid === null ||
        pid <= 0 ||
        pid !== report.pid ||
        since === null ||
        since < consent.since ||
        since > report.time ||
        !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(capture) ||
        !header.startsWith("MSIME-CRASH/1 ")
      )
        return null;
      const metadata: unknown = JSON.parse(header.slice("MSIME-CRASH/1 ".length));
      if (metadata === null || typeof metadata !== "object" || Array.isArray(metadata)) return null;
      const context = metadata as Record<string, unknown>;
      if (
        context.platform !== "harmony" ||
        context.consent_epoch !== consent.epoch ||
        (context.component !== "input" && context.component !== "application") ||
        (report.version !== undefined &&
          report.version !== "" &&
          context.version !== report.version)
      )
        return null;
      for (const key of ["version", "revision", "run", "attempt", "arch", "component"]) {
        if (
          typeof context[key] !== "string" ||
          !/^[A-Za-z0-9._-]{0,64}$/.test(context[key] as string)
        )
          return null;
      }
      if (context.version === "" || context.arch === "") return null;
      return { format: 1, capture, header, pid, since };
    } catch {
      return null;
    }
  }

  /** Whether a running keyboard should send its queue again. */
  static flushDue(lastFlushMs: number, nowMs: number): boolean {
    return lastFlushMs <= 0 || nowMs - lastFlushMs >= FLUSH_INTERVAL_MS || nowMs < lastFlushMs;
  }
}
