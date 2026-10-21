import { SettingsGroupNote } from "./settings-group-note";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsRowStack } from "./settings-row-stack";
import * as doc from "./document-style";
import { ActionButton } from "../core/action-button";
import { GroupList } from "../core/platform-controls";
import { ActionRow } from "./action-row";
import { SettingsPageFieldset } from "./settings-page-fieldset";
import type { EditionInfo } from "../core/host-capabilities";
import { linuxEditionNames } from "../account/linux-edition-names";
import { isChineseScheme } from "./input-scheme-options";

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

interface HelpCard {
  title: string;
  rows: readonly { term: string; text: string }[];
}

/**
 * Linux 的帮助页沿用 macOS 的术语/答案卡片版式，键名和排查步骤换成 Linux 上实际生效的：简繁和中英文标点的组合键两个宿主都处理，输入法列表分 Fcitx5 和 IBus 两条路，重启快捷键在 IBus 下重启服务、在 Fcitx5 下只重置水杉插件。设置前端在各 Linux 版本之间共用，输入法条目名和命令按本版本取（`linuxEditionNames`）；简繁和中文标点两行只给提供中文方案的版本。
 */
function linuxHelpCards(edition: EditionInfo | undefined): readonly HelpCard[] {
  const { name, fcitx5Label, ibusLabel, setupCommand } = linuxEditionNames(edition);
  const chinese = !edition || edition.input_schemes.some(isChineseScheme);
  return [
    {
      title: "开始输入",
      rows: [
        {
          term: "Shift",
          text: "在中文和英文之间切换。切换时会短暂显示「中」或「英」，可以在「输入 › 中英文」里关掉。",
        },
        { term: "数字键 1–9", text: "选中候选栏里对应位置的词，空格上屏第一个。" },
        {
          term: "翻页",
          text: "默认是减号和等号（- / =）。在「输入 › 选词与翻页」的翻页方式里可以换成逗号句号（, / .）或方括号（[ / ]）。",
        },
        ...(chinese
          ? [
              { term: "Ctrl+Shift+F", text: "在简体和繁体输出之间切换。" },
              { term: "Ctrl+.", text: "切换中文标点与英文标点。" },
            ]
          : []),
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
          text: "默认不联网。只有在「翻译服务」里选了腾讯云、小牛翻译、自定义服务或「水杉账号」后，才会把当前页的中文候选词发给所选服务；选「水杉账号」会发到 api.msime.app。请求由用户级的 msime-linux-online-provider 服务发出，输入法本身不联网。",
        },
      ],
    },
    {
      title: "遇到问题",
      rows: [
        {
          term: "输入法列表里没有",
          text: `使用 Fcitx5 时，用 fcitx5-configtool 把${fcitx5Label}加入当前输入法组；使用 IBus 时，执行 ibus restart 后在系统设置的输入源中添加${ibusLabel}。也可以在终端运行 ${setupCommand} --register 自动加回去。`,
        },
        {
          term: "输入没有反应",
          text: "按 Ctrl+Shift+Alt+R 重启：IBus 下重启 IBus 服务，Fcitx5 下只重置水杉插件，不影响其他输入法。也可以在「维护与诊断」里点「重启」。",
        },
        {
          term: "状态栏没有入口",
          text: `中英文、简繁、工具栏等入口挂在输入法的状态区，需要桌面提供托盘；没有托盘时输入本身不受影响，设置可以从应用列表里的「${name}」打开。`,
        },
        {
          term: "词库没有更新",
          text: `随安装包提供的词库由包管理器随版本升级一起更新。首次配置时自己下载的词库不随软件包更新：升级后会收到「词库需要更新」的通知，打开设置点「立即更新」即可，也可以在终端运行 ${setupCommand} --update --download。`,
        },
      ],
    },
  ];
}

export interface HelpSettingsPageProps {
  busy: boolean;
  hidden: boolean;
  macos: boolean;
  /** Linux 用自己的术语卡片（`linuxHelpCards`），版式与 macOS 相同。 */
  linux?: boolean;
  /** 本安装包所属的版本（`HostCapabilities.edition`）；Linux 卡片里的输入法条目名和命令按它取。 */
  edition?: EditionInfo;
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
  linux = false,
  edition,
  mobile,
  ios,
  android,
  platformHelpIntro,
  platformQuickStart,
  platformNetworkDescription,
  onOpenDocumentation,
  onOpenSystemKeyboardSettings,
}: HelpSettingsPageProps) {
  const helpCards: readonly HelpCard[] | null = macos
    ? macosHelpCards
    : linux
      ? linuxHelpCards(edition)
      : null;
  return (
    <SettingsPageFieldset disabled={busy} hidden={hidden} ariaLabel="帮助">
      {helpCards &&
        helpCards.map((card) => (
          <GroupList key={card.title} title={card.title}>
            <SettingsGroupBlock className={doc.guide} role="group" aria-label={card.title}>
              {card.rows.map((row, index) => (
                <div
                  key={row.term}
                  className={`${doc.guideRow}${index === 0 && card.rows.length > 1 ? ` ${doc.guideLead}` : ""}`}
                >
                  <span className={doc.guideTerm}>{row.term}</span>
                  <p className={doc.guideText}>{row.text}</p>
                </div>
              ))}
            </SettingsGroupBlock>
          </GroupList>
        ))}
      {linux && !macos && (
        // macOS 的卡片只讲候选释义的联网；Linux 还有云候选、语音和 AI 的用户级服务与本机凭据文件，这段说明照旧保留。
        <GroupList title="联网与凭据">
          <SettingsGroupBlock className={doc.page}>
            <p>{platformNetworkDescription}</p>
          </SettingsGroupBlock>
        </GroupList>
      )}
      {helpCards && onOpenDocumentation && (
        <GroupList>
          <SettingsRowStack role="group" aria-label="更多">
            <ActionRow
              title="更多"
              description="更完整的说明、词库来源和更新记录在官网上。"
              action={onOpenDocumentation}
              label="打开官网"
            />
          </SettingsRowStack>
        </GroupList>
      )}
      {!helpCards && (
        <>
          <SettingsGroupNote>{platformHelpIntro}</SettingsGroupNote>
          <GroupList title="快速上手">
            <SettingsGroupBlock className={doc.page}>
              <p>{platformQuickStart}</p>
              {mobile && onOpenSystemKeyboardSettings && (
                <ActionButton
                  action={onOpenSystemKeyboardSettings}
                  ariaBusy={busy}
                  disabled={busy}
                  label={ios ? "打开系统键盘设置" : "打开系统输入法设置"}
                />
              )}
            </SettingsGroupBlock>
          </GroupList>
          {ios && (
            <GroupList title="允许完全访问">
              <SettingsGroupNote>
                打字统计保存本机字数、手写首次下载识别模型时需要在系统键盘设置中开启“允许完全访问”。不开启也可以正常打字；键盘默认离线，不会因为未开启而上传输入内容。
              </SettingsGroupNote>
            </GroupList>
          )}
          {android && (
            <GroupList title="输入权限">
              <SettingsGroupNote>
                Android
                的输入法服务只在当前编辑器请求时接收文本。云功能、语音和社区按你主动启用的功能联网，日常拼音输入无需联网。
              </SettingsGroupNote>
            </GroupList>
          )}
          <GroupList title="基本功能">
            <SettingsGroupBlock className={doc.page}>
              <p>
                支持全拼、双拼和五笔，在「输入」页切换。全拼和双拼均支持辅助码，辅助码方案目前支持自然码辅助码、蓝天小雨点、首右
                2.0、首右 plus、小鹤、拼音加加和五笔 86。
              </p>
              <p>{platformNetworkDescription}</p>
              <p>更多功能欢迎自由探索～</p>
            </SettingsGroupBlock>
          </GroupList>
          {onOpenDocumentation && (
            <GroupList title="更多">
              <ActionRow title="完整文档" action={onOpenDocumentation} label="完整文档（网页）" />
            </GroupList>
          )}
        </>
      )}
    </SettingsPageFieldset>
  );
}
