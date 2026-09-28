import * as doc from "./document-style";

const macosHelpCards = [
  {
    title: "开始输入",
    rows: [
      {
        term: "Shift",
        text: "在中文和英文之间切换。切换时光标下方会短暂显示「中」或「英」，可以在快捷键里关掉。",
      },
      { term: "数字键 1–9", text: "选中候选栏里对应位置的词，空格上屏第一个。" },
      {
        term: "翻页",
        text: "默认是减号和等号（- / =）。在快捷键页可以换成逗号句号（, / .）或方括号（[ / ]）。",
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
      { term: "两种语言", text: "可以同时显示两种语言的释义，在输入页的候选词翻译里设置。" },
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
        text: "先确认输入页的候选词翻译是开着的。词典没收录的词要联网查询，断网时只会显示词典里有的那些。",
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

/** Shared help page content for desktop and mobile settings hosts. */
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
      {macos &&
        macosHelpCards.map((card) => (
          <div
            key={card.title}
            className={`section ${doc.guide}`}
            role="group"
            aria-label={card.title}
          >
            <div className="section-title">{card.title}</div>
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
        ))}
      {macos && onOpenDocumentation && (
        <div className="section" role="group" aria-label="更多">
          <div className="section-header">
            <span className="section-title">
              更多<small>更完整的说明、词库来源和更新记录在官网上。</small>
            </span>
            <button type="button" className="secondary" onClick={onOpenDocumentation}>
              打开官网
            </button>
          </div>
        </div>
      )}
      {!macos && (
        <div className={`section ${doc.page}`}>
          <p>{platformHelpIntro}</p>
          <div className={doc.subsection}>
            <div className="section-title">快速上手</div>
            <p>{platformQuickStart}</p>
            {mobile && onOpenSystemKeyboardSettings && (
              <button type="button" className="secondary" onClick={onOpenSystemKeyboardSettings}>
                {ios ? "打开系统键盘设置" : "打开系统输入法设置"}
              </button>
            )}
          </div>
          {ios && (
            <div className={doc.subsection}>
              <div className="section-title">允许完全访问</div>
              <p>
                打字统计保存本机字数、手写首次下载识别模型时需要在系统键盘设置中开启“允许完全访问”。不开启也可以正常打字；键盘默认离线，不会因为未开启而上传输入内容。
              </p>
            </div>
          )}
          {android && (
            <div className={doc.subsection}>
              <div className="section-title">输入权限</div>
              <p>
                Android
                的输入法服务只在当前编辑器请求时接收文本。云功能、语音和社区按你主动启用的功能联网，日常拼音输入无需联网。
              </p>
            </div>
          )}
          <div className={doc.subsection}>
            <div className="section-title">基本功能</div>
            <p>
              支持全拼、双拼和五笔。可以在设置窗口下的输入功能分区进行切换。全拼和双拼均支持辅助码，辅助码方案目前支持自然码辅助码、蓝天小雨点、首右
              2.0、首右 plus 和小鹤。
            </p>
            <p>{platformNetworkDescription}</p>
            <p>更多功能欢迎自由探索～</p>
          </div>
          {onOpenDocumentation && (
            <div className={doc.subsection}>
              <div className="section-title">更多</div>
              <button type="button" className="secondary" onClick={onOpenDocumentation}>
                完整文档（网页）
              </button>
            </div>
          )}
        </div>
      )}
    </fieldset>
  );
}
