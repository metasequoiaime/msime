import { useId, useState, type ReactNode } from "react";
import { isComposingKeyEvent } from "../core/ime-key-event";
import { LabeledInputField } from "../core/labeled-input-field";
import * as dialog from "../community/community-style";
import {
  normalizePinyinCode,
  validCollectionName,
  validPinyinCode,
} from "./dictionary-collections";

/** 页内的小对话框：标题、若干输入框、取消和确认。 */
function FormDialog({
  title,
  submitLabel,
  error,
  onClose,
  onSubmit,
  children,
}: {
  title: string;
  submitLabel: string;
  error: string;
  onClose: () => void;
  onSubmit: () => Promise<void>;
  children: ReactNode;
}) {
  const titleId = useId();
  const [submitting, setSubmitting] = useState(false);
  const submit = async () => {
    if (submitting) return;
    setSubmitting(true);
    try {
      await onSubmit();
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div
      // 与底部动作面板同一层：手机上收起的页头和底部标签栏都在 z-20 之上，键盘弹起时会把对话框的按钮盖住。
      className={`${dialog.backdrop} z-50`}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !submitting) onClose();
      }}
    >
      {/* 不用 <form>：整个设置页已经在一个 <form> 里，嵌套的表单在 WebView 里会被当成外层表单提交，页面整个重新加载回首页。回车由输入框自己确认。 */}
      <div
        className={`${dialog.dialog} w-[min(420px,100%)]`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={(event) => {
          if (isComposingKeyEvent(event)) return;
          if (event.key === "Escape" && !submitting) onClose();
          if (event.key === "Enter" && event.target instanceof HTMLInputElement) {
            event.preventDefault();
            void submit();
          }
        }}
      >
        <h2 id={titleId} className={dialog.dialogTitle}>
          {title}
        </h2>
        {children}
        {error && (
          <p role="alert" className="m-0 text-[13px] text-danger">
            {error}
          </p>
        )}
        <div className={dialog.dialogActions}>
          <button type="button" className="secondary" disabled={submitting} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="primary"
            disabled={submitting}
            onClick={() => void submit()}
          >
            {submitLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

function DialogField({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <LabeledInputField
      label={label}
      labelClassName="flex flex-col gap-1.5 text-[14px] [color:var(--p-sub)]"
      className={dialog.fieldControl}
      ariaLabel={label}
      value={value}
      placeholder={placeholder}
      onChange={onChange}
    />
  );
}

/** 「新建词库」：起个名字，比如「工作」「游戏」。 */
export function DictionaryNameDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  return (
    <FormDialog
      title="新建词库"
      submitLabel="创建"
      error={error}
      onClose={onClose}
      onSubmit={async () => {
        const trimmed = name.trim();
        if (!validCollectionName(trimmed)) {
          setError("词库名需要 1–32 个字。");
          return;
        }
        await onSubmit(trimmed);
      }}
    >
      <DialogField
        label="词库名称"
        value={name}
        placeholder="比如「工作」「游戏」"
        onChange={(value) => {
          setName(value);
          setError("");
        }}
      />
    </FormDialog>
  );
}

/** 「添加词条」：词语和它的拼音。 */
export function DictionaryWordDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (word: string, code: string) => Promise<void>;
}) {
  const [word, setWord] = useState("");
  const [pinyin, setPinyin] = useState("");
  const [error, setError] = useState("");
  return (
    <FormDialog
      title="添加词条"
      submitLabel="添加"
      error={error}
      onClose={onClose}
      onSubmit={async () => {
        const value = word.trim();
        const code = normalizePinyinCode(pinyin);
        if (!value) {
          setError("请填写词语。");
          return;
        }
        if (!validPinyinCode(code)) {
          setError("拼音只能是字母，音节之间可以用空格或 ' 分开。");
          return;
        }
        await onSubmit(value, code);
      }}
    >
      <DialogField
        label="词语"
        value={word}
        onChange={(value) => {
          setWord(value);
          setError("");
        }}
      />
      <DialogField
        label="拼音"
        value={pinyin}
        placeholder="比如 shui shan"
        onChange={(value) => {
          setPinyin(value);
          setError("");
        }}
      />
    </FormDialog>
  );
}
