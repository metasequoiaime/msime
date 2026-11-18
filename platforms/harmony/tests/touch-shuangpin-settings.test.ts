import {
  ShuangpinSettingsBridge,
  TouchShuangpinSettings,
} from "../entry/src/main/ets/keyboard/settings/TouchShuangpinSettings";
function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
class Bridge implements ShuangpinSettingsBridge {
  revision: number = 1;
  profile: string = "xiaohe";
  unrelated: number = 3;
  saves: number = 0;
  failure: string = "";
  loadFailure: boolean = false;
  planFailure: boolean = false;
  loadPreferences(): string {
    return this.loadFailure
      ? '{"ok":false}'
      : JSON.stringify({
          ok: true,
          value: {
            format_version: 1,
            revision: this.revision,
            preferences: { shuangpin_profile: this.profile, unrelated: this.unrelated },
          },
        });
  }
  plan(document: string): string {
    if (this.planFailure) return '{"ok":false}';
    const request = JSON.parse(document);
    if (request.profile !== undefined) request.preferences.shuangpin_profile = request.profile;
    return JSON.stringify({
      ok: true,
      value: {
        options: [
          { profile: "xiaohe", title: "小鹤双拼", touch_scheme: "xiaohe" },
          { profile: "sogou", title: "搜狗双拼", touch_scheme: "sogou" },
        ],
        profile: request.preferences.shuangpin_profile,
        preferences: request.preferences,
      },
    });
  }
  savePreferences(revision: number, document: string): string {
    this.saves++;
    check(revision === this.revision, "必须使用最新快照的修订号");
    const snapshot = JSON.parse(document);
    check(snapshot.preferences.unrelated === this.unrelated, "保留读取前发生的并发修改");
    if (this.failure.length > 0) {
      if (this.failure === "conflict") {
        this.revision++;
        this.profile = "xiaohe";
      }
      return JSON.stringify({ ok: false, error: this.failure });
    }
    this.profile = snapshot.preferences.shuangpin_profile;
    this.revision++;
    return this.loadPreferences();
  }
}
const bridge = new Bridge();
const model = new TouchShuangpinSettings(bridge, "full");
model.load();
check(model.ready && model.options.length === 2 && bridge.saves === 0, "打开只读，不启用新方案");
bridge.revision = 8;
bridge.unrelated = 11;
model.select("sogou");
check(
  model.profile === "sogou" && bridge.saves === 1 && model.message === "",
  "选择后展示实际保存的方案",
);
model.select("unknown");
check(bridge.saves === 1, "拒绝目录外的方案");
bridge.failure = "conflict";
model.select("sogou");
check(
  model.profile === "xiaohe" && bridge.saves === 2 && model.message.includes("重新选择"),
  "冲突只回读，不重试覆盖",
);
bridge.failure = "storage";
model.select("sogou");
check(model.profile === "xiaohe" && model.message.includes("保存失败"), "存储失败不乐观改为新方案");
bridge.loadFailure = true;
model.load();
model.select("sogou");
check(!model.ready && model.options.length === 0 && bridge.saves === 3, "读取失败禁止写入");
bridge.loadFailure = false;
bridge.planFailure = true;
model.load();
check(!model.ready && model.options.length === 0, "共享目录不可用时不自造默认规则");
console.log("touch shuangpin settings: passed");
