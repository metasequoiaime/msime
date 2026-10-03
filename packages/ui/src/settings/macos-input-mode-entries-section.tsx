import { useCallback, useEffect, useRef, useState } from "react";
import type { InputScheme } from "../index";
import { Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";

/** macOS 输入法列表的读取与系统设置入口，见 `SettingsClient.macosInputModes`。 */
export interface MacosInputModesClient {
  /** 用户已经加入输入法列表的本输入法模式（完整标识符）；读不到列表时为 `null`。 */
  enabled(): Promise<string[] | null>;
  /** 打开系统设置里添加输入法的那一页。 */
  openSettings(): Promise<void>;
}

const modePrefix = "app.msime.inputmethod.MetasequoiaIME.";

/** 一个菜单栏入口：输入法菜单里显示的名字、它属于哪个方案（`null` 表示不随方案出现），以及系统设置「添加」对话框把它归在哪个语言下。 */
interface ModeEntry {
  mode: string;
  name: string;
  scheme: InputScheme | null;
  language: string;
}

/** 顺序与输入法菜单一致，名字与 `platforms/macos/resources/*.lproj/InfoPlist.strings` 一致，语言对应 Info.plist.in 里各模式的 `TISIntendedLanguage`。 */
export const macosInputModeEntries: readonly ModeEntry[] = [
  { mode: "Hans", name: "水杉输入法 · 中", scheme: null, language: "简体中文" },
  { mode: "Shuangpin", name: "水杉输入法 · 双", scheme: "shuangpin", language: "简体中文" },
  { mode: "Wubi", name: "水杉输入法 · 五", scheme: "wubi", language: "简体中文" },
  { mode: "Cantonese", name: "水杉输入法 · 粤", scheme: "cantonese", language: "粤语" },
  { mode: "Zhuyin", name: "水杉输入法 · 注", scheme: "zhuyin", language: "繁体中文" },
  { mode: "Japanese", name: "水杉输入法 · 日", scheme: "japanese", language: "日语" },
  { mode: "Korean", name: "水杉输入法 · 韩", scheme: "korean", language: "韩语" },
  { mode: "Vietnamese", name: "水杉输入法 · 越", scheme: "vietnamese", language: "越南语" },
  { mode: "Tibetan", name: "水杉输入法 · 藏", scheme: "tibetan", language: "藏语" },
  { mode: "Roman", name: "水杉输入法 · 英", scheme: null, language: "简体中文" },
];

/** 点了「打开键盘设置」之后多久内反复读取列表：用户在系统设置里添加时，设置窗口不一定会重新获得焦点。 */
export const INPUT_MODE_RECHECK_MS = 3000;
export const INPUT_MODE_RECHECK_WINDOW_MS = 120_000;

/** 读取输入法列表：窗口重新获得焦点时读一次，点过「打开键盘设置」后的两分钟里每 3 秒读一次。 */
function useEnabledInputModes(client: MacosInputModesClient | undefined) {
  const [enabled, setEnabled] = useState<readonly string[] | null>(null);
  const [watchingUntil, setWatchingUntil] = useState(0);
  const request = useRef(0);
  const refresh = useCallback(async () => {
    if (!client) return;
    const current = ++request.current;
    const value = await client.enabled().catch(() => null);
    if (current === request.current) setEnabled(value);
  }, [client]);

  useEffect(() => {
    if (!client || typeof window === "undefined") {
      request.current++;
      setEnabled(null);
      return;
    }
    const onFocus = () => void refresh();
    onFocus();
    window.addEventListener("focus", onFocus);
    return () => {
      request.current++;
      window.removeEventListener("focus", onFocus);
    };
  }, [client, refresh]);

  useEffect(() => {
    if (!watchingUntil || typeof window === "undefined") return;
    const timer = window.setInterval(() => {
      if (Date.now() >= watchingUntil) {
        setWatchingUntil(0);
        return;
      }
      void refresh();
    }, INPUT_MODE_RECHECK_MS);
    return () => window.clearInterval(timer);
  }, [watchingUntil, refresh]);

  const watch = useCallback(() => setWatchingUntil(Date.now() + INPUT_MODE_RECHECK_WINDOW_MS), []);
  return { enabled, watch } as const;
}

export interface MacosInputModeEntriesSectionProps {
  client?: MacosInputModesClient;
  /** 当前（草稿里）的输入方案：它的入口还没加入时，这一行先说它。 */
  scheme: InputScheme;
  /** 宿主提供的方案；没提供的方案（例如没装词库的粤拼、注音）不列出它的入口。 */
  inputSchemes: readonly InputScheme[];
  onError: (message: string) => void;
}

/**
 * 「菜单栏入口」：列出还没加入输入法列表的模式，告诉用户在系统设置里去哪个语言下添加。
 *
 * macOS 27 不允许进程启用键盘输入模式（`TISEnableInputSource` 返回 noErr 而状态不变），输入法和设置应用都没法替用户加；而系统设置的「添加」对话框按语言分组，粤在「粤语」、注在「繁体中文」，只看「简体中文」会以为它们不存在。
 */
export function MacosInputModeEntriesSection({
  client,
  scheme,
  inputSchemes,
  onError,
}: MacosInputModeEntriesSectionProps) {
  const { enabled, watch } = useEnabledInputModes(client);
  if (!client || !enabled) return null;
  const offered = macosInputModeEntries.filter(
    (entry) => entry.scheme === null || inputSchemes.includes(entry.scheme),
  );
  const missing = offered.filter((entry) => !enabled.includes(modePrefix + entry.mode));
  const current = missing.find((entry) => entry.scheme === scheme);

  let description: string;
  if (missing.length === 0) {
    description = "输入法菜单里已经有水杉输入法的全部入口。";
  } else {
    const where = missing.map((entry) => `「${entry.name}」在「${entry.language}」下`).join("，");
    description = `${
      current ? `菜单栏里还没有「${current.name}」，要先把它加进输入法列表才能从菜单栏切过去。` : ""
    }macOS 只允许你自己添加：点「打开键盘设置」，在「输入法」一行点「编辑…」，再点左下角「+」，在左栏选或搜索对应的语言后添加。还没加入的：${where}。`;
  }

  const openSettings = () => {
    watch();
    return client
      .openSettings()
      .catch(() =>
        onError("无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。"),
      );
  };

  return (
    <Row title="菜单栏入口" description={description}>
      {missing.length > 0 && (
        <ActionButton action={openSettings} className="secondary" label="打开键盘设置" />
      )}
    </Row>
  );
}
