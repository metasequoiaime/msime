/**
 * 键盘可以直接打开哪些设置页。请求方（`KeyboardSession.openSettings`，功能面板的「词库」「反馈」「关于」图块）和应答方（`EntryAbility`，读取 Want 的 `page` 参数）共用这一份，两边不会走样。
 *
 * 这些 id 是 `packages/ui` 里 `settings-page-registry` 的 id，设置页把它作为初始页路由，或在打开时导航过去。其他值一律不认：Want 可能来自别的应用，设置页不该被引到没人打算开放的路由上。
 */
export class SettingsPageLink {
  /** 这些页面，不分先后。 */
  static readonly PAGES: string[] = ["dictionary", "feedback", "about"];

  /** `page` 是否是可链接的页面之一。 */
  static accepts(page: string): boolean {
    return SettingsPageLink.PAGES.includes(page);
  }

  /** 把 Want 的 `page` 参数转成可链接的页面 id；缺失、不是字符串或不在 `PAGES` 里时为 null。 */
  static fromParameter(value: Object | null | undefined): string | null {
    if (typeof value !== "string") {
      return null;
    }
    const page: string = value as string;
    return SettingsPageLink.accepts(page) ? page : null;
  }
}
