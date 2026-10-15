import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ConfirmRequest } from "../core/confirm";
import { downloadBlob } from "../core/download-blob";
import { ErrorAlert } from "../core/error-alert";
import { GroupList, PageIntro, Row } from "../core/platform-controls";
import { select as selectStyle, textInput } from "../core/platform-controls-style";
import { StatusMessage } from "../core/status-message";
import { useAsyncActionRunner, type AsyncActionOperation } from "../core/use-async-action";
import { useDebouncedCallback } from "../core/use-debounced-callback";
import { readDictionaryFile } from "../dictionary/dictionary-file";
import { ActionButton } from "./action-button";
import { SettingsManagerActions } from "./settings-manager-actions";
import * as settings from "./settings-style";
import { useAsyncGeneration } from "./use-async-generation";
import {
  builtinShuangpinProfiles,
  emptyShuangpinCustomProfile,
  MAX_SHUANGPIN_CUSTOM_IMPORT_BYTES,
  normalizedShuangpinCustomProfile,
  parseShuangpinCustomProfile,
  sameShuangpinCustomProfile,
  SHUANGPIN_CUSTOM_EXPORT_NAME,
  shuangpinCustomProblemMessage,
  shuangpinCustomProfileExport,
  shuangpinCustomUnits,
  type BuiltinShuangpinProfile,
  type ShuangpinCustomProblem,
  type ShuangpinCustomProfile,
  type ShuangpinCustomProfileClient,
  type ShuangpinCustomTablePart,
} from "./shuangpin-custom-profile";

/** 改键之后等这么久再问宿主，连续打几个键只校验最后那张表。 */
const VALIDATE_DELAY_MS = 150;

type Draft = Required<ShuangpinCustomProfile>;

const partGroups: readonly {
  part: ShuangpinCustomTablePart;
  title: string;
  note: string;
  maxLength: number;
}[] = [
  {
    part: "initials",
    title: "声母",
    note: "单字母声母固定在自己的字母键上，这里只排 zh、ch、sh，各一个字母键，不能放在单字母声母的键上。",
    maxLength: 1,
  },
  {
    part: "finals",
    title: "韵母",
    note: "每个韵母一个键，可以是字母或 ;，ü 写作 v。同一个键上的韵母不能和同一个声母拼出两个音节。",
    maxLength: 1,
  },
  {
    part: "zero_initials",
    title: "零声母",
    note: "没有声母的音节各用两个键，第一个是字母，第二个是字母或 ;，不能和别的音节的编码相同。",
    maxLength: 2,
  },
];

export interface ShuangpinCustomEditorProps {
  client: ShuangpinCustomProfileClient;
  /** 偏好里存着的表；没有时从空表开始。 */
  saved: ShuangpinCustomProfile | undefined;
  /** 当前方案已经是自定义双拼。不是时，保存会同时切到它。 */
  active: boolean;
  /** 宿主把导出文件写进「下载」并回答写到了哪里（`SettingsClient.saveExport`）；没有时退回下载链接。 */
  saveExport?: (name: string, contents: string) => Promise<string | null>;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
  /** 把这张合法的表写进偏好，并选用自定义双拼。 */
  onSave: (profile: Draft) => void;
  onBack: () => void;
}

/**
 * 自定义双拼的编辑页：从内置方案开始、逐格改键、导入导出 JSON。表合不合法只问宿主（host-api 用 Engine 不留存的检查），与 MCP 保存前的校验结论相同；不合法时不能保存，并写明是哪一格、为什么。Engine 在第一个错误处就停，所以一次只报一处，改好一处再看下一处。
 *
 * 保存只写 `shuangpin_profile: "custom"` 和 `shuangpin_custom_profile`，随设置页的自动保存生效。导入导出只在本机读写文件，表不进账号同步。
 */
export function ShuangpinCustomEditor({
  client,
  saved,
  active,
  saveExport,
  confirm,
  onSave,
  onBack,
}: ShuangpinCustomEditorProps) {
  const [draft, setDraft] = useState<Draft>(() => normalizedShuangpinCustomProfile(saved));
  // `undefined` 是还在校验（或校验失败）：这时不能保存，也不能导出。
  const [problem, setProblem] = useState<ShuangpinCustomProblem | null | undefined>(undefined);
  const [template, setTemplate] = useState<BuiltinShuangpinProfile>("xiaohe");
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  // 宿主没能回答这张表能不能用；改下一个键时重新问。
  const [checkFailed, setCheckFailed] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const back = useRef<HTMLButtonElement>(null);
  const {
    busy,
    mounted,
    generation: actionGeneration,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, setNotice, client, saveExport);
  const generation = useAsyncGeneration(client, draft);
  const { schedule: validateLater } = useDebouncedCallback(
    (validationClient: ShuangpinCustomProfileClient, profile: Draft, current: number) => {
      validationClient
        .validate(profile)
        .then((result) => {
          if (mounted.current && current === generation.current) setProblem(result ?? null);
        })
        .catch(() => {
          if (mounted.current && current === generation.current) setCheckFailed(true);
        });
    },
    VALIDATE_DELAY_MS,
  );
  // 打开时焦点落在返回上，键盘用户不会停在一个已经消失的「编辑」行上。
  useLayoutEffect(() => {
    back.current?.focus();
    back.current?.scrollIntoView?.({ block: "nearest" });
  }, []);

  useEffect(() => {
    const current = ++generation.current;
    setProblem(undefined);
    setCheckFailed(false);
    validateLater(client, draft, current);
  }, [client, draft, validateLater]);

  const dirty = !sameShuangpinCustomProfile(draft, saved);
  const valid = problem === null;
  const canSave = valid && !busy && (dirty || !active);

  const setKey = (part: ShuangpinCustomTablePart, unit: string, key: string) => {
    setNotice("");
    setDraft((current) => {
      const next = { ...current[part] };
      const value = key.trim().toLowerCase();
      if (value) next[unit] = value;
      else delete next[unit];
      return { ...current, [part]: next };
    });
  };

  const run = (action: AsyncActionOperation) =>
    runAsyncAction(action, {
      formatError: () => "操作失败，请稍后重试。",
    });

  const applyTemplate = () =>
    run(async (isCurrent) => {
      if (
        !emptyShuangpinCustomProfile(draft) &&
        !(await confirm({
          title: "换成内置方案",
          message: "现在填的键位会被这个内置方案的键位替换。",
          confirmLabel: "替换",
        }))
      )
        return;
      if (!isCurrent()) return;
      try {
        const table = await client.builtin(template);
        if (!isCurrent()) return;
        if (!table) {
          setError("这个方案没有可用的键位表。");
          return;
        }
        setDraft(normalizedShuangpinCustomProfile(table));
        const title = builtinShuangpinProfiles.find(([name]) => name === template)?.[1] ?? "";
        setNotice(`已填入${title}的键位，改动想换的键后保存。`);
      } catch {
        if (isCurrent()) setError("无法读取内置方案的键位，请稍后重试。");
      }
    });

  const importFile = (file: File | undefined) => {
    if (!file) return;
    void run(async (isCurrent) => {
      let profile: ShuangpinCustomProfile;
      try {
        profile = parseShuangpinCustomProfile(
          await readDictionaryFile(file, MAX_SHUANGPIN_CUSTOM_IMPORT_BYTES),
        );
      } catch (cause) {
        if (isCurrent())
          setError(
            `没有导入：${cause instanceof Error ? cause.message : "无法读取所选文件，请重新选择。"}`,
          );
        return;
      }
      if (!isCurrent()) return;
      let result: ShuangpinCustomProblem | null;
      try {
        result = await client.validate(normalizedShuangpinCustomProfile(profile));
      } catch {
        if (isCurrent()) setError("无法校验导入的键位表，请稍后重试。");
        return;
      }
      if (!isCurrent()) return;
      // 不合法的表不进草稿，免得把正在编辑的表冲掉，只换来一张存不了的表。
      if (result) {
        setError(`没有导入：${shuangpinCustomProblemMessage(result)}`);
        return;
      }
      setDraft(normalizedShuangpinCustomProfile(profile));
      setNotice(`已导入 ${file.name}，保存后生效。`);
    });
  };

  const exportFile = () =>
    run(async (isCurrent) => {
      const contents = shuangpinCustomProfileExport(draft);
      if (!saveExport) {
        downloadBlob(
          new Blob([contents], { type: "application/json;charset=utf-8" }),
          SHUANGPIN_CUSTOM_EXPORT_NAME,
        );
        setNotice("已导出键位表。");
        return;
      }
      try {
        const path = await saveExport(SHUANGPIN_CUSTOM_EXPORT_NAME, contents);
        if (!isCurrent()) return;
        setNotice(path === null ? "已取消导出。" : `已导出到 ${path}`);
      } catch (cause) {
        if (isCurrent())
          setError(
            cause instanceof Error && cause.message
              ? cause.message
              : "无法写入“下载”文件夹，键位表未导出。",
          );
      }
    });

  const save = () => {
    if (!canSave) return;
    onSave(draft);
    setNotice(active ? "已保存。" : "已保存，并已选用自定义双拼。");
  };

  const leave = async () => {
    const current = actionGeneration.current;
    if (
      dirty &&
      !(await confirm({
        title: "放弃改动",
        message: "还没有保存的键位改动会丢失。",
        confirmLabel: "放弃",
        danger: true,
      }))
    )
      return;
    if (!mounted.current || current !== actionGeneration.current) return;
    onBack();
  };

  // 出错的那一格：编码冲突不属于某一格，只在下面写明。
  const invalidCell =
    problem && problem.part && problem.unit ? `${problem.part}/${problem.unit}` : null;

  return (
    <div className={settings.subViewStack}>
      <div className={settings.subViewHeader}>
        <button
          ref={back}
          type="button"
          className={settings.backLink}
          aria-label="返回输入"
          onClick={() => void leave()}
        >
          <span aria-hidden="true">‹ </span>
          输入
        </button>
        <h2 className={settings.subViewTitle}>自定义双拼</h2>
      </div>
      <PageIntro>
        自己安排 zh、ch、sh、33 个韵母和 12
        个零声母音节的键位。表要能用才能保存，合不合法与输入法实际运行时的判定相同。
      </PageIntro>
      <GroupList title="起点与文件">
        <Row title="从内置方案开始" description="用它的键位填满整张表，再改动想换的键。">
          <select
            className={selectStyle}
            aria-label="内置方案"
            value={template}
            disabled={busy}
            onChange={(event) => setTemplate(event.target.value as BuiltinShuangpinProfile)}
          >
            {builtinShuangpinProfiles.map(([name, title]) => (
              <option key={name} value={name}>
                {title}
              </option>
            ))}
          </select>
          <ActionButton action={applyTemplate} disabled={busy} label="填入" />
        </Row>
        <Row
          title="导入与导出"
          description="JSON 文件，形状与偏好里的 shuangpin_custom_profile 相同；只在本机读写，不会上传。导入的表要能用才会替换现在的草稿。"
        >
          <SettingsManagerActions as="span">
            <ActionButton
              action={() => fileInput.current?.click()}
              disabled={busy}
              label="导入 JSON"
            />
            <ActionButton action={exportFile} disabled={busy || !valid} label="导出 JSON" />
          </SettingsManagerActions>
          <input
            ref={fileInput}
            hidden
            type="file"
            aria-label="选择自定义双拼 JSON 文件"
            accept=".json,application/json"
            onChange={(event) => {
              importFile(event.currentTarget.files?.[0]);
              event.currentTarget.value = "";
            }}
          />
        </Row>
      </GroupList>
      {partGroups.map(({ part, title, note, maxLength }) => (
        <GroupList key={part} title={title}>
          <p className={settings.groupNote}>{note}</p>
          <div
            className={`${settings.groupBlock} grid grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))] gap-x-3 gap-y-2`}
          >
            {shuangpinCustomUnits[part].map((unit) => {
              const invalid = invalidCell === `${part}/${unit}`;
              return (
                <label key={unit} className="flex min-w-0 items-center justify-between gap-2">
                  <span className="[font-size:var(--p-row-fs)] [color:var(--p-text)]">{unit}</span>
                  <input
                    className={`${textInput} w-12 text-center font-mono${invalid ? " outline-2 outline-danger" : ""}`}
                    aria-label={`${title} ${unit}`}
                    aria-invalid={invalid || undefined}
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                    maxLength={maxLength}
                    value={draft[part][unit] ?? ""}
                    onChange={(event) => setKey(part, unit, event.target.value)}
                  />
                </label>
              );
            })}
          </div>
        </GroupList>
      ))}
      <GroupList title="保存">
        <Row
          title={active ? "保存键位" : "保存并选用自定义双拼"}
          description={
            problem === undefined
              ? "正在检查键位…"
              : problem
                ? "改好上面标出的问题后才能保存。"
                : dirty || !active
                  ? "这张表可以用。"
                  : "没有改动。"
          }
        >
          <ActionButton
            action={save}
            disabled={!canSave}
            className="primary"
            label={active ? "保存" : "保存并选用"}
          />
        </Row>
      </GroupList>
      {problem && <ErrorAlert>{shuangpinCustomProblemMessage(problem)}</ErrorAlert>}
      {checkFailed && <ErrorAlert>无法校验键位表，改动任意一个键后会重新校验。</ErrorAlert>}
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {notice && <StatusMessage role="status">{notice}</StatusMessage>}
    </div>
  );
}
