import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Row } from "../core/platform-controls";
import { isComposingKeyEvent } from "../core/ime-key-event";
import type { AiProviderOption } from "./ai-provider-options";
import { AiProviderIcon } from "./ai-provider-icon";
import { SelectRow } from "./select-row";
import { useOptionalSettingsForm } from "./settings-form-context";

export interface AiProviderSelectProps {
  options: readonly AiProviderOption[];
  value: string;
  disabled?: boolean;
  hidden?: boolean;
  onChange: (value: string) => void;
}

/** 鸿蒙手机保留整行选择面板，其余宿主使用能承载图标的服务商菜单。 */
export function AiProviderSelect(props: AiProviderSelectProps) {
  const form = useOptionalSettingsForm();
  if (form?.settingsPlatform === "harmony") {
    return (
      <SelectRow
        title="服务提供商"
        aria-label="AI 服务提供商"
        value={props.value}
        disabled={props.disabled}
        onChange={(event) => props.onChange(event.target.value)}
      >
        {props.options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.title}
          </option>
        ))}
      </SelectRow>
    );
  }
  return <IconProviderSelect {...props} />;
}

function IconProviderSelect({ options, value, disabled, hidden, onChange }: AiProviderSelectProps) {
  const id = useId();
  const root = useRef<HTMLSpanElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const search = useRef({ text: "", time: 0 });
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState({ left: 0, top: 0, width: 280, maxHeight: 400 });
  const selected = options.findIndex((option) => option.id === value);
  const current = options[selected];
  const visible = open && !disabled && !hidden && options.length > 0;
  const close = () => setOpen(false);
  const show = () => {
    if (disabled || !options.length) return;
    setActive(Math.max(0, selected));
    search.current = { text: "", time: 0 };
    setOpen(true);
  };
  const choose = (index: number) => {
    close();
    if (options[index] && options[index].id !== value) onChange(options[index].id);
  };

  useLayoutEffect(() => {
    if (!visible || !trigger.current) return;
    const rect = trigger.current.getBoundingClientRect();
    const width = Math.min(Math.max(280, rect.width), window.innerWidth - 16);
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    const down = below >= Math.min(options.length * 32 + 12, 400) || below >= above;
    const maxHeight = Math.max(32, Math.min(400, down ? below : above));
    const height = Math.min(options.length * 32 + 12, maxHeight);
    setPosition({
      left: Math.max(8, Math.min(rect.right - width, window.innerWidth - width - 8)),
      top: down ? rect.bottom + 4 : rect.top - height - 4,
      width,
      maxHeight,
    });
  }, [visible, options.length]);

  useEffect(() => {
    if (!visible) return;
    const outside = (event: PointerEvent) => {
      if (
        !root.current?.contains(event.target as Node) &&
        !menu.current?.contains(event.target as Node)
      )
        close();
    };
    const scroll = (event: Event) => {
      if (!menu.current?.contains(event.target as Node)) close();
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [visible]);

  useEffect(() => {
    if (disabled || hidden) close();
  }, [disabled, hidden]);

  useEffect(() => {
    const list = menu.current;
    const option = document.getElementById(`${id}-${active}`);
    if (!visible || !list || !option) return;
    // 只滚动菜单自身；scrollIntoView 会连外层页面一起移动，触发外部滚动关闭。
    const top = list.getBoundingClientRect().top + list.clientTop;
    const rect = option.getBoundingClientRect();
    if (rect.top < top) list.scrollTop -= top - rect.top;
    else if (rect.bottom > top + list.clientHeight)
      list.scrollTop += rect.bottom - top - list.clientHeight;
  }, [visible, active, id]);

  return (
    <Row title="服务提供商">
      <span ref={root} className="ai-provider-select">
        <button
          ref={trigger}
          type="button"
          role="combobox"
          aria-label="AI 服务提供商"
          aria-haspopup="listbox"
          aria-expanded={visible}
          aria-controls={visible ? id : undefined}
          aria-activedescendant={visible ? `${id}-${active}` : undefined}
          disabled={disabled || !options.length}
          className="ai-provider-trigger"
          onClick={() => (visible ? close() : show())}
          onBlur={(event) => {
            if (!root.current?.contains(event.relatedTarget as Node)) close();
          }}
          onKeyDown={(event) => {
            if (isComposingKeyEvent(event)) return;
            if (event.key === "Escape") {
              if (visible) {
                event.preventDefault();
                event.stopPropagation();
                close();
              }
            } else if (event.key === "Tab") {
              if (visible) choose(active);
            } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
              event.preventDefault();
              if (!visible) show();
              const start = visible ? active : Math.max(0, selected);
              setActive(
                event.key === "Home"
                  ? 0
                  : event.key === "End"
                    ? options.length - 1
                    : !visible
                      ? start
                      : (start + (event.key === "ArrowDown" ? 1 : -1) + options.length) %
                        options.length,
              );
            } else if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              if (visible) choose(active);
              else show();
            } else if (
              event.key.length === 1 &&
              !event.ctrlKey &&
              !event.metaKey &&
              !event.altKey
            ) {
              event.preventDefault();
              if (!visible) show();
              const now = Date.now();
              const previous = now - search.current.time < 700 ? search.current.text : "";
              const text = previous + event.key.toLocaleLowerCase();
              search.current = { text, time: now };
              const repeated = [...text].every((letter) => letter === text[0]);
              const query = repeated ? text[0] : text;
              const start = visible ? active : Math.max(0, selected);
              for (
                let step = repeated ? 1 : 0;
                step < options.length + (repeated ? 1 : 0);
                step++
              ) {
                const index = (start + step) % options.length;
                if (options[index].title.toLocaleLowerCase().startsWith(query)) {
                  setActive(index);
                  break;
                }
              }
            }
          }}
        >
          <AiProviderIcon provider={current?.id ?? value} />
          <span className="ai-provider-title">{current?.title ?? (value || "选择服务商")}</span>
          <svg className="ai-provider-chevron" viewBox="0 0 16 16" aria-hidden="true">
            <path d="m5 6 3-3 3 3M5 10l3 3 3-3" />
          </svg>
        </button>
        {visible &&
          createPortal(
            <div
              ref={menu}
              id={id}
              role="listbox"
              aria-label="AI 服务提供商"
              className="ai-provider-menu"
              style={{ position: "fixed", ...position }}
            >
              {options.map((option, index) => (
                <div
                  key={option.id}
                  id={`${id}-${index}`}
                  role="option"
                  aria-selected={option.id === value}
                  data-active={index === active || undefined}
                  className="ai-provider-option"
                  onPointerMove={() => setActive(index)}
                  onPointerDown={(event) => event.preventDefault()}
                  onClick={() => {
                    choose(index);
                    trigger.current?.focus();
                  }}
                >
                  <AiProviderIcon provider={option.id} />
                  <span className="ai-provider-title">{option.title}</span>
                  <span className="ai-provider-check" aria-hidden="true">
                    {option.id === value ? "✓" : ""}
                  </span>
                </div>
              ))}
            </div>,
            root.current?.closest("[data-settings-shell]") ?? document.body,
          )}
      </span>
    </Row>
  );
}
