import * as settings from "./settings-style";

/** The head of a view opened from the 我的插件 list: the way back to the list, then the view's title. `data-plugin-back` is where focus lands when the view opens. */
export function PluginViewHeader({ title, onBack }: { title: string; onBack: () => void }) {
  return (
    <div className={settings.subViewHeader}>
      <button
        type="button"
        className={settings.backLink}
        aria-label="返回我的插件"
        data-plugin-back=""
        onClick={onBack}
      >
        <span aria-hidden="true">‹ </span>
        我的插件
      </button>
      <h2 className={settings.subViewTitle}>{title}</h2>
    </div>
  );
}
