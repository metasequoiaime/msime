/**
 * The redesign's settings primitives. Each renders one markup for every platform; the shape comes from the `--p-*` tokens the nearest `[data-platform]` ancestor sets (see `theme/platform-tokens.ts`), so a Windows card row, a macOS inset group and a Material list are the same component. Native elements and roles are kept throughout: a switch is a checkbox with `role="switch"`, a segmented control is a radio group, a select is a `<select>`, a slider is a range input.
 *
 * A control placed in a `Row` is named and described by the row's title and description unless it is given its own `aria-label` or `aria-labelledby`.
 */
import {
  createContext,
  isValidElement,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type ReactNode,
  type Ref,
  type SelectHTMLAttributes,
} from "react";
import { ActionSheet, type SheetOption } from "./action-sheet";
import { FluentIcon } from "./fluent-icons";
import * as style from "./platform-controls-style";

type RowLabels = { labelId: string; descriptionId?: string };
const RowLabelsContext = createContext<RowLabels | null>(null);

type Labelled = { "aria-label"?: string; "aria-labelledby"?: string; "aria-describedby"?: string };

function useRowLabels(own: Labelled): Labelled {
  const row = useContext(RowLabelsContext);
  if (own["aria-label"] || own["aria-labelledby"]) return own;
  return {
    "aria-labelledby": row?.labelId,
    "aria-describedby": own["aria-describedby"] ?? row?.descriptionId,
  };
}

export function GroupList({ title, children }: { title?: ReactNode; children: ReactNode }) {
  const titleId = useId();
  return (
    <section className={style.group} aria-labelledby={title ? titleId : undefined}>
      {/* `data-group-title` marks the name of a set of settings, as `data-row-title` marks one setting's. */}
      {title && (
        <h3 id={titleId} className={style.groupTitle} data-group-title="">
          {title}
        </h3>
      )}
      <div className={style.groupRows}>{children}</div>
    </section>
  );
}

export function Row({
  title,
  description,
  icon,
  hidden,
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  /** Keeps a row that does not apply right now in the page, out of sight and out of the accessibility tree, the way the `hidden` attribute does on any element. */
  hidden?: boolean;
  /** Shown where the platform leads rows with a glyph (Windows); decorative. */
  icon?: ReactNode;
  /** The control on the trailing edge. */
  children?: ReactNode;
}) {
  const labelId = useId();
  const descriptionId = useId();
  return (
    <div className={style.row} hidden={hidden}>
      {icon && (
        <span className={style.rowIcon} aria-hidden="true">
          {icon}
        </span>
      )}
      <span className={style.rowText}>
        {/* `data-row-title` marks the setting's name for whatever reads a page's settings back, as `section-title` does on the older markup. */}
        <span id={labelId} className={style.rowTitle} data-row-title="">
          {title}
        </span>
        {description && (
          <span id={descriptionId} className={style.rowDescription}>
            {description}
          </span>
        )}
      </span>
      {children && (
        <RowLabelsContext.Provider
          value={{ labelId, descriptionId: description ? descriptionId : undefined }}
        >
          <span className={style.rowControl}>{children}</span>
        </RowLabelsContext.Provider>
      )}
    </div>
  );
}

/** 跳转行：没有控件，点击后去别处。`external` 为假时打开另一个设置页，行尾画 ›；为真时打开应用外的链接，行尾画 ↗。按钮以标题命名，以说明描述。 */
export function LinkRow({
  title,
  description,
  external,
  onClick,
}: {
  title: string;
  description?: ReactNode;
  external?: boolean;
  onClick: () => void;
}) {
  const descriptionId = useId();
  return (
    <button
      type="button"
      className={style.linkRow}
      aria-label={title}
      aria-describedby={description ? descriptionId : undefined}
      onClick={onClick}
    >
      <span className={style.rowText}>
        <span className={style.rowTitle}>{title}</span>
        {description && (
          <span id={descriptionId} className={style.rowDescription}>
            {description}
          </span>
        )}
      </span>
      <span className={style.rowDescription} aria-hidden="true">
        {external ? "↗" : "›"}
      </span>
    </button>
  );
}

/** 「更多选项」折叠区：放在组内，收起不常改的进阶项（下载镜像、手动目录、接口地址之类）。展开后各行接在折叠标题下面，分隔线与组内其他行一致。名字固定为「更多选项」，不另起「高级」之类的叫法。 */
export function MoreOptions({
  defaultOpen = false,
  children,
}: {
  /** 首次显示时是否展开，例如里面的值无效、别的控件因此不能用时。只在挂载时读一次，之后由用户自己展开收起，不会因为值变对了就突然收起。 */
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open] = useState(defaultOpen);
  return (
    <details className={style.moreOptions} open={open}>
      <summary className={style.moreOptionsSummary}>
        <span className={style.rowTitle}>更多选项</span>
        <span className={style.moreOptionsMarker} aria-hidden="true">
          ›
        </span>
      </summary>
      <div className={style.moreOptionsRows}>{children}</div>
    </details>
  );
}

/** 页首说明：一段说明整页用途或适用范围的文字，放在页面 `settings.groups` 的第一项。 */
export function PageIntro({ children }: { children: ReactNode }) {
  return <p className={style.pageIntro}>{children}</p>;
}

export function Switch({
  checked,
  onChange,
  disabled,
  ...labels
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
} & Labelled) {
  const names = useRowLabels(labels);
  return (
    <span className={style.switchWrap}>
      <span className={style.switchState} aria-hidden="true">
        {checked ? "开" : "关"}
      </span>
      <input
        type="checkbox"
        role="switch"
        className="platform-switch"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
        {...names}
      />
    </span>
  );
}

export type SegmentedOption<T extends string> = { value: T; label: ReactNode; disabled?: boolean };

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  disabled,
  ...labels
}: {
  options: readonly SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
} & Labelled) {
  const name = useId();
  const names = useRowLabels(labels);
  return (
    <span role="radiogroup" className={style.segmented} {...names}>
      {options.map((option) => (
        <label key={option.value} className={style.segment}>
          <input
            type="radio"
            className={style.segmentInput}
            name={name}
            value={option.value}
            checked={option.value === value}
            disabled={disabled || option.disabled}
            onChange={() => onChange(option.value)}
          />
          {option.label}
        </label>
      ))}
    </span>
  );
}

/** A native `<select>`; options are its children, exactly as with the element itself. */
export function Select({
  className,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement> & { ref?: Ref<HTMLSelectElement> }) {
  const labels = useRowLabels({
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    "aria-describedby": ariaDescribedBy,
  });
  return (
    <select
      {...props}
      {...labels}
      className={className ? `${style.select} ${className}` : style.select}
    />
  );
}

export function Slider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  disabled,
  valueText,
  ticks = false,
  ...labels
}: {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (value: number) => void;
  disabled?: boolean;
  /** Read out instead of the bare number, e.g. "18 号". */
  valueText?: string;
  /** Marks every step along the track, for a slider over a handful of discrete values. */
  ticks?: boolean;
} & Labelled) {
  const names = useRowLabels(labels);
  const fill = max > min ? ((Math.min(Math.max(value, min), max) - min) / (max - min)) * 100 : 0;
  const steps = ticks && max > min && step > 0 ? Math.round((max - min) / step) : 0;
  const input = (
    <input
      type="range"
      className="platform-slider"
      min={min}
      max={max}
      step={step}
      value={value}
      disabled={disabled}
      aria-valuetext={valueText}
      style={{ "--slider-fill": `${fill}%` } as CSSProperties}
      onChange={(event: ChangeEvent<HTMLInputElement>) =>
        onChange(Number(event.currentTarget.value))
      }
      {...names}
    />
  );
  if (steps === 0) return input;
  return (
    <span className="platform-slider-ticks">
      <span aria-hidden="true">
        {Array.from({ length: steps + 1 }, (_, index) => (
          <span key={index} style={{ "--tick": index / steps } as CSSProperties} />
        ))}
      </span>
      {input}
    </span>
  );
}

export type CheckItem<T extends string> = {
  value: T;
  label: ReactNode;
  /** `mixed` for a parent whose children are only partly on; drawn and announced through the native `indeterminate` state. */
  checked: boolean | "mixed";
  disabled?: boolean;
};

function Check<T extends string>({
  item,
  onChange,
  className,
}: {
  item: CheckItem<T>;
  onChange: (value: T, checked: boolean) => void;
  className: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  const mixed = item.checked === "mixed";
  useEffect(() => {
    if (input.current) input.current.indeterminate = mixed;
  }, [mixed]);
  return (
    <label className={className}>
      <input
        ref={input}
        type="checkbox"
        className="platform-check"
        checked={item.checked === true}
        disabled={item.disabled}
        onChange={(event) => onChange(item.value, event.currentTarget.checked)}
      />
      {item.label}
    </label>
  );
}

/** 一组带标签的复选框。`layout="grid"` 把它们排成两列短标签，这是 HarmonyOS 手机上按键对列表的样式。 */
export function Checks<T extends string>({
  legend,
  description,
  items,
  onChange,
  layout = "list",
}: {
  legend: ReactNode;
  description?: ReactNode;
  items: readonly CheckItem<T>[];
  onChange: (value: T, checked: boolean) => void;
  layout?: "list" | "grid";
}) {
  const descriptionId = useId();
  const checks = items.map((item) => (
    <Check
      key={item.value}
      item={item}
      onChange={onChange}
      className={layout === "grid" ? style.checkGridItem : style.check}
    />
  ));
  return (
    <fieldset className={style.checks} aria-describedby={description ? descriptionId : undefined}>
      {/* The legend names the setting the way a row title does, so it carries the same `data-row-title` mark. */}
      <legend className={style.checksLegend} data-row-title="">
        {legend}
      </legend>
      {description && (
        <span id={descriptionId} className={style.checksDescription}>
          {description}
        </span>
      )}
      {layout === "grid" ? <div className={style.checksGrid}>{checks}</div> : checks}
    </fieldset>
  );
}

/** One sidebar or drawer destination. The selected one is the current page. */
export function NavItem({
  label,
  icon,
  selected,
  onSelect,
  controls,
}: {
  label: string;
  icon?: ReactNode;
  selected: boolean;
  onSelect: () => void;
  /** The id of the region the item swaps, for `aria-controls`. */
  controls?: string;
}) {
  return (
    <button
      type="button"
      className={style.navItem}
      aria-current={selected ? "page" : undefined}
      aria-controls={controls}
      onClick={onSelect}
    >
      {icon && (
        <span className={style.navIcon} aria-hidden="true">
          {icon}
        </span>
      )}
      <span className={style.navLabel}>{label}</span>
    </button>
  );
}

/** HarmonyOS 手机的导航列表：所有行放在一张无边框的 20px 圆角卡片里，使用页面的分组颜色，设置根页和我的标签页都这样绘制。 */
export function NavGroup({ children }: { children: ReactNode }) {
  return <div className={`${style.navGroup} ${style.navGroupRows}`}>{children}</div>;
}

/**
 * `NavGroup` 中的一行。`root` 变体是设置根页的样式（52px 高，文字颜色的 20px 图标，17px 标题）；`me` 是我的标签页的样式（48px 高，强调色的 18px 图标，16px 标题，下方可选描述）。两者末尾都是当前值和一个 chevron，按下时变暗，并从标签起始处绘制一条内缩的分隔线。
 *
 * 这一行是调用 `onClick` 的按钮。带 `trailing` 时它容纳一个自己的控件（例如开关），因此改为普通行，没有 chevron，该控件由行的标题和描述来命名和描述，与 `Row` 命名其控件的方式相同。
 */
export function NavRow({
  icon,
  title,
  description,
  value,
  trailing,
  onClick,
  disabled,
  variant = "root",
}: {
  icon?: ReactNode;
  title: string;
  description?: string;
  value?: string;
  trailing?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant?: "root" | "me";
}) {
  const titleId = useId();
  const descriptionId = useId();
  const valueId = useId();
  const describedBy =
    [value ? valueId : "", description ? descriptionId : ""].filter(Boolean).join(" ") || undefined;
  const body = (
    <>
      {icon && (
        <span className={style.navRowIcon[variant]} aria-hidden="true">
          {icon}
        </span>
      )}
      <span className={style.navRowBody[variant]} data-nav-rule="">
        <span className={style.navRowText}>
          <span id={titleId} className={style.navRowTitle[variant]} data-row-title="">
            {title}
          </span>
          {description && (
            <span id={descriptionId} className={style.navRowDescription}>
              {description}
            </span>
          )}
        </span>
        {value && (
          <span id={valueId} className={style.navRowValue[variant]}>
            {value}
          </span>
        )}
        {trailing ? (
          <RowLabelsContext.Provider
            value={{ labelId: titleId, descriptionId: description ? descriptionId : undefined }}
          >
            <span className={style.navRowTrailing}>{trailing}</span>
          </RowLabelsContext.Provider>
        ) : (
          <FluentIcon name="chevron_right" size={20} className={style.navRowChevron} />
        )}
      </span>
    </>
  );
  if (trailing)
    return (
      <div className={style.navRow} aria-disabled={disabled || undefined}>
        {body}
      </div>
    );
  return (
    <button
      type="button"
      className={`${style.navRow} ${style.navRowButton}`}
      aria-labelledby={titleId}
      aria-describedby={describedBy}
      disabled={disabled}
      onClick={onClick}
    >
      {body}
    </button>
  );
}

/**
 * HarmonyOS 手机上选择或分段行的形式：整行是一个按钮，点击打开以行标题为标题、包含相同选项的 `ActionSheet`，并在小 chevron 前以灰色显示当前选择。原控件仍留在页面中，视觉隐藏且 inert，作为写入和读取选择的元素；`control` 接收它。
 */
export function PickerRow({
  title,
  description,
  hidden,
  disabled,
  valueLabel,
  options,
  onSelect,
  control,
}: {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  disabled?: boolean;
  /** 显示在末端的当前选择。 */
  valueLabel: string;
  options: readonly SheetOption[];
  onSelect: (value: string) => void;
  /** 原生控件，以给定的标题 id 作为标签。 */
  control: (titleId: string) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const titleId = useId();
  const sheetTitle = textOf(title);
  return (
    <div className="min-w-0" hidden={hidden}>
      {/* 按内容（标题、描述、当前值）命名，而不是用 `aria-labelledby`：标题已经是隐藏控件的标签，同一个标签命名两个控件会让标签查找指向哪个控件变得含糊。 */}
      <button
        type="button"
        className={style.pickerRow}
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <span className={style.rowText}>
          <span id={titleId} className={style.rowTitle} data-row-title="">
            {title}
          </span>
          {description && <span className={style.rowDescription}>{description}</span>}
        </span>
        <span className={style.pickerValue}>
          <span className={style.pickerValueText}>{valueLabel}</span>
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
            className="shrink-0"
          >
            <path d="m9 6 6 6-6 6" />
          </svg>
        </span>
      </button>
      {/* inert 让隐藏控件不受触摸、焦点和辅助技术的访问，用户遇到的唯一控件就是行按钮；它仍然保存值并接收变更。 */}
      <span className="sr-only" inert>
        {control(titleId)}
      </span>
      <ActionSheet
        open={open}
        title={sheetTitle}
        options={options}
        onSelect={onSelect}
        onClose={() => setOpen(false)}
      />
    </div>
  );
}

/** 节点会显示的文本，用于由行的标记构建面板标题或选项标签。 */
export function textOf(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number" || typeof node === "bigint")
    return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) return textOf(node.props.children);
  return "";
}
