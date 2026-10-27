/**
 * 首次启动的隐私政策授权：什么时候弹窗、同意了几号版本、没同意时哪些桥接请求不许发出去。
 *
 * 应用市场（中国大陆）审核要求首次启动时先征得同意、弹窗有拒绝选项；《个人信息保护法》对所有用户都适用，所以老用户升级后同样要看到一次。授权记录写在设置应用的 `files/state/privacy-consent.json`，经共享沙箱镜像给键盘（`SharedStateMirrorPolicy` 的 `privacy-consent` 条目），两个进程按同一条规则判断能不能联网。
 *
 * 拒绝不退出应用：本机打字、词库、皮肤、统计都不需要网络，照常可用；登录、云同步、社区、云剪贴板、检查更新、词库更新、匿名使用统计、公告，以及用户自己配置的云联想、在线翻译、AI 和云端语音一律不发请求，直到用户在「隐私」页重新打开弹窗并同意。拒绝是粘性的：之后启动不再自动弹窗，免得反复打扰（工信部 164 号文把拒绝后频繁弹窗列为违规）。
 *
 * 版本号只往上加：隐私政策有实质变化时把 `PRIVACY_POLICY_VERSION` 加一，已同意旧版本的用户下次打开应用时重新弹窗，同意之前联网功能保持关闭；拒绝过的用户本来就没有联网，不再追问。
 *
 * 这里只有纯规则，读写文件在 `HarmonyPrivacyConsent.ets`，弹窗在 `PrivacyConsentDialog.ets`。
 */

/** 授权记录的文件名，位于各进程的 `files/state`。 */
export const PRIVACY_CONSENT_FILE: string = "privacy-consent.json";
/** 共享沙箱镜像里这条记录的条目名。 */
export const PRIVACY_CONSENT_ENTRY: string = "privacy-consent";
/** 记录文件自身的格式号，与隐私政策的版本无关。 */
export const PRIVACY_CONSENT_FORMAT: number = 1;
/** 当前隐私政策的授权版本。隐私政策有实质变化时加一，已同意旧版本的用户会重新看到弹窗。 */
export const PRIVACY_POLICY_VERSION: number = 1;
/** 完整隐私政策的地址，与 `packages/ui/src/settings/app-resources.ts` 的 `privacyUrl` 相同。 */
export const PRIVACY_POLICY_URL: string = "https://msime.app/privacy/";
/** 用户协议的地址，与 `packages/ui/src/settings/app-resources.ts` 的 `termsUrl` 相同。 */
export const USER_AGREEMENT_URL: string = "https://msime.app/terms/";
/** 没有授权时桥接请求被拒绝的错误码，页面据此提示去「隐私」页同意。 */
export const PRIVACY_CONSENT_REQUIRED: string = "privacy_consent_required";

export type ConsentDecision = "granted" | "refused";

/** `privacy-consent.json` 的内容。 */
export interface ConsentRecord {
  format: number;
  /** 用户做决定时看到的隐私政策版本。 */
  version: number;
  decision: ConsentDecision;
  /** 做决定的时刻（毫秒），只作记录。 */
  decided_at_ms: number;
}

/** 弹窗里可以点开的协议文档。 */
export interface ConsentDocument {
  title: string;
  url: string;
}

/** 交给设置页的授权状态：`granted` 为当前版本已同意，`decided` 为用户对当前版本或更早版本做过决定。 */
export interface ConsentState {
  granted: boolean;
  decided: boolean;
}

/** 弹窗的文案，集中在这里由逻辑测试核对。 */
export interface ConsentCopy {
  title: string;
  intro: string;
  points: string[];
  linkPrefix: string;
  refusalNote: string;
  /** 已同意的用户从「隐私」页重新打开弹窗时多显示的一句。 */
  grantedNote: string;
  agree: string;
  refuse: string;
}

interface ParsedRecord {
  format?: Object;
  version?: Object;
  decision?: Object;
  decided_at_ms?: Object;
}

interface BridgeAction {
  operation?: Object;
  command?: Object;
}

/** 设置页桥接 `startRequest` 里会联网的请求种类。 */
const NETWORK_REQUESTS: string[] = [
  "account",
  "chat",
  "cloud_dictionary",
  "cloud_dictionary_snapshot",
  "cloud_clipboard",
  // WebDAV 设置的「测试连接」：用草稿里的地址和应用密码向网盘发大约二十个请求。
  "cloud_clipboard_webdav_probe",
  // 「我的词条」经 WebDAV 网盘同步（`msime_client_dictionary_sync`）：除 `status` 外的动作都可能联网，`run` 的 `local_change` 在发送的防抖到期时也会发出去。
  "dictionary_sync",
  "settings_sync",
  "ai_skin",
  "community_resource",
  "community_skin",
  "community_plugin",
  "typing_upload",
  "ai_models",
  "ai_test",
  "api_credential",
  "update_check",
  "voice_local_model",
];

/** 上面这些种类里不联网、没有授权也照常处理的操作：取消 Google 登录（华为账号登录的授权页、challenge 和登录都要联网，不在这里）、本机回复模板的保存与删除、结束皮肤试用，以及本地语音模型除下载以外的操作。 */
const LOCAL_OPERATIONS: Record<string, string[]> = {
  account: ["google_cancel"],
  chat: [],
  cloud_dictionary: [],
  cloud_dictionary_snapshot: [],
  cloud_clipboard: [],
  cloud_clipboard_webdav_probe: [],
  dictionary_sync: ["status"],
  settings_sync: [],
  ai_skin: ["cancel"],
  community_resource: ["store_reply", "remove_reply"],
  community_skin: ["finish_trial"],
  community_plugin: [],
  typing_upload: [],
  ai_models: [],
  ai_test: [],
  api_credential: [],
  update_check: [],
  voice_local_model: ["list", "cancel", "remove", "import"],
};

/** 词库更新里会联网的命令：`status`、`configure`、`rollback` 只读写本机。 */
const NETWORK_DICTIONARY_UPDATE_COMMANDS: string[] = ["automatic", "check"];

export class PrivacyConsentPolicy {
  /**
   * 弹窗里列出的协议，按显示顺序，弹窗按这个列表画链接。只列已经上线、点得开的页面：AppGallery 驳回过点了没反应的《用户协议》（#10009）。
   */
  static readonly DOCUMENTS: ConsentDocument[] = [
    { title: "《用户协议》", url: USER_AGREEMENT_URL },
    { title: "《隐私政策》", url: PRIVACY_POLICY_URL },
  ];

  static readonly COPY: ConsentCopy = {
    title: "欢迎使用水杉输入法",
    intro: "开始使用前，请阅读并同意用户协议和隐私政策。我们这样处理你的信息：",
    points: [
      "打字在本机完成：输入内容、个人词库和学习记录保存在手机上，不上传。",
      // 不提 AI：鸿蒙版拿到 AI 生成类资质之前 AI 功能全部隐藏（`AiGenerationGate`），弹窗不介绍用户用不到的功能；总闸打开时把「AI」加回这一句。
      "联网功能在你同意后才启用：账号登录、云同步、社区、云剪贴板、检查更新与词库更新、匿名使用统计和崩溃报告，以及你自己开启的云联想、在线翻译和云端语音服务。",
      "匿名使用统计不含输入内容，可随时在「我的 › 隐私」中关闭。",
    ],
    linkPrefix: "完整内容请阅读",
    refusalNote:
      "选择「不同意」仍可使用本机打字，联网功能保持关闭；以后可在「我的 › 隐私」中查看并同意。",
    grantedNote: "你已同意当前版本。选择「不同意」会撤回同意并关闭联网功能。",
    agree: "同意",
    refuse: "不同意",
  };

  /** 键盘里云端语音因没有授权而不能启动时显示的话。 */
  static readonly KEYBOARD_REFUSED: string =
    "未同意隐私政策，云端服务不可用。请在水杉输入法应用的「隐私」中查看并同意";

  /** 读取记录；文件不存在、读不懂或字段不全时为 null，按从未做过决定处理。 */
  static parse(text: string | null): ConsentRecord | null {
    if (text === null || text.length === 0 || text.length > 4096) return null;
    let parsed: ParsedRecord;
    try {
      parsed = JSON.parse(text) as ParsedRecord;
    } catch {
      return null;
    }
    if (parsed === null || typeof parsed !== "object") return null;
    const format: Object | undefined = parsed.format;
    const version: Object | undefined = parsed.version;
    const decision: Object | undefined = parsed.decision;
    const decidedAt: Object | undefined = parsed.decided_at_ms;
    if (typeof format !== "number" || format !== PRIVACY_CONSENT_FORMAT) return null;
    if (typeof version !== "number" || !Number.isInteger(version) || version < 1) return null;
    if (decision !== "granted" && decision !== "refused") return null;
    const decidedAtMs: number =
      typeof decidedAt === "number" && Number.isFinite(decidedAt) ? decidedAt : 0;
    return {
      format: PRIVACY_CONSENT_FORMAT,
      version: version as number,
      decision: decision as ConsentDecision,
      decided_at_ms: decidedAtMs,
    };
  }

  /** 用户刚做的决定，记在 `version` 上。 */
  static record(granted: boolean, version: number, nowMs: number): ConsentRecord {
    return {
      format: PRIVACY_CONSENT_FORMAT,
      version: version,
      decision: granted ? "granted" : "refused",
      decided_at_ms: nowMs,
    };
  }

  static serialize(record: ConsentRecord): string {
    return JSON.stringify(record);
  }

  /** 打开设置应用时要不要弹窗：从没决定过，或同意的是更早的版本。拒绝过的不再自动弹窗。 */
  static shouldPrompt(record: ConsentRecord | null, currentVersion: number): boolean {
    if (record === null) return true;
    return record.decision === "granted" && record.version < currentVersion;
  }

  /** 现在能不能联网：只有同意了当前（或更新的）版本才可以。 */
  static networkAllowed(record: ConsentRecord | null, currentVersion: number): boolean {
    return record !== null && record.decision === "granted" && record.version >= currentVersion;
  }

  static state(record: ConsentRecord | null, currentVersion: number): ConsentState {
    return {
      granted: PrivacyConsentPolicy.networkAllowed(record, currentVersion),
      decided: record !== null,
    };
  }

  /** 设置页经 `startRequest` 发来的这个请求是否要联网，因而需要授权。 */
  static requestNeedsConsent(kind: string, payload: string): boolean {
    if (!NETWORK_REQUESTS.includes(kind)) return false;
    const local: string[] | undefined = LOCAL_OPERATIONS[kind];
    if (local === undefined || local.length === 0) return true;
    const operation: string = PrivacyConsentPolicy.operationOf(payload);
    return !local.includes(operation);
  }

  /** 设置页同步桥接 `dictionary` 的这个动作是否要联网：只有词库更新的 `automatic` 和 `check` 会去下载。 */
  static dictionaryActionNeedsConsent(actionDocument: string): boolean {
    let action: BridgeAction;
    try {
      action = JSON.parse(actionDocument) as BridgeAction;
    } catch {
      return false;
    }
    if (action === null || typeof action !== "object" || action.operation !== "updates")
      return false;
    const command: Object | undefined = action.command;
    return (
      typeof command === "string" && NETWORK_DICTIONARY_UPDATE_COMMANDS.includes(command as string)
    );
  }

  /** 没有授权时这个请求是否要拒绝。 */
  static refuses(kind: string, payload: string, allowed: boolean): boolean {
    return !allowed && PrivacyConsentPolicy.requestNeedsConsent(kind, payload);
  }

  /** 拒绝时交给页面的应答，与共享 C ABI 的失败信封同形。 */
  static refusal(): string {
    return `{"ok":false,"error":"${PRIVACY_CONSENT_REQUIRED}"}`;
  }

  private static operationOf(payload: string): string {
    try {
      const action: BridgeAction = JSON.parse(payload) as BridgeAction;
      if (action !== null && typeof action === "object" && typeof action.operation === "string") {
        return action.operation as string;
      }
    } catch {
      // 读不懂的请求按联网处理。
    }
    return "";
  }
}
