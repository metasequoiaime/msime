import { useEffect, useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useLatestRef } from "../core/use-latest-ref";
import {
  appInputModeRuleProblem,
  appInputModeRuleProblemText,
  maxAppInputModeRules,
  normalizeAppInputModeRuleId,
  type AppInputModeRule,
  type PickedApplication,
} from "./app-input-mode-rules";
import { useAsyncGeneration } from "./use-async-generation";

export interface UseVimModeAppsOptions {
  /** 复用应用名单交互时，在选择错误中显示的名单名称。 */
  listName?: string;
  value?: string[];
  /** Windows 使用进程基名，macOS 使用 Bundle ID；决定规范化与提示。 */
  windows: boolean;
  /** 宿主的选择对话框；取消时返回 null，缺省时由调用方提供手填输入。 */
  pickApplication?: () => Promise<PickedApplication | null>;
  /** 按标识查询本机应用名称；缺失或失败时缓存标识回退。 */
  applicationNames?: (ids: string[]) => Promise<Record<string, string>>;
  onChange: (apps: string[]) => void;
}

export interface VimModeAppsState {
  draft: string;
  warning: string | null;
  names: Record<string, string>;
  entries: string[];
  full: boolean;
  picking: boolean;
  edit: (value: string) => void;
  addTyped: () => void;
  pick: (() => Promise<void> | undefined) | undefined;
  remove: (id: string) => void;
}

/** Vim 模式应用名单的草稿、名称查询及编辑动作，与 useAppInputModeRules 同一份契约，只是没有模式。 */
export function useVimModeApps(options: UseVimModeAppsOptions): VimModeAppsState {
  const { value, windows, pickApplication, applicationNames } = options;
  const latest = useLatestRef(options);
  const [draft, setDraft] = useState("");
  const [warning, setWarning] = useState<string | null>(null);
  const [names, setNames] = useState<Record<string, string>>({});
  const entries = [...(value ?? [])].sort((left, right) =>
    left < right ? -1 : left > right ? 1 : 0,
  );
  const unnamedKey = entries.filter((id) => !(id in names)).join("\n");
  const namesGeneration = useAsyncGeneration(applicationNames, unnamedKey);
  const { run, busy: picking } = useAsyncActionRunner(
    setWarning,
    undefined,
    pickApplication,
    windows,
  );

  useEffect(() => {
    setWarning(null);
  }, [pickApplication, windows]);

  useEffect(() => {
    if (!applicationNames || unnamedKey === "") return;
    const current = namesGeneration.current;
    const ids = unnamedKey.split("\n");
    void applicationNames(ids).then(
      (found) => {
        if (namesGeneration.current !== current) return;
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

  const addApp = (raw: string): boolean => {
    const current = latest.current;
    const apps = current.value ?? [];
    const id = normalizeAppInputModeRuleId(raw, current.windows);
    // 名单没有模式，借用应用例外的校验：把名单看成全是 chinese 的规则表。
    const problem = appInputModeRuleProblem(
      id,
      Object.fromEntries(apps.map((app) => [app, "chinese" as AppInputModeRule])),
      current.windows,
    );
    setWarning(problem && appInputModeRuleProblemText(problem, current.windows));
    if (problem) return false;
    current.onChange([...apps, id]);
    return true;
  };
  const addTyped = () => {
    if (addApp(draft)) setDraft("");
  };
  const pick =
    pickApplication &&
    (() => {
      if ((latest.current.value ?? []).length >= maxAppInputModeRules) return;
      return run(
        async (isCurrent) => {
          const picked = await pickApplication();
          if (!isCurrent() || !picked) return;
          const id = normalizeAppInputModeRuleId(picked.id, latest.current.windows);
          if (addApp(picked.id)) setNames((known) => ({ ...known, [id]: picked.name || id }));
        },
        {
          formatError: (error) =>
            errorCode(error) === "not_application"
              ? `所选项目不是应用程序，无法加入${latest.current.listName ?? " Vim 模式名单"}。`
              : "无法打开选择应用的对话框，请稍后再试。",
        },
      );
    });
  const remove = (id: string) => {
    const current = latest.current;
    current.onChange((current.value ?? []).filter((app) => app !== id));
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
    remove,
  };
}
