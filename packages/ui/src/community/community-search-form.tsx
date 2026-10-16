import { boundedGraphemes } from "../core/text";
import * as style from "./community-style";

export interface CommunitySearchFormProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  /** 鸿蒙手机使用胶囊输入框，以键盘搜索键提交；缺省保留桌面搜索按钮。 */
  look?: "harmony";
}

/** 社区图库共用的有界搜索表单，两种外观共用输入与提交规则。 */
export function CommunitySearchForm({
  label,
  value,
  onChange,
  onSubmit,
  look,
}: CommunitySearchFormProps) {
  const harmony = look === "harmony";
  return (
    <form
      className={harmony ? "m-0 min-w-0" : style.searchRow}
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <input
        className={harmony ? style.harmonySearch : style.searchInput}
        aria-label={label}
        placeholder={label}
        enterKeyHint={harmony ? "search" : undefined}
        value={value}
        onChange={(event) => onChange(boundedGraphemes(event.target.value, 128))}
      />
      {!harmony && (
        <button type="submit" className={style.searchSubmit}>
          搜索
        </button>
      )}
    </form>
  );
}

/** 兼容原鸿蒙搜索入口，逻辑统一由公共搜索表单持有。 */
export function CommunityHarmonySearch(props: Omit<CommunitySearchFormProps, "look">) {
  return <CommunitySearchForm {...props} look="harmony" />;
}
