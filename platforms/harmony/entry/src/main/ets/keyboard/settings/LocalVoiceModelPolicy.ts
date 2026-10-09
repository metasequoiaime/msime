/**
 * The settings page's on-device model manager, as this host answers it.
 *
 * The page expects what the desktop host hands it: `{models, default, root}` for a list, and a refusal carrying one of the stable `local_model_*` codes it has a sentence for. The C ABI answers with `{models, default}` and puts the core error's `Display` text in its single error string, `local_model_network: <detail>` and the like, so this adds the root, maps the text back to the desktop's codes (`local_model_error_code` in the Tauri host) and drops the detail, which may carry a URL the page has no use for.
 *
 * Models the catalog marks desktop-only are not offered on this host. One that is already installed stays listed, so it can still be removed.
 */

/** One catalog row, reduced to the fields this reads; everything else passes through untouched. */
export interface LocalVoiceModelRow {
  id?: string;
  desktop_only?: boolean;
  installed?: boolean;
  import_files?: LocalVoiceModelImportFile[];
}

/** 「从文件导入」需要的一个文件，只读它的大小。 */
export interface LocalVoiceModelImportFile {
  size?: number;
}

interface LocalVoiceModelsValue {
  models?: LocalVoiceModelRow[];
  default?: string;
  root?: string;
}

interface LocalVoiceModelsReply {
  ok: boolean;
  value?: LocalVoiceModelsValue;
  error?: string;
}

interface InstallValue {
  path?: string;
}

interface InstallReply {
  ok: boolean;
  value?: InstallValue;
  error?: string;
}

interface InstallPathReply {
  ok: boolean;
  value: string;
  error: string;
}

interface VoiceInputMirror {
  asr_model_mirror?: string;
}

interface PreferencesDocument {
  voice_input?: VoiceInputMirror;
}

interface PreferencesSnapshot {
  preferences?: PreferencesDocument;
}

interface PreferencesReply {
  ok: boolean;
  value?: PreferencesSnapshot;
}

/** What the page sends: one operation on one catalog id; `list` needs no id. */
export interface LocalVoiceModelAction {
  operation?: string;
  id?: string;
}

export class LocalVoiceModelPolicy {
  /** Where models live, beside the keyboard's state and engine directories; the keyboard reads the same path back from `asr_model_path`. */
  static root(filesDir: string): string {
    return `${filesDir}/voice-models`;
  }

  /** The page's code for a refusal. Unrecognised text becomes `local_model_failed`, which the page answers with its general sentence. */
  static errorCode(error: string): string {
    if (typeof error !== "string" || error.length === 0) {
      return "local_model_failed";
    }
    const separator: number = error.indexOf(":");
    const head: string = separator >= 0 ? error.substring(0, separator) : error;
    switch (head) {
      case "local_model_unknown":
      case "local_model_invalid_root":
      case "local_model_invalid_mirror":
      case "local_model_cancelled":
      case "local_model_network":
      case "local_model_http_status":
      case "local_model_checksum_mismatch":
      case "local_model_import_missing":
      case "local_model_import_unreadable":
      case "local_model_io":
        return head;
      case "local_model_size_mismatch":
        return "local_model_checksum_mismatch";
      case "local_model_unsafe_archive":
      case "local_model_missing_file":
        return "local_model_invalid_archive";
      case "local_model_install_running":
        return "busy";
      case "invalid local model root":
        return "local_model_invalid_root";
      default:
        return "local_model_failed";
    }
  }

  /** The same reply with its error replaced by a code; an accepted reply is left alone, and one this cannot read becomes a general refusal. */
  static rewrite(reply: string): string {
    let parsed: LocalVoiceModelsReply;
    try {
      parsed = JSON.parse(reply) as LocalVoiceModelsReply;
    } catch {
      return LocalVoiceModelPolicy.refusal("local_model_failed");
    }
    if (parsed === null || typeof parsed !== "object") {
      return LocalVoiceModelPolicy.refusal("local_model_failed");
    }
    if (parsed.ok === true) {
      return reply;
    }
    return LocalVoiceModelPolicy.refusal(LocalVoiceModelPolicy.errorCode(parsed.error ?? ""));
  }

  /** The list reply the page reads: the root added, desktop-only models that are not installed left out, a refusal mapped to its code. */
  static listReply(reply: string, root: string): string {
    let parsed: LocalVoiceModelsReply;
    try {
      parsed = JSON.parse(reply) as LocalVoiceModelsReply;
    } catch {
      return LocalVoiceModelPolicy.refusal("local_model_failed");
    }
    if (parsed === null || typeof parsed !== "object" || parsed.ok !== true) {
      return LocalVoiceModelPolicy.rewrite(reply);
    }
    const value: LocalVoiceModelsValue = parsed.value ?? {};
    const models: LocalVoiceModelRow[] = Array.isArray(value.models) ? value.models : [];
    const list: LocalVoiceModelsValue = {
      models: models.filter((model: LocalVoiceModelRow): boolean =>
        LocalVoiceModelPolicy.offered(model),
      ),
      default: value.default ?? "",
      root: root,
    };
    return JSON.stringify({ ok: true, value: list, error: "" } as LocalVoiceModelsReply);
  }

  /** The install reply the page reads: the installed directory as the value, rather than the C ABI's `{path}` record. */
  static installReply(reply: string): string {
    let parsed: InstallReply;
    try {
      parsed = JSON.parse(reply) as InstallReply;
    } catch {
      return LocalVoiceModelPolicy.refusal("local_model_failed");
    }
    if (parsed === null || typeof parsed !== "object" || parsed.ok !== true) {
      return LocalVoiceModelPolicy.rewrite(reply);
    }
    const path: string | undefined = parsed.value?.path;
    if (typeof path !== "string" || path.length === 0) {
      return LocalVoiceModelPolicy.refusal("local_model_failed");
    }
    return JSON.stringify({ ok: true, value: path, error: "" } as InstallPathReply);
  }

  /**
   * 「从文件导入」时值得复制进缓存的文件大小：目录里这个模型要的每个文件的长度。选择器给的是文档 URI，原生库打不开，要先复制一份；长度对不上的文件反正认不出来，不复制，免得用户多选了一个大文件时白占一份空间。列表读不出来或没有这个模型时为空。
   */
  static importSizes(listReply: string, id: string): number[] {
    let parsed: LocalVoiceModelsReply;
    try {
      parsed = JSON.parse(listReply) as LocalVoiceModelsReply;
    } catch {
      return [];
    }
    if (parsed === null || typeof parsed !== "object" || parsed.ok !== true) return [];
    const models: LocalVoiceModelRow[] = Array.isArray(parsed.value?.models)
      ? (parsed.value?.models ?? [])
      : [];
    const model: LocalVoiceModelRow | undefined = models.find(
      (row: LocalVoiceModelRow): boolean => row.id === id,
    );
    const files: LocalVoiceModelImportFile[] = Array.isArray(model?.import_files)
      ? (model?.import_files ?? [])
      : [];
    return files
      .map((file: LocalVoiceModelImportFile): number => file.size ?? 0)
      .filter((size: number): boolean => Number.isSafeInteger(size) && size > 0);
  }

  /** Whether this host offers a model: anything not desktop-only, and a desktop-only one only once installed. */
  static offered(model: LocalVoiceModelRow): boolean {
    return model.desktop_only !== true || model.installed === true;
  }

  /** The saved download mirror, trimmed, or `""` when the preferences cannot be read; client-core validates it. */
  static mirror(preferencesReply: string): string {
    try {
      const parsed: PreferencesReply = JSON.parse(preferencesReply) as PreferencesReply;
      const mirror: string | undefined = parsed?.value?.preferences?.voice_input?.asr_model_mirror;
      return typeof mirror === "string" ? mirror.trim() : "";
    } catch {
      return "";
    }
  }

  /** The action the page sent, or `null` when it is not one this answers. */
  static action(document: string): LocalVoiceModelAction | null {
    let action: LocalVoiceModelAction;
    try {
      action = JSON.parse(document) as LocalVoiceModelAction;
    } catch {
      return null;
    }
    if (action === null || typeof action !== "object") return null;
    if (action.operation === "list") return action;
    if (
      action.operation !== "install" &&
      action.operation !== "import" &&
      action.operation !== "cancel" &&
      action.operation !== "remove"
    ) {
      return null;
    }
    if (typeof action.id !== "string" || action.id.length === 0 || action.id.length > 256) {
      return null;
    }
    return action;
  }

  static refusal(code: string): string {
    return JSON.stringify({ ok: false, error: code } as LocalVoiceModelsReply);
  }
}
