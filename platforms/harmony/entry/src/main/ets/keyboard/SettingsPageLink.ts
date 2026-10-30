/**
 * 键盘请求打开的一处设置：一个页面，可带一个页面里的子目标。
 */
export interface SettingsLinkRequest {
  /** `SettingsPageLink.PAGES` 之一。 */
  page: string;
  /** 页面里要直接打开的那一处，是这个页面在 `SettingsPageLink.targets` 里列出的值之一；没有时为 null，只打开页面。 */
  target: string | null;
}

/**
 * 键盘可以直接打开哪些设置页。请求方（`KeyboardSession.openSettings`，功能面板的「词库」「反馈」「关于」图块，皮肤选择器的「更多皮肤」，长按工具栏任一按钮打开的「屏幕键盘」、「添加语言」打开的「输入」，长按九键左侧符号栏打开的「按键布局」编辑页）和应答方（`EntryAbility`，读取 Want 的 `page` 与 `target` 参数）共用这一份，两边不会走样。
 *
 * 这些 id 是 `packages/ui` 里 `settings-page-registry` 的 id，设置页把它作为初始页路由，或在打开时导航过去。其他值一律不认：Want 可能来自别的应用，设置页不该被引到没人打算开放的路由上。
 *
 * `target` 是可选的第二个参数，只认 `targets(page)` 里列出的值；没有、不认得或不属于这个页面时丢掉它，照旧只打开页面，所以只带 `page` 的旧链接意思不变。
 */
export class SettingsPageLink {
  /** 这些页面，不分先后。 */
  static readonly PAGES: string[] = [
    "input",
    "dictionary",
    "feedback",
    "about",
    "skin",
    "screen-keyboard",
  ];

  /** 长按九键字母层左侧符号栏（拼音九键、九键双拼、笔画）：「按键布局」里字母表的编辑页。 */
  static readonly NINE_KEY_SIDEBAR_TARGET: string = "touch_nine_key_sidebar_symbols";
  /** 长按九宫格数字键盘的左侧符号栏：「按键布局」里数字表的编辑页。 */
  static readonly NINE_KEY_DIGIT_SIDEBAR_TARGET: string = "touch_nine_key_digit_sidebar_symbols";

  /**
   * 「屏幕键盘」页的子目标：九键左侧符号栏两张表的共享偏好名，打开「按键布局」子页并直接进入那张表的编辑页。与 Android 长按符号栏带的 `edit_sidebar_key`（`NineKeySidebarPolicy.EDIT_ARGUMENT`）取同样的值、同样只用于导航：编辑页确认前什么也不写。
   */
  static readonly SCREEN_KEYBOARD_TARGETS: string[] = [
    SettingsPageLink.NINE_KEY_SIDEBAR_TARGET,
    SettingsPageLink.NINE_KEY_DIGIT_SIDEBAR_TARGET,
  ];

  /** `page` 是否是可链接的页面之一。 */
  static accepts(page: string): boolean {
    return SettingsPageLink.PAGES.includes(page);
  }

  /** `page` 接受哪些子目标；没有子目标的页面是空表。 */
  static targets(page: string): string[] {
    return page === "screen-keyboard" ? SettingsPageLink.SCREEN_KEYBOARD_TARGETS : [];
  }

  /** `target` 是否是 `page` 的子目标之一。 */
  static acceptsTarget(page: string, target: string): boolean {
    return SettingsPageLink.targets(page).includes(target);
  }

  /** 把 Want 的 `page` 参数转成可链接的页面 id；缺失、不是字符串或不在 `PAGES` 里时为 null。 */
  static fromParameter(value: Object | null | undefined): string | null {
    if (typeof value !== "string") {
      return null;
    }
    const page: string = value as string;
    return SettingsPageLink.accepts(page) ? page : null;
  }

  /** 把 Want 的 `page` 与 `target` 参数转成一次请求；页面不可链接时为 null，子目标缺失、不是字符串或不属于这个页面时请求照旧，只是不带子目标。 */
  static fromParameters(
    page: Object | null | undefined,
    target: Object | null | undefined,
  ): SettingsLinkRequest | null {
    const linked: string | null = SettingsPageLink.fromParameter(page);
    if (linked === null) {
      return null;
    }
    const request: SettingsLinkRequest = {
      page: linked,
      target:
        typeof target === "string" && SettingsPageLink.acceptsTarget(linked, target as string)
          ? (target as string)
          : null,
    };
    return request;
  }
}
