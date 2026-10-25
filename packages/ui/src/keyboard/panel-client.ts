import type { EmojiCatalogGroup } from "../emoji/emoji-catalog";
import type { ModelMirrorClient, ResourcePackClient } from "../settings/resource-packs";
import type { CloudDictionaryKind } from "./cloud-dictionary-kind-tabs";
import type { WindowResizeEdge } from "./window-host";

export interface KeyboardInputRequest {
  virtual_key: number;
  shift: boolean;
  modifiers: { ctrl: boolean; alt: boolean; win: boolean };
  include_sticky_modifiers: boolean;
  grouped_letter?: string;
}

export interface InkPoint {
  x: number;
  y: number;
}
export interface InkStroke {
  points: InkPoint[];
}
export interface HandwritingRecognitionRequest {
  language: string;
  strokes: InkStroke[];
}
export interface HandwritingRecognitionResult {
  candidates: string[];
}

export interface PanelClient {
  close(): Promise<void>;
  openVoice?(): Promise<void>;
  beginWindowDrag?(): Promise<void>;
  rememberInputTarget?(): Promise<void>;
  sendKey?(request: KeyboardInputRequest): Promise<void>;
  sendText?(text: string): Promise<void>;
  recognizeHandwriting?(
    request: HandwritingRecognitionRequest,
  ): Promise<HandwritingRecognitionResult>;
  submitHandwritingCandidate?(candidate: string): Promise<void>;
  copyHandwritingCandidate?(candidate: string): Promise<void>;
  /** 桌面发布包不再内置手写模型：宿主列出手写模型时，手写面板第一次打开就用它下载；没列出（例如 Windows Ink 有中文识别器）时面板照常识别。 */
  resourcePacks?: ResourcePackClient;
  /** 读写已保存的下载镜像（`voice_input.asr_model_mirror`）：手写模型下载失败时，面板就地提供镜像设置。 */
  modelMirror?: ModelMirrorClient;
}

export interface VoicePanelClient extends PanelClient {
  maxSubmitBytes?: number;
  /** 宿主在录音控件上方显示的平台说明。 */
  description?: string;
  /** 宿主将结果交给另一个原生界面时，可提供交接提示文案。 */
  submitNotice?: string;
  loadVoiceLanguage?(): Promise<string>;
  recognizeVoice?(language: string): Promise<{ text: string }>;
  onVoiceUpdate?(
    listener: (update: {
      text: string;
      final: boolean;
      phase?: "recording" | "recognizing" | "polishing";
      level?: number;
    }) => void,
  ): Promise<() => void>;
  cancelVoice?(): Promise<void>;
  stopVoice?(): Promise<void>;
  sendVoiceText?(text: string): Promise<void>;
  copyText?(text: string): Promise<void>;
}

/** 云剪贴板面板发给宿主的动作，见 `docs/cloud-clipboard-e2e.md` 的「共享面板的动作」。 */
export type CloudClipboardAction =
  | { operation: "list"; search: string }
  | { operation: "add"; text: string }
  | { operation: "delete"; id: string }
  | { operation: "set_enabled"; enabled: boolean }
  | { operation: "devices" }
  | { operation: "approve" | "reject" | "remove"; device_id: string }
  | { operation: "reset" };
export type CloudClipboardItem = {
  id: string;
  text: string;
  updated_at?: string;
  pinned?: boolean;
  device?: string;
};
/** 宿主的回答：`list` 带 `items`、`enabled`、`retention_days` 和 `e2e`，设备管理动作带 `status`、`device_id`、`code` 和 `devices`。各字段由 `cloud-response` 里的读取函数逐项核对。 */
export type CloudClipboardResponse = {
  items?: CloudClipboardItem[];
  enabled?: boolean;
  retention_days?: number;
  e2e?: unknown;
  status?: unknown;
  device_id?: unknown;
  code?: unknown;
  devices?: unknown;
};
export interface CloudClipboardPanelClient extends PanelClient {
  canSendText?(): Promise<boolean>;
  copyText?(text: string): Promise<void>;
  request(action: CloudClipboardAction): Promise<CloudClipboardResponse>;
}

export type { CloudDictionaryKind } from "./cloud-dictionary-kind-tabs";
export type CloudCandidateKind = CloudDictionaryKind | "jianpin";
export type CloudRankingMode = "disabled" | "pin" | "halve" | "linear" | "promote";
export type CloudDictionaryFileFormat = "standard" | "windows" | "hans";
export type CloudDictionaryAction =
  | { operation: "snapshot_preview" }
  | { operation: "snapshot_export" }
  | { operation: "snapshot_restore_preview"; text: string }
  | { operation: "snapshot_restore"; text: string; expected_sha256: string; revision: number }
  | { operation: "snapshot_restore_native"; token: string }
  | { operation: "snapshot_restore_cancel" }
  | { operation: "snapshot_choose_restore" }
  | { operation: "snapshot_enqueue"; token: string }
  | { operation: "snapshot_status" }
  | { operation: "snapshot_cancel" }
  | { operation: "list"; kind: CloudDictionaryKind; offset: number; search: string }
  | {
      operation: "catalog";
      kind: CloudDictionaryKind;
      code: string;
      offset: number;
      scheme: string;
      profile: string;
    }
  | { operation: "add"; kind: CloudDictionaryKind; code: string; word: string; weight: number }
  | {
      operation: "update";
      kind: CloudDictionaryKind;
      id: string;
      code: string;
      word: string;
      weight: number;
      revision: number;
    }
  | {
      operation: "edit_catalog";
      kind: CloudDictionaryKind;
      code: string;
      word: string;
      revision: number;
      replacement: { code: string; word: string; weight: number } | null;
    }
  | {
      operation: "candidates";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
    }
  | {
      operation: "rank";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
      code: string;
      word: string;
      revision: number;
      mode: CloudRankingMode;
      linear_step: number;
      trigger_count: number;
      force_top: boolean;
    }
  | {
      operation: "remove_candidate";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
      code: string;
      word: string;
      revision: number;
    }
  | { operation: "fixed_positions"; context: string; offset: number }
  | {
      operation: "set_fixed_position";
      context: string;
      code: string;
      word: string;
      position: number | null;
      revision: number;
    }
  | { operation: "delete"; kind: CloudDictionaryKind; id: string; revision: number }
  | {
      operation: "import";
      kind: CloudDictionaryKind;
      format: CloudDictionaryFileFormat;
      text: string;
    }
  | {
      operation: "export";
      kind: CloudDictionaryKind;
      format: Exclude<CloudDictionaryFileFormat, "hans">;
    };
export type CloudDictionaryEntry = {
  id: string;
  kind: CloudDictionaryKind;
  code: string;
  word: string;
  weight: number;
  revision: number;
};
export type CloudDictionaryCatalogEntry = {
  kind: CloudDictionaryKind;
  code: string;
  word: string;
  weight: number;
};
export type CloudCandidate = {
  code: string;
  word: string;
  weight: number;
  canonical_pinyin?: string | null;
};
export type CloudFixedPosition = { context: string; code: string; word: string; position: number };
export type CloudDictionarySnapshotMetadata = {
  cloudRevision: number;
  sha256: string;
  bytes: number;
  records: number;
  entries: number;
  overlays: number;
  positions: number;
  selections: number;
};
export type CloudDictionarySnapshotRequest = {
  id: string;
  cloudRevision: number;
  expectedLocalVersion?: string;
  fileSha256: string;
  status: "queued" | "preparing" | "applied" | "conflict" | "failed" | "cancelled";
};
export type CloudDictionaryResponse = {
  saved?: boolean;
  previewToken?: string;
  snapshot?: CloudDictionarySnapshotMetadata;
  expectedRevision?: number;
  localVersion?: string;
  request?: CloudDictionarySnapshotRequest;
  entries?: CloudDictionaryEntry[];
  catalog_entries?: CloudDictionaryCatalogEntry[];
  candidates?: CloudCandidate[];
  positions?: CloudFixedPosition[];
  has_more?: boolean;
  offset?: number;
  revision?: number;
  context?: string;
  normalized?: string;
  changed?: boolean;
  selection_count?: number;
  text?: string;
  content?: string;
  filename?: string;
};
export interface CloudDictionaryPanelClient extends PanelClient {
  request(action: CloudDictionaryAction): Promise<CloudDictionaryResponse>;
  snapshot?: boolean;
  snapshotNative?: boolean;
  /** 「应用到本机」由这个面板自己处理队列（Windows 设置应用）：入列后马上应用、输入暂停，没能应用的请求只在面板开着时重试，输入法不会在空闲时自己应用。 */
  snapshotAppliesInPanel?: boolean;
  exportNative?: boolean;
  /** 桌面设置宿主保存导出文本，避免 WebView 取消浏览器下载。 */
  saveExport?(name: string, contents: string): Promise<void>;
  chooseSnapshotRestore?(): Promise<CloudDictionaryResponse>;
  downloadToLocal?(entry: CloudDictionaryEntry): Promise<void>;
  openCatalog?(): Promise<void>;
  openCandidates?(): Promise<void>;
  openFiles?(): Promise<void>;
  openApply?(): Promise<void>;
  back?(): Promise<void>;
}

/** 宿主剪贴板采集最近一轮的结果，文案与 macOS 原生剪贴板页的状态行一致。`read_unavailable*` 是 Linux 读不到系统剪贴板、剪贴板服务也没在运行：`_x11` 装 xclip 或 xsel 能读，`_wayland` 装 wl-clipboard 能读，不带后缀的是这个会话里装什么都读不到。 */
export type ClipboardCaptureStatus =
  | "disabled"
  | "monitoring"
  | "unavailable"
  | "read_unavailable"
  | "read_unavailable_x11"
  | "read_unavailable_wayland"
  | "save_failed";

export interface EmojiPanelClient extends PanelClient {
  /** 宿主允许调整表情与符号窗口的大小时，处理边缘拖动。 */
  resizeWindow?(edge: WindowResizeEdge): Promise<void>;
  copyText?(text: string): Promise<void>;
  /** 「输入到原应用」或「粘贴」成功后关闭面板，与 macOS 的表情面板一样只插入一次（Linux 设置宿主）。复制不关闭。 */
  closeAfterInsert?: boolean;
  clipboard?: {
    list?(): Promise<string[]>;
    isEnabled?(): Promise<boolean>;
    enable?(): Promise<void>;
    onChanged?(listener: () => void): Promise<() => void>;
    sync?(): Promise<string[]>;
    remove?(text: string): Promise<void>;
    clear?(): Promise<void>;
    copy?(text: string): Promise<void>;
    paste?(text: string): Promise<void>;
    /** 把这一条本地记录发到云剪贴板（`cloud_clipboard_request` 的 `add`），与 macOS 原生剪贴板行的右键菜单同一个动作。 */
    sendToCloud?(text: string): Promise<void>;
    /** 采集状态；`null` 表示宿主还没有跑完第一轮。 */
    captureStatus?(): Promise<ClipboardCaptureStatus | null>;
  };
  loadCatalog?(): Promise<{
    emoji: EmojiCatalogGroup[];
    kaomoji: EmojiCatalogGroup[];
    symbols: EmojiCatalogGroup[];
    /** 只标记内置目录不可用；成功响应中的插件组仍是当前完整快照。 */
    unavailable?: ("emoji" | "kaomoji" | "symbols")[];
  }>;
}
