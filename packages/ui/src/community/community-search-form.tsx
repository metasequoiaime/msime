import { boundedGraphemes } from "../core/text";
import * as style from "./community-style";

export interface CommunitySearchFormProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}

/** Shared bounded search form used by the community galleries. */
export function CommunitySearchForm({
  label,
  value,
  onChange,
  onSubmit,
}: CommunitySearchFormProps) {
  return (
    <form
      className={style.searchRow}
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <input
        className={style.searchInput}
        aria-label={label}
        placeholder={label}
        value={value}
        onChange={(event) => onChange(boundedGraphemes(event.target.value, 128))}
      />
      <button type="submit" className={style.searchSubmit}>
        搜索
      </button>
    </form>
  );
}
