export interface ShuangpinOption {
  profile: string;
  title: string;
  touch_scheme: string;
}
interface PreferencesSnapshot {
  format_version: number;
  revision: number;
  preferences: Object;
}
interface PreferencesReply {
  ok: boolean;
  value?: PreferencesSnapshot;
  error?: string;
}
interface SettingsValue {
  options: ShuangpinOption[];
  profile: string;
  preferences: Object;
}
interface SettingsReply {
  ok: boolean;
  value?: SettingsValue;
  error?: string;
}
interface SettingsRequest {
  preferences: Object;
  edition: string;
  profile?: string;
}
export interface ShuangpinSettingsBridge {
  loadPreferences(): string;
  savePreferences(revision: number, document: string): string;
  plan(request: string): string;
}

/** 每次选择都从最新快照计算；冲突后只回读，不拿旧选项覆盖并发修改。 */
export class TouchShuangpinSettings {
  options: ShuangpinOption[] = [];
  profile: string = "";
  message: string = "";
  ready: boolean = false;

  constructor(
    private bridge: ShuangpinSettingsBridge,
    private edition: string,
  ) {}

  private snapshot(): PreferencesSnapshot {
    const reply: PreferencesReply = JSON.parse(this.bridge.loadPreferences()) as PreferencesReply;
    if (!reply.ok || reply.value === undefined) throw new Error("load");
    return reply.value;
  }

  private plan(snapshot: PreferencesSnapshot, profile?: string): SettingsValue {
    const request: SettingsRequest = { preferences: snapshot.preferences, edition: this.edition };
    if (profile !== undefined) request.profile = profile;
    const reply: SettingsReply = JSON.parse(
      this.bridge.plan(JSON.stringify(request)),
    ) as SettingsReply;
    if (!reply.ok || reply.value === undefined || !Array.isArray(reply.value.options))
      throw new Error("plan");
    return reply.value;
  }

  load(): void {
    this.ready = false;
    this.message = "";
    try {
      const value: SettingsValue = this.plan(this.snapshot());
      this.options = value.options;
      this.profile = value.profile;
      this.ready = true;
      if (this.options.length === 0) this.message = "当前版本不提供双拼";
    } catch {
      this.options = [];
      this.profile = "";
      this.message = "读取双拼设置失败，请重试";
    }
  }

  select(profile: string): void {
    if (
      !this.ready ||
      !this.options.some((option: ShuangpinOption): boolean => option.profile === profile)
    )
      return;
    let notice: string = "";
    try {
      const snapshot: PreferencesSnapshot = this.snapshot();
      snapshot.preferences = this.plan(snapshot, profile).preferences;
      const saved: PreferencesReply = JSON.parse(
        this.bridge.savePreferences(snapshot.revision, JSON.stringify(snapshot)),
      ) as PreferencesReply;
      if (!saved.ok || saved.value === undefined) {
        notice =
          saved.error === "conflict" ? "设置已在其他位置更新，请重新选择" : "保存失败，请重试";
      }
    } catch {
      notice = "保存失败，请重试";
    }
    this.load();
    if (notice.length > 0 && this.ready) this.message = notice;
  }
}
