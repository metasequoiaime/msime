import { useState } from "react";
import { ActionButton } from "../core/action-button";
import { MascotIllustration } from "../core/mascot";
import type { LinuxEditionNames } from "./linux-edition-names";

/** `msime-linux-setup` 报告的输入法列表情况：正在运行的输入法框架（都没在运行时为 `null`）以及水杉输入法是否已在它的列表里。 */
export interface LinuxSetupRegistration {
  host: "fcitx5" | "ibus" | null;
  registered: boolean;
}

export interface LinuxInputMethodNoticeProps {
  /** 没有结果时（旧版脚本、或配置在报告之前就结束了）按两种框架都给出手动加入的步骤。 */
  registration: LinuxSetupRegistration | null | undefined;
  /** 本版本的名字和首次配置命令（`linuxEditionNames`）。 */
  names: LinuxEditionNames;
  /** 配置脚本报告的、要用户留意的提示，配置成功时也列在卡片上。 */
  notes?: readonly string[];
  openFcitx5Config?: () => Promise<void>;
  copyText?: (text: string) => Promise<void>;
}

function Steps({ steps }: { steps: readonly string[] }) {
  return (
    <ol className="m-0 flex list-decimal flex-col gap-1 pl-5 text-left text-[13px] leading-relaxed text-[var(--p-sub)]">
      {steps.map((step) => (
        <li key={step}>{step}</li>
      ))}
    </ol>
  );
}

/**
 * 首次配置完成后的「加入输入法列表」状态卡，对应 macOS 设置页的 `InputSourceStartupNotice`：结果写成标题、说明和步骤，而不是让用户自己去读脚本输出的最后几行。已经加入时配水杉娘插图；没能自动加入时给出分步说明和对应的按钮（Fcitx5 打开 fcitx5-configtool，IBus 复制 `ibus restart`）。
 */
export function LinuxInputMethodNotice({
  registration,
  names,
  notes = [],
  openFcitx5Config,
  copyText,
}: LinuxInputMethodNoticeProps) {
  const [message, setMessage] = useState("");
  const { name, fcitx5Label: fcitx5Name, ibusLabel: ibusName, setupCommand } = names;
  const host = registration?.host ?? null;
  const registered = registration?.registered === true && host !== null;

  let title: string;
  let detail: string | null = null;
  let steps: readonly string[] = [];
  if (registered && host === "fcitx5") {
    title = "已加入 Fcitx5 输入法组";
    detail = `用 Fcitx5 的输入法切换快捷键（默认是 Ctrl+空格）切换到${name}即可开始输入。`;
  } else if (registered) {
    title = "已加入输入源列表";
    detail = `在桌面的输入源里切换到${ibusName}即可开始输入（GNOME 默认是 Super+空格）。`;
  } else if (host === "fcitx5") {
    title = `把${name}加入 Fcitx5 输入法组`;
    detail = "没能自动加入，请手动添加：";
    steps = [
      "点「打开 Fcitx5 配置」。",
      `在右侧「可用输入法」里找到${fcitx5Name}，把它移到左侧的当前输入法组。`,
      `点「应用」，再用 Ctrl+空格 切换到${name}。`,
    ];
  } else if (host === "ibus") {
    title = `把${name}加入输入源`;
    detail = "没能自动加入，请手动添加：";
    steps = [
      "在终端运行 ibus restart，让 IBus 读到新装的输入法。",
      `在系统设置的键盘或输入源里添加${ibusName}。`,
      "用桌面的输入源切换快捷键切换到它。",
    ];
  } else if (registration) {
    title = "启动 Fcitx5 或 IBus 后加入输入法列表";
    detail = `没有检测到正在运行的 Fcitx5 或 IBus，输入法列表没有改动。启动其中一个后，在终端运行 ${setupCommand} --register 自动加入，或按下面的方法手动添加：`;
    steps = [
      `Fcitx5：用 fcitx5-configtool 把${fcitx5Name}加入当前输入法组。`,
      `IBus：执行 ibus restart，再在系统设置的输入源里添加${ibusName}。`,
    ];
  } else {
    title = "加入输入法列表";
    detail = `${name}还不在输入法列表里时，按正在使用的输入法框架手动添加：`;
    steps = [
      `Fcitx5：用 fcitx5-configtool 把${fcitx5Name}加入当前输入法组。`,
      `IBus：执行 ibus restart，再在系统设置的输入源里添加${ibusName}。`,
    ];
  }

  const showConfigtool = !registered && host === "fcitx5" && openFcitx5Config;
  const showCopy = !registered && host === "ibus" && copyText;

  return (
    <div
      className="flex w-full flex-col items-center gap-2 rounded-[12px] border border-[var(--p-hair)] bg-[var(--p-group-bg)] px-5 py-4 text-center"
      role="status"
      aria-label="输入法列表"
    >
      {registered && <MascotIllustration scene="updated" size={72} />}
      <p className="m-0 text-[15px] font-semibold text-[var(--p-text)]">{title}</p>
      {detail && <p className="m-0 text-[13px] leading-relaxed text-[var(--p-sub)]">{detail}</p>}
      {steps.length > 0 && <Steps steps={steps} />}
      {notes.length > 0 && (
        <ul
          className="m-0 mt-1 flex list-none flex-col gap-1.5 p-0 text-left text-[13px] leading-relaxed text-[var(--p-sub)] select-text"
          aria-label="配置提示"
        >
          {notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      )}
      {(showConfigtool || showCopy) && (
        <div className="mt-1 flex flex-wrap justify-center gap-2">
          {showConfigtool && (
            <ActionButton
              action={() =>
                void openFcitx5Config().catch(() =>
                  setMessage("无法打开 Fcitx5 配置，请在应用列表里打开「Fcitx 5 配置」。"),
                )
              }
              className={secondaryButton}
              label="打开 Fcitx5 配置"
            />
          )}
          {showCopy && (
            <ActionButton
              action={() =>
                void copyText("ibus restart")
                  .then(() => setMessage("已复制 ibus restart"))
                  .catch(() => setMessage("无法复制，请手动输入 ibus restart。"))
              }
              className={secondaryButton}
              label="复制 ibus restart"
            />
          )}
        </div>
      )}
      {message && <p className="m-0 text-xs text-[var(--p-sub)]">{message}</p>}
    </div>
  );
}

const secondaryButton =
  "h-8 rounded-[var(--p-r-ctl)] border-0 bg-[var(--p-hover)] px-3 text-[13px] text-[var(--p-text)] hover:bg-[var(--p-press)]";
