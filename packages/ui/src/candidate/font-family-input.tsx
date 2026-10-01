import { useEffect, useId, useState } from "react";
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
    [active, setActive] = useState(-1);
  const matches = fonts.filter(
    (font) =>
      !excluded.includes(font) &&
      (!search || font.toLocaleLowerCase().includes(value.toLocaleLowerCase())),
  );
  const options = matches.slice(0, 100);
  const selected = active >= 0 && active < options.length ? active : -1;
  useEffect(() => setActive(-1), [fonts]);
  useEffect(() => {
    if (open && selected >= 0)
      document.getElementById(`${id}-${selected}`)?.scrollIntoView?.({ block: "nearest" });
  }, [id, open, selected]);
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
    <div className="font-family-combobox">
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
      {enabled && open && (
        <div className="font-family-menu">
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
        </div>
      )}
    </div>
  );
}
