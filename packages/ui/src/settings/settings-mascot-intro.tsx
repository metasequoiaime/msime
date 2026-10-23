import type { ReactNode } from "react";
import { MascotIllustration, type MascotScene } from "../core/mascot";
import type { SettingsPageId } from "./settings-page-registry";
import type { SettingsIntroTabs } from "./use-settings-intro-tabs";

interface SettingsMascotIntro {
  scene: MascotScene;
  description: string;
}

const introductions: Partial<Record<SettingsPageId, SettingsMascotIntro>> = {
  input: { scene: "input", description: "选择输入方案，调整选词与输出习惯。" },
  dictionary: { scene: "downloading", description: "管理词库、词条与常用表达。" },
  shortcuts: { scene: "input", description: "查看按键速查，设置常用快捷键。" },
  expression: { scene: "input", description: "设置标点、候选释义与翻译服务。" },
  handwriting: { scene: "notes", description: "设置手写识别与输入方式。" },
  skin: {
    scene: "welcome",
    description: "选择皮肤与配色；候选窗口的布局、字体和显示方式在「候选窗口」中设置。",
  },
  appearance: {
    scene: "welcome",
    description: "调整候选窗口的布局、字体和显示方式；皮肤与配色在「主题」中设置。",
  },
  "floating-toolbar": { scene: "settings", description: "设置工具栏的显示与常用功能。" },
  "typing-statistics": { scene: "notes", description: "查看输入统计与使用习惯。" },
  plugins: { scene: "settings", description: "管理插件与功能扩展。" },
  ai: { scene: "notes", description: "配置候选辅助与文本处理服务。" },
  account: { scene: "sync", description: "管理账号与跨设备设置同步。" },
  community: { scene: "welcome", description: "发现主题、词库与社区作品。" },
  developer: { scene: "settings", description: "检查输入法状态与维护工具。" },
  mcp: { scene: "settings", description: "配置 AI 助手与输入法的连接。" },
  "usage-reporting": { scene: "privacy", description: "了解匿名使用统计发送哪些内容。" },
  privacy: { scene: "privacy", description: "查看数据处理方式与隐私设置。" },
};

const tabIntroductions: {
  [Page in keyof SettingsIntroTabs]: Record<SettingsIntroTabs[Page], SettingsMascotIntro>;
} = {
  voice: {
    input: { scene: "notes", description: "配置语音识别、服务与输入快捷键。" },
    recording: { scene: "settings", description: "调整录音提示音与录音期间的静音行为。" },
  },
  "screen-keyboard": {
    keyboard: { scene: "input", description: "调整屏幕键盘布局与外观。" },
    handwriting: { scene: "notes", description: "设置手写识别与输入方式。" },
  },
  tools: {
    clipboard: { scene: "sync", description: "设置剪贴板历史的保存与云端同步。" },
    history: { scene: "notes", description: "查看、复制和管理已保存的剪贴板记录。" },
  },
};

/** 桌面常用设置页的场景插画与简短引导；关于页已有品牌卡，对话等工具页保留完整操作空间。 */
export function settingsMascotIntro(page: SettingsPageId, tabs: SettingsIntroTabs) {
  if (page === "voice") return tabIntroductions.voice[tabs.voice];
  if (page === "screen-keyboard")
    return tabIntroductions["screen-keyboard"][tabs["screen-keyboard"]];
  if (page === "tools") return tabIntroductions.tools[tabs.tools];
  return introductions[page];
}

/** 内容区页首保留原标题与返回操作，插画只占自己的列，不遮挡表单或导航。 */
export function SettingsMascotIntro({
  scene,
  description,
  children,
}: SettingsMascotIntro & { children?: ReactNode }) {
  return (
    <div
      data-settings-mascot-intro=""
      className="mb-5 flex min-h-20 items-center justify-between gap-5 px-1 max-phone:mb-3 max-phone:min-h-14 max-phone:gap-3 [@media(max-height:700px)]:mb-3 [@media(max-height:700px)]:min-h-14 [@media(max-height:700px)]:gap-3 [&_header]:mb-1.5"
    >
      <div className="min-w-0 flex-1">
        {children}
        <p className="m-0 text-[13px] leading-relaxed text-secondary">{description}</p>
      </div>
      <MascotIllustration
        scene={scene}
        size={80}
        className="shrink-0 max-phone:size-14 [@media(max-height:700px)]:size-14"
      />
    </div>
  );
}
