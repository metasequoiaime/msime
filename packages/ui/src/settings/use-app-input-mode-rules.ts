import { useEffect, useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useLatestRef } from "../core/use-latest-ref";
import {
  appInputModeRuleProblem,
  appInputModeRuleProblemText,
  maxAppInputModeRules,
  normalizeAppInputModeRuleId,
  sortedAppInputModeRules,
  type AppInputModeRule,
  type PickedApplication,
} from "./app-input-mode-rules";
import { useAsyncGeneration } from "./use-async-generation";

export interface UseAppInputModeRulesOptions {
  value?: Record<string, AppInputModeRule>;
  /** Windows 使用进程基名，macOS 使用 Bundle ID；决定规范化与提示。 */
  windows: boolean;
  /** Linux 使用输入法框架报告的程序名（Fcitx5 的 `program()`、IBus 里 GTK 程序的客户端名）；决定规范化与提示。 */
  linux?: boolean;
  /** 宿主的选择对话框；取消时返回 null，缺省时由调用方提供手填输入。 */
  pickApplication?: () => Promise<PickedApplication | null>;
  /** 按规则标识查询本机应用名称；缺失或失败时缓存标识回退。 */
  applicationNames?: (ids: string[]) => Promise<Record<string, string>>;
  onChange: (rules: Record<string, AppInputModeRule>) => void;
}

export interface AppInputModeRulesState {
  draft: string;
  warning: string | null;
  names: Record<string, string>;
  entries: [string, AppInputModeRule][];
  full: boolean;
  picking: boolean;
  edit: (value: string) => void;
  addTyped: () => void;
  pick: (() => Promise<void> | undefined) | undefined;
  changeMode: (id: string, mode: AppInputModeRule) => void;
  remove: (id: string) => void;
}

/** 受控规则表的草稿、名称查询及编辑动作，异步选择完成时合并当前规则。 */
export function useAppInputModeRules(options: UseAppInputModeRulesOptions): AppInputModeRulesState {
  const { value, windows, linux = false, pickApplication, applicationNames } = options;
  const latest = useLatestRef(options);
  const [draft, setDraft] = useState("");
  const [warning, setWarning] = useState<string | null>(null);
  const [names, setNames] = useState<Record<string, string>>({});
  const entries = sortedAppInputModeRules(value);
  const unnamedKey = entries
    .map(([id]) => id)
    .filter((id) => !(id in names))
    .join("\n");
  const namesGeneration = useAsyncGeneration(applicationNames, unnamedKey);
  const { run, busy: picking } = useAsyncActionRunner(
    setWarning,
    undefined,
    pickApplication,
    windows,
  );

  useEffect(() => {
    setWarning(null);
  }, [pickApplication, windows, linux]);

  useEffect(() => {
    if (!applicationNames || unnamedKey === "") return;
    const current = namesGeneration.current;
    const ids = unnamedKey.split("\n");
    void applicationNames(ids).then(
      (found) => {
        if (namesGeneration.current !== current) return;
        // 缺失名称也缓存为标识，模式变化时不反复查同一批应用。
        setNames((known) => ({
          ...known,
          ...Object.fromEntries(ids.map((id) => [id, found[id] || id])),
        }));
      },
      () => {
        if (namesGeneration.current === current)
          setNames((known) => ({ ...known, ...Object.fromEntries(ids.map((id) => [id, id])) }));
      },
    );
  }, [applicationNames, unnamedKey, namesGeneration]);

  const addRule = (raw: string): boolean => {
    const current = latest.current;
    const rules = current.value ?? {};
    const id = normalizeAppInputModeRuleId(raw, current.windows, current.linux);
    const problem = appInputModeRuleProblem(id, rules, current.windows);
    setWarning(problem && appInputModeRuleProblemText(problem, current.windows, current.linux));
    if (problem) return false;
    current.onChange({ ...rules, [id]: "chinese" });
    return true;
  };
  const addTyped = () => {
    if (addRule(draft)) setDraft("");
  };
  const pick =
    pickApplication &&
    (() => {
      if (Object.keys(latest.current.value ?? {}).length >= maxAppInputModeRules) return;
      return run(
        async (isCurrent) => {
          const picked = await pickApplication();
          if (!isCurrent() || !picked) return;
          const id = normalizeAppInputModeRuleId(
            picked.id,
            latest.current.windows,
            latest.current.linux,
          );
          if (addRule(picked.id)) setNames((known) => ({ ...known, [id]: picked.name || id }));
        },
        {
          formatError: (error) =>
            errorCode(error) === "not_application"
              ? "所选项目不是应用程序，无法加入应用例外。"
              : "无法打开选择应用的对话框，请稍后再试。",
        },
      );
    });
  const changeMode = (id: string, mode: AppInputModeRule) => {
    const current = latest.current;
    current.onChange({ ...current.value, [id]: mode });
  };
  const remove = (id: string) => {
    const current = latest.current;
    const next = { ...current.value };
    delete next[id];
    current.onChange(next);
  };
  return {
    draft,
    warning: warning || null,
    names,
    entries,
    full: entries.length >= maxAppInputModeRules,
    picking,
    edit: (text) => {
      setDraft(text);
      setWarning(null);
    },
    addTyped,
    pick,
    changeMode,
    remove,
  };
}
