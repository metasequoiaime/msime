import { isComposingKeyEvent } from "../core/ime-key-event";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { validFontFamily } from "./candidate-font-family";
export function FontFamilyInput({
  label,
  value,
  onChange,
  fonts,
  enabled,
  ready,
  request,
  excluded = [],
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  fonts: readonly string[];
  enabled: boolean;
  ready: boolean;
  request: () => void;
  excluded?: readonly string[];
}) {
  const id = useId(),
    [open, setOpen] = useState(false),
    [search, setSearch] = useState(false),
    [active, setActive] = useState(-1),
    [up, setUp] = useState(false),
    [position, setPosition] = useState({ left: 0, top: 0, width: 240, maxHeight: 240 });
  const root = useRef<HTMLDivElement>(null),
    menu = useRef<HTMLDivElement>(null);
  const matches = fonts.filter(
    (font) =>
      !excluded.includes(font) &&
      (!search || font.toLocaleLowerCase().includes(value.toLocaleLowerCase())),
  );
  const options = matches.slice(0, 100);
  const selected = active >= 0 && active < options.length ? active : -1;
  const visible = enabled && open;
  useEffect(() => setActive(-1), [fonts]);
  // 菜单挂在设置外壳上、按视口定位：设置卡片 `overflow: hidden`，挂在行里的菜单会被卡片底边截断（#11889）。下方放不下时往上弹。
  useLayoutEffect(() => {
    if (!visible || !root.current) return;
    const rect = root.current.getBoundingClientRect();
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    const down = below >= 240 || below >= above;
    const maxHeight = Math.max(32, Math.min(240, down ? below : above));
    setPosition({
      left: rect.left,
      top: down ? rect.bottom + 4 : rect.top - 4,
      width: rect.width,
      maxHeight,
    });
    setUp(!down);
  }, [visible, options.length]);
  // 菜单不再随页面滚动，页面一滚或窗口一变就收起，免得它停在原处、跟字段脱开。
  useEffect(() => {
    if (!visible) return;
    const close = () => {
      setOpen(false);
      setActive(-1);
    };
    const scroll = (event: Event) => {
      if (!menu.current?.contains(event.target as Node)) close();
    };
    document.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", close);
    };
  }, [visible]);
  useEffect(() => {
    const list = menu.current;
    const option = selected >= 0 ? document.getElementById(`${id}-${selected}`) : null;
    if (!visible || !list || !option) return;
    // 只滚动菜单自身；scrollIntoView 会连外层页面一起移动，触发上面的滚动收起。
    const top = list.getBoundingClientRect().top + list.clientTop;
    const rect = option.getBoundingClientRect();
    if (rect.top < top) list.scrollTop -= top - rect.top;
    else if (rect.bottom > top + list.clientHeight)
      list.scrollTop += rect.bottom - top - list.clientHeight;
  }, [id, visible, selected]);
  const choose = (font: string) => {
    onChange(font);
    setOpen(false);
    setSearch(false);
    setActive(-1);
  };
  const show = () => {
    if (enabled) {
      setOpen(true);
      request();
    }
  };
  return (
    <div ref={root} className="font-family-combobox">
      <input
        aria-label={label}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={enabled && open}
        aria-controls={enabled && open ? id : undefined}
        aria-activedescendant={enabled && open && selected >= 0 ? `${id}-${selected}` : undefined}
        aria-invalid={!validFontFamily(value)}
        autoComplete="off"
        spellCheck={false}
        value={value}
        onFocus={show}
        onClick={show}
        onBlur={() => {
          setOpen(false);
          setActive(-1);
        }}
        onChange={(event) => {
          onChange(event.target.value);
          setSearch(true);
          setActive(-1);
          show();
        }}
        onKeyDown={(event) => {
          if (isComposingKeyEvent(event)) return;
          if (event.key === "Escape") {
            event.preventDefault();
            setOpen(false);
            setActive(-1);
          } else if (enabled && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
            event.preventDefault();
            show();
            setActive(
              options.length
                ? event.key === "ArrowDown"
                  ? (selected + 1) % options.length
                  : selected < 0
                    ? options.length - 1
                    : (selected - 1 + options.length) % options.length
                : -1,
            );
          } else if (open && event.key === "Enter" && selected >= 0) {
            event.preventDefault();
            choose(options[selected]);
          }
        }}
      />
      {/* Decorative, and only where there is a list to open: a click on it lands on the field. */}
      {enabled && (
        <svg className="font-family-chevron" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2.5 4l2.5 2.5L7.5 4" />
        </svg>
      )}
      {visible &&
        createPortal(
          <div
            ref={menu}
            className="font-family-menu"
            style={{
              position: "fixed",
              left: position.left,
              top: position.top,
              width: position.width,
              maxHeight: position.maxHeight,
              translate: up ? "0 -100%" : undefined,
            }}
          >
            <div id={id} role="listbox" aria-label={`${label}可用字体`}>
              {options.map((font, index) => (
                <div
                  key={font}
                  id={`${id}-${index}`}
                  role="option"
                  aria-selected={index === selected}
                  data-current={font === value || undefined}
                  onPointerDown={(event) => event.preventDefault()}
                  onClick={() => choose(font)}
                >
                  {font}
                </div>
              ))}
            </div>
            {ready && !options.length && <small>没有匹配的字体，可继续手动输入。</small>}
            {matches.length > 100 && <small>显示前 100 项，请输入名称缩小范围。</small>}
          </div>,
          root.current?.closest("[data-settings-shell]") ?? document.body,
        )}
    </div>
  );
}
