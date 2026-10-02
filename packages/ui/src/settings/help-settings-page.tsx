import * as doc from "./document-style";
import * as settings from "./settings-style";
import { ActionButton } from "../core/action-button";
import { GroupList } from "../core/platform-controls";
import { ActionRow } from "./action-row";

/**
 * The macOS help page. The reference window answers the three questions a new user actually has --
 * how do I type, why is there English next to the candidates, and why is it not in my input menu --
 * as term/description rows rather than prose, so each answer is findable without reading the page.
 */
const macosHelpCards = [
  {
    title: "开始输入",
    rows: [
      {
        term: "Shift",
        text: "在中文和英文之间切换。切换时光标下方会短暂显示「中」或「英」，可以在「输入 › 中英文」里关掉。",
      },
      { term: "数字键 1–9", text: "选中候选栏里对应位置的词，空格上屏第一个。" },
      {
        term: "翻页",
        text: "默认是减号和等号（- / =）。在「输入 › 选词与翻页」的翻页方式里可以换成逗号句号（, / .）或方括号（[ / ]）。",
      },
      { term: "Option+Shift+H", text: "切换全角与半角。" },
    ],
  },
  {
    title: "候选词释义",
    rows: [
      {
        term: "离线优先",
        text: "常见词直接用本机词典，不联网、没有延迟。选了在线服务后，生僻字和多字词的释义可能要等半秒左右才出现。",
      },
      {
        term: "在线释义",
        text: "默认不联网。只有在「翻译服务」里选了腾讯云、小牛翻译、自定义服务或「水杉账号」后，才会把当前页的中文候选词发给所选服务；选「水杉账号」会发到 api.msime.app，首次使用时创建一个匿名账号。",
      },
      { term: "两种语言", text: "可以同时显示两种语言的释义，在标点与翻译页的候选词翻译里设置。" },
      {
        term: "Tab",
        text: "在候选词和它的释义之间切换要上屏的那一列，Shift+Tab 反向。切到哪一列，那一列就会加下划线，数字键、空格和点击上屏的都是它。",
      },
      {
        term: "Option / Control + 数字",
        text: "不切换，直接上屏那一格的释义：Option 是目标语言，Control 是第二语言。",
      },
    ],
  },
  {
    title: "遇到问题",
    rows: [
      {
        term: "输入菜单里没有",
        text: "到「系统设置 › 键盘 › 文字输入 › 输入法」里添加水杉输入法。刚安装或刚更新过时，可能需要在输入菜单里切走再切回来。",
      },
      {
        term: "候选旁没有释义",
        text: "先确认标点与翻译页的候选词翻译是开着的。词典没收录的词要联网查询，断网时只会显示词典里有的那些。",
      },
      { term: "词库没有更新", text: "词库更新随版本发布。在「关于」页检查更新。" },
    ],
  },
] as const;

export interface HelpSettingsPageProps {
  busy: boolean;
  hidden: boolean;
  macos: boolean;
  mobile: boolean;
  ios: boolean;
  android: boolean;
  platformHelpIntro: string;
  platformQuickStart: string;
  platformNetworkDescription: string;
  onOpenDocumentation?: () => void;
  onOpenSystemKeyboardSettings?: () => void;
}

/** 「帮助」页，桌面和移动设置宿主共用；从「帮助与反馈」上的一行进入。 */
export function HelpSettingsPage({
  busy,
  hidden,
  macos,
  mobile,
  ios,
  android,
  platformHelpIntro,
  platformQuickStart,
  platformNetworkDescription,
  onOpenDocumentation,
  onOpenSystemKeyboardSettings,
}: HelpSettingsPageProps) {
  return (
    <fieldset disabled={busy} hidden={hidden} aria-label="帮助">
      <div className={settings.groups}>
        {macos &&
          macosHelpCards.map((card) => (
            <GroupList key={card.title} title={card.title}>
              <div
                className={`${settings.groupBlock} ${doc.guide}`}
                role="group"
                aria-label={card.title}
              >
                {card.rows.map((row, index) => (
                  <div
                    key={row.term}
                    className={`${doc.guideRow}${index === 0 && card.rows.length > 1 ? ` ${doc.guideLead}` : ""}`}
                  >
                    <span className={doc.guideTerm}>{row.term}</span>
                    <p className={doc.guideText}>{row.text}</p>
                  </div>
                ))}
              </div>
            </GroupList>
          ))}
        {macos && onOpenDocumentation && (
          <GroupList>
            <div className={settings.rowStack} role="group" aria-label="更多">
              <ActionRow
                title="更多"
                description="更完整的说明、词库来源和更新记录在官网上。"
                action={onOpenDocumentation}
                label="打开官网"
              />
            </div>
          </GroupList>
        )}
        {!macos && (
          <>
            <p className={settings.groupNote}>{platformHelpIntro}</p>
            <GroupList title="快速上手">
              <div className={`${settings.groupBlock} ${doc.page}`}>
                <p>{platformQuickStart}</p>
                {mobile && onOpenSystemKeyboardSettings && (
                  <ActionButton
                    action={onOpenSystemKeyboardSettings}
                    ariaBusy={busy}
                    disabled={busy}
                    label={ios ? "打开系统键盘设置" : "打开系统输入法设置"}
                  />
                )}
              </div>
            </GroupList>
            {ios && (
              <GroupList title="允许完全访问">
                <p className={settings.groupNote}>
                  打字统计保存本机字数、手写首次下载识别模型时需要在系统键盘设置中开启“允许完全访问”。不开启也可以正常打字；键盘默认离线，不会因为未开启而上传输入内容。
                </p>
              </GroupList>
            )}
            {android && (
              <GroupList title="输入权限">
                <p className={settings.groupNote}>
                  Android
                  的输入法服务只在当前编辑器请求时接收文本。云功能、语音和社区按你主动启用的功能联网，日常拼音输入无需联网。
                </p>
              </GroupList>
            )}
            <GroupList title="基本功能">
              <div className={`${settings.groupBlock} ${doc.page}`}>
                <p>
                  支持全拼、双拼和五笔，在「输入」页切换。全拼和双拼均支持辅助码，辅助码方案目前支持自然码辅助码、蓝天小雨点、首右
                  2.0、首右 plus 和小鹤。
                </p>
                <p>{platformNetworkDescription}</p>
                <p>更多功能欢迎自由探索～</p>
              </div>
            </GroupList>
            {onOpenDocumentation && (
              <GroupList title="更多">
                <ActionRow title="完整文档" action={onOpenDocumentation} label="完整文档（网页）" />
              </GroupList>
            )}
          </>
        )}
      </div>
    </fieldset>
  );
}
