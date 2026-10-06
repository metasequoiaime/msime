"""Local synthetic settings UI regression; never loads a native host or user data."""
import argparse
import json
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

parser = argparse.ArgumentParser()
parser.add_argument("--url", required=True)
parser.add_argument("--csp", required=True)
parser.add_argument("--executable")
parser.add_argument("--screenshot")
args = parser.parse_args()
assert args.url.startswith("http://127.0.0.1:"), "use a loopback fixture server"

def open_page(page, name):
    page.get_by_role("button", name=name, exact=True).click()

def choose_theme(page, title):
    # The global theme cards on 主题 are switches named after the theme.
    open_page(page, "主题")
    page.get_by_role("switch", name=title, exact=True).click()

def choose_segment(page, group, label):
    # A segmented control draws its label over a visually hidden radio, so click the segment the way a pointer would.
    segments = page.get_by_role("radiogroup", name=group, exact=True)
    radio = segments.get_by_role("radio", name=label, exact=True)
    segments.locator("label").filter(has=page.get_by_role("radio", name=label, exact=True)).click()
    expect(radio).to_be_checked()

def set_color_mode(page, label):
    open_page(page, "主题")
    choose_segment(page, "颜色模式", label)

def set_surface_theme(page, label, value):
    # The per-surface light/dark overrides live under 主题 › 高级.
    open_page(page, "主题")
    page.get_by_label(label, exact=True).select_option(value)

def set_layout(page, orientation):
    choose_segment(page, "候选项排列方式", {"horizontal": "横向", "vertical": "纵向"}[orientation])

def reload_settings(page):
    page.get_by_role("button", name="重新读取", exact=True).click()
    discard = page.get_by_role("alertdialog").get_by_role("button", name="放弃并重新读取", exact=True)
    if discard.count():
        discard.click()
    expect(page.get_by_role("alertdialog")).to_have_count(0)

def theme_card_previews(page):
    return page.locator("article [data-global-theme]")

def verify_font_sizes(page, preview):
    for orientation in ["horizontal", "vertical"]:
        set_layout(page, orientation)
        for size in range(12, 33):
            page.get_by_label("候选窗字号", exact=True).select_option(str(size))
            page.get_by_label("候选窗预编辑字号", exact=True).select_option(str(44 - size))
            expect(preview.locator("[data-preview-layout] .cand .text").first).to_have_css("font-size", f"{size}px")
            expect(preview.locator(".pinyin .text")).to_have_css("font-size", f"{44 - size}px")

def verify_helpcode_display(page, preview):
    # The 辅助码 group is part of the 输入 page; the fixture uses quanpin. The synthetic snapshot has no quanpin_helpcode, and the switch (core default: hidden) and the preview (shown) read that absence differently, so start from an explicit choice.
    open_page(page, "输入")
    display = page.get_by_role("switch", name="在候选窗口中显示全拼辅助码", exact=True)
    display.check()
    open_page(page, "候选窗口")
    count = preview.locator("[data-preview-layout] .cand").count()
    expect(preview.locator("[data-preview-layout] .cand-helpcode")).to_have_count(count)
    open_page(page, "输入")
    display.uncheck()
    open_page(page, "候选窗口")
    expect(preview.locator("[data-preview-layout] .cand-helpcode")).to_have_count(0)
    expect(preview.locator("[data-preview-layout] .cand")).to_have_count(count)
    open_page(page, "输入")
    display.check()
    open_page(page, "候选窗口")
    expect(preview.locator("[data-preview-layout] .cand-helpcode")).to_have_count(count)

def verify_text_color(page, preview):
    # The seven candidate colour pickers are the custom theme's, on 主题; the preview they drive is on 候选窗口.
    text = preview.locator("[data-preview-layout] .cand:not(.first) .text").first
    number = preview.locator("[data-preview-layout] .cand:not(.first) .num, [data-preview-layout] .cand:not(.first) .cand-no").first
    open_page(page, "候选窗口")
    original = text.evaluate("el => getComputedStyle(el).color")
    original_number = number.evaluate("el => getComputedStyle(el).color")
    open_page(page, "主题")
    picker = page.get_by_label("候选文字颜色", exact=True)
    reset = page.get_by_role("button", name="跟随主题", exact=True)
    picker.fill("#ab1234")
    expect(picker).to_have_css("width", "36px")
    expect(picker).to_have_css("height", "28px")
    expect(reset).to_have_attribute("aria-pressed", "false")
    # Using a picker selects the custom theme.
    expect(page.get_by_role("switch", name="自定义", exact=True)).to_have_attribute("aria-checked", "true")
    open_page(page, "候选窗口")
    expect(text).to_have_css("color", "rgb(171, 18, 52)")
    expect(number).to_have_css("color", "rgba(171, 18, 52, 0.616)")
    open_page(page, "主题")
    reset.click()
    expect(reset).to_have_attribute("aria-pressed", "true")
    open_page(page, "候选窗口")
    expect(text).to_have_css("color", original)
    expect(number).to_have_css("color", original_number)

def verify_font_families(page, preview):
    # The draft starts with the default fallback families; clear them so the synthetic fonts alone decide the fallback order.
    remove_first = page.get_by_role("button", name="移除补充字体 1", exact=True)
    while remove_first.count():
        remove_first.click()
    page.get_by_label("候选窗主字体", exact=True).fill("缺字示例")
    page.get_by_role("button", name="添加补充字体", exact=True).click()
    page.get_by_label("补充字体 1", exact=True).fill("示例字体")
    page.get_by_role("button", name="添加补充字体", exact=True).click()
    page.get_by_label("补充字体 2", exact=True).fill("加倍示例")
    def width():
        return preview.locator(".candidate[data-preview-layout]").evaluate("""el => {
          const ctx = document.createElement('canvas').getContext('2d');
          ctx.font = '20px ' + getComputedStyle(el).fontFamily;
          return ctx.measureText('A').width;
        }""")
    assert abs(width() - 20) < .01
    page.get_by_role("button", name="上移补充字体 2", exact=True).click()
    assert abs(width() - 40) < .01
    page.get_by_role("button", name="移除补充字体 1", exact=True).click()
    assert abs(width() - 20) < .01
    page.get_by_role("button", name="移除补充字体 1", exact=True).click()
    text = preview.locator("[data-preview-layout] .cand .text").first
    original_color = text.evaluate("el => getComputedStyle(el).color")
    page.get_by_label("候选窗主字体", exact=True).fill('示例";color:red;/*')
    family = preview.locator(".candidate[data-preview-layout]").evaluate("el => getComputedStyle(el).fontFamily")
    assert family.endswith("sans-serif") and "color:red;" in family
    expect(text).to_have_css("color", original_color)
    page.get_by_label("候选窗主字体", exact=True).fill("Segoe UI")

with sync_playwright() as playwright:
    browser = playwright.chromium.launch(headless=True, executable_path=args.executable)
    try:
        page = browser.new_page(viewport={"width": 1000, "height": 850})
        page.on("dialog", lambda dialog: dialog.accept())
        page.route(args.url + "/fixture", lambda route: route.fulfill(
            body='<html><head><link rel="stylesheet" href="/settings.css"></head><body><div id="root"></div></body></html>',
            content_type="text/html", headers={"Content-Security-Policy": args.csp}))
        page.goto(args.url + "/fixture")
        page.evaluate("""async base64 => {
          const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0)).buffer;
          window.fixtureFonts = [new FontFace('缺字示例', bytes, {unicodeRange:'U+0042'}), new FontFace('示例字体', bytes), new FontFace('加倍示例', bytes, {sizeAdjust:'200%'})];
          for (const face of window.fixtureFonts) { await face.load(); document.fonts.add(face); }
        }""", json.loads(Path(__file__).with_name("skin-font-fixture.json").read_text())["base64"])
        page.evaluate("async () => { const {mount} = await import('/settings.js'); window.removeFixture = mount(); }")
        preview = page.get_by_role("region", name="候选窗口预览")
        open_page(page, "屏幕键盘")
        keyboard = page.get_by_role("img", name="屏幕键盘完整布局预览")
        expect(keyboard.locator("[data-keyboard-key]")).to_have_count(61)
        keyboard_background = keyboard.locator(":scope > rect").first
        letter_key = keyboard.locator('[data-keyboard-key="a"] path').first
        letter_label = keyboard.locator('[data-keyboard-key="a"] text')
        # Under the default 跟随系统 theme the 屏幕键盘主题 override picks the light or dark system keyboard; a built-in theme draws its own catalog palette whatever the override says.
        for theme, look, background, fill, text in [
            ("light", "system", "rgb(209, 212, 219)", "rgb(255, 255, 255)", "rgb(0, 0, 0)"),
            ("dark", "system", "rgb(43, 43, 45)", "rgb(107, 107, 110)", "rgb(255, 255, 255)"),
            ("light", "夜青", "rgb(15, 27, 34)", "rgb(29, 51, 64)", "rgb(230, 241, 244)"),
        ]:
            set_surface_theme(page, "屏幕键盘主题", theme)
            if look != "system":
                choose_theme(page, look)
            open_page(page, "屏幕键盘")
            expect(keyboard).to_have_attribute("data-preview-theme", theme)
            expect(keyboard_background).to_have_css("fill", background)
            expect(letter_key).to_have_css("fill", fill)
            expect(letter_label).to_have_css("fill", text)
            for width in [800, 1000]:
                page.set_viewport_size({"width": width, "height": 850})
                assert keyboard.evaluate("""svg => {
                  const bounds = svg.getBoundingClientRect();
                  return [...svg.querySelectorAll('[data-keyboard-row]')].every(row => {
                    let right = bounds.left;
                    return [...row.querySelectorAll('[data-keyboard-key] > path')].every(key => {
                      const rect = key.getBoundingClientRect();
                      const valid = rect.left >= right - .1 && rect.right <= bounds.right + .1 && rect.bottom <= bounds.bottom + .1;
                      right = rect.right; return valid;
                    });
                  });
                }""")
        choose_theme(page, "跟随系统")
        set_surface_theme(page, "悬浮工具栏主题", "light")
        open_page(page, "悬浮工具栏")
        toolbar = page.get_by_label("悬浮工具栏预览", exact=True)
        expect(toolbar.locator(".status-bar")).to_be_visible()
        expect(toolbar.locator(".status-bar")).to_have_css("background-color", "rgb(255, 255, 255)")
        set_surface_theme(page, "悬浮工具栏主题", "dark")
        open_page(page, "悬浮工具栏")
        expect(toolbar.locator(".status-bar")).to_have_css("background-color", "rgb(26, 26, 26)")
        for scale in [75, 100, 125, 150]:
            page.get_by_label("工具栏缩放", exact=True).select_option(str(scale))
            for size in [16, 18, 20, 22, 24, 26, 28]:
                page.get_by_label("图标尺寸", exact=True).select_option(str(size))
                expect(toolbar.locator(".icon").first).to_have_css("width", f"{size * scale / 100:g}px")
        for key, label in [("fullwidth", "全角 / 半角"), ("punctuation", "中英文标点"), ("character_set", "简繁切换"), ("emoji", "表情与符号"), ("screen_keyboard", "屏幕键盘"), ("settings", "设置")]:
            toggle = page.get_by_role("checkbox", name=label, exact=True)
            toggle.uncheck()
            expect(toolbar.locator(f'[data-toolbar-item="{key}"]')).to_be_hidden()
            toggle.check()
            expect(toolbar.locator(f'[data-toolbar-item="{key}"]')).to_be_visible()
        expect(toolbar.locator('[data-toolbar-item="language"]')).to_be_visible()
        reload_settings(page)
        open_page(page, "候选窗口")
        expect(preview.locator("[data-preview-layout] .cand")).to_have_count(6)
        expect(preview.locator(".candidate[data-preview-layout]")).to_have_css("font-size", "18px")
        verify_helpcode_display(page, preview)
        set_color_mode(page, "浅色")
        expect(page.get_by_label("候选文字颜色", exact=True)).to_have_value("#1a1a1a")
        open_page(page, "候选窗口")
        expect(preview.locator(".appearance-candidate-preview")).to_have_attribute("data-preview-theme", "light")
        light_surface = preview.locator(".container").evaluate("el => getComputedStyle(el).backgroundColor")
        open_page(page, "主题")
        # Built-in themes are one fixed palette; only 跟随系统 and 自定义 follow the colour mode and offer a per-card light/dark preview.
        cards = page.locator("article:has([data-global-theme])")
        expect(cards).to_have_count(7)
        for card in cards.all():
            card_preview = card.locator("[data-global-theme]")
            theme_id = card_preview.get_attribute("data-global-theme")
            toggle = card.get_by_role("button", name="预览深色", exact=True)
            if theme_id in ("system", "custom"):
                expect(card_preview).to_have_attribute("data-preview-theme", "light")
                toggle.click()
                expect(card_preview).to_have_attribute("data-preview-theme", "dark")
            else:
                expect(card_preview).to_have_attribute("data-preview-theme", "dark" if theme_id in ("shuishan", "night", "ink") else "light")
                expect(toggle).to_have_count(0)
        set_surface_theme(page, "设置界面主题", "dark")
        open_page(page, "候选窗口")
        expect(preview.locator(".container")).to_have_css("background-color", light_surface)
        set_surface_theme(page, "候选窗口主题", "dark")
        open_page(page, "候选窗口")
        assert preview.locator(".container").evaluate("el => getComputedStyle(el).backgroundColor") != light_surface
        set_color_mode(page, "跟随系统")
        set_surface_theme(page, "候选窗口主题", "follow")
        page.emulate_media(color_scheme="light")
        expect(page.get_by_label("候选文字颜色", exact=True)).to_have_value("#1a1a1a")
        open_page(page, "候选窗口")
        expect(preview.locator(".container")).to_have_css("background-color", light_surface)
        page.emulate_media(color_scheme="dark")
        expect(preview.locator(".appearance-candidate-preview")).to_have_attribute("data-preview-theme", "dark")
        open_page(page, "主题")
        expect(page.get_by_label("候选文字颜色", exact=True)).to_have_value("#e9e8e8")
        set_color_mode(page, "深色")
        set_surface_theme(page, "设置界面主题", "follow")
        for card in theme_card_previews(page).all():
            if card.get_attribute("data-global-theme") in ("system", "custom"):
                expect(card).to_have_attribute("data-preview-theme", "dark")
        open_page(page, "候选窗口")
        primary = page.get_by_label("候选窗主字体", exact=True)
        primary.click()
        expect(page.get_by_role("option", name="示例字体", exact=True)).to_be_visible()
        primary.fill("加倍")
        expect(page.get_by_role("listbox", name="候选窗主字体可用字体").get_by_role("option")).to_have_count(1)
        primary.press("ArrowDown")
        primary.press("Enter")
        expect(primary).to_have_value("加倍示例")
        expect(primary).to_have_attribute("aria-expanded", "false")
        primary.fill("Segoe UI")
        primary.press("Escape")
        set_surface_theme(page, "设置界面主题", "light")
        open_page(page, "候选窗口")
        primary.click()
        expect(page.locator(".font-family-menu")).to_have_css("background-color", "rgb(255, 255, 255)")
        expect(page.locator(".font-family-menu")).to_have_css("color", "rgb(36, 36, 36)")
        primary.press("Escape")
        set_surface_theme(page, "设置界面主题", "follow")
        open_page(page, "候选窗口")
        expect(preview.locator(".pinyin")).to_have_css("font-size", "15px")
        verify_font_sizes(page, preview)
        verify_text_color(page, preview)
        verify_font_families(page, preview)
        set_layout(page, "horizontal")
        page.get_by_label("候选窗字号", exact=True).select_option("20")
        page.get_by_label("每页候选项数量", exact=True).fill("9")
        expect(preview.locator("[data-preview-layout] .cand")).to_have_count(9)
        expect(preview.locator(".wnd-h")).to_have_css("font-size", "20px")
        page.get_by_label("候选窗预编辑", exact=True).select_option("empty")
        expect(preview.locator(".pinyin")).to_be_hidden()
        expect(preview.locator(".container.preedit-hidden > .pinyin + .row-wrapper > .first")).to_have_count(1)
        # 墨 stands in for the removed built-in 微信绿 skin as the dark built-in palette under a custom text colour. The old white-selected-text assertion is gone: the preview draws no theme's selected_text slot (the old rule was 微信绿's own stylesheet).
        choose_theme(page, "墨")
        open_page(page, "候选窗口")
        expect(preview.locator(".appearance-candidate-preview")).to_have_attribute("data-global-theme", "ink")
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(26, 26, 26)")
        verify_text_color(page, preview)
        expect(preview.locator(".appearance-candidate-preview")).to_have_attribute("data-global-theme", "custom")
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(26, 26, 26)")
        set_layout(page, "vertical")
        expect(preview.locator(".wnd-v")).to_have_css("font-size", "20px")
        if args.screenshot:
            page.screenshot(path=args.screenshot)
        reload_settings(page)
        open_page(page, "候选窗口")
        expect(preview.locator("[data-preview-layout] .cand")).to_have_count(6)
        expect(preview.locator(".candidate[data-preview-layout]")).to_have_css("font-size", "18px")
        expect(preview.locator(".pinyin")).to_have_count(1)
        expect(preview.locator(".pinyin")).to_be_visible()
        # Hidden preedit keeps its row across layouts and page sizes (formerly exercised under the removed Willow green skin, whose gradient surface no longer exists).
        page.get_by_label("候选窗预编辑", exact=True).select_option("empty")
        expect(preview.locator(".pinyin")).to_be_hidden()
        expect(preview.locator(".container.preedit-hidden > .pinyin + .row-wrapper > .first")).to_have_count(1)
        set_layout(page, "horizontal")
        expect(preview.locator(".container.preedit-hidden > .pinyin + .row-wrapper > .first")).to_have_count(1)
        # 3 is the smallest page size the selector offers.
        page.get_by_label("每页候选项数量", exact=True).fill("3")
        expect(preview.locator("[data-preview-layout] .cand")).to_have_count(3)
        expect(preview.locator(".container.preedit-hidden > .pinyin + .row-wrapper > .first")).to_have_count(1)
        page.get_by_label("候选窗预编辑", exact=True).select_option("pinyin")
        expect(preview.locator(".pinyin")).to_be_visible()
        expect(preview.locator(".preedit-hidden")).to_have_count(0)
        page.get_by_label("每页候选项数量", exact=True).fill("6")
        set_layout(page, "vertical")
        open_page(page, "主题")
        page.get_by_role("button", name="刷新皮肤", exact=True).click()
        # An external package is part of the custom theme: choosing it selects 自定义.
        page.get_by_role("switch", name="Synthetic external", exact=True).click()
        expect(page.get_by_role("switch", name="自定义", exact=True)).to_have_attribute("aria-checked", "true")
        open_page(page, "候选窗口")
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(18, 52, 86)")
        expect(preview.locator(".containerParent")).to_have_css("padding-top", "24px")
        verify_helpcode_display(page, preview)
        set_surface_theme(page, "候选窗口主题", "light")
        open_page(page, "候选窗口")
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(171, 205, 239)")
        # The package's light palette declares no border or selected-bar switch, so the light preview keeps the base theme's (system) light defaults rather than borrowing the dark mode's #112233 and hidden bar: resolve() layers a package per mode.
        expect(preview.locator(".container")).to_have_css("border-top-color", "rgba(0, 0, 0, 0.12)")
        assert preview.locator(".first").evaluate("el => getComputedStyle(el, '::before').display") != "none"
        set_surface_theme(page, "候选窗口主题", "dark")
        open_page(page, "候选窗口")
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(18, 52, 86)")
        expect(preview.locator(".container")).to_have_css("border-top-color", "rgb(17, 34, 51)")
        assert preview.locator(".first").evaluate("el => getComputedStyle(el, '::before').display") == "none"
        expect(preview.locator("img.skin-decoration-image")).to_be_visible()
        assert preview.locator("img").evaluate("img => img.complete && img.naturalWidth > 0")
        page.get_by_role("button", name="刷新预览", exact=True).click()
        expect(preview.locator(".container")).to_have_css("background-color", "rgb(18, 52, 86)")
        set_layout(page, "horizontal")
        expect(preview.locator(".wnd-h .cand")).to_have_count(6)
        verify_font_sizes(page, preview)
        verify_text_color(page, preview)
        verify_font_families(page, preview)
        if args.screenshot:
            page.screenshot(path=args.screenshot)
        page.evaluate("window.removeFixture()")
        page.evaluate("() => {for (const face of window.fixtureFonts) document.fonts.delete(face); delete window.fixtureFonts;}")
        expect(page.locator("#root")).to_be_empty()
        assert page.evaluate("document.adoptedStyleSheets.length") == 0
        page.evaluate("async () => { const {mountKeyboard} = await import('/settings.js'); window.removeKeyboard = mountKeyboard(); }")
        # The panel now carries the Linux function and numpad rows (seven rows, 103 keys off macOS), so the key count and per-key weights come from the rendered buttons' inline flex-grow rather than a hard-coded Windows table; the intent is unchanged: every key's width is its weight's share of the row after the key gaps, and keys never overlap or overflow the row.
        expect(page.locator(".keyboard-key")).to_have_count(103)
        expect(page.locator("[data-keyboard-row]")).to_have_count(7)
        for width in [800, 1100]:
            page.set_viewport_size({"width": width, "height": 500})
            assert page.locator("[data-keyboard-layout-grid]").evaluate("""layout => {
              const gap = parseFloat(getComputedStyle(layout).getPropertyValue('--keyboard-key-gap'));
              return gap === 6 && [...layout.children].every(row => {
                const keys = [...row.children];
                const weights = keys.map(key => parseFloat(key.style.flexGrow));
                const bounds = row.getBoundingClientRect();
                const available = bounds.width - gap * (keys.length - 1);
                const total = weights.reduce((a,b) => a+b, 0);
                let right = bounds.left;
                return keys.every((key, i) => {
                  const rect = key.getBoundingClientRect();
                  const valid = weights[i] > 0 && Math.abs(rect.width - available * weights[i] / total) < .1 && rect.left >= right - .1 && rect.right <= bounds.right + .1;
                  right = rect.right; return valid;
                });
              });
            }""")
        # Spot-check the Windows width ratios on the rows that carry them.
        assert page.evaluate("""() => {
          const grow = name => [...document.querySelectorAll('.keyboard-key')].filter(key => key.textContent === name).map(key => key.style.flexGrow);
          return JSON.stringify([grow('Backspace'), grow('Tab'), grow('Caps Lock'), grow('Enter'), grow('Shift'), grow('Space'), grow('Ctrl'), grow('a')]) === JSON.stringify([['1.9'], ['1.5'], ['1.85'], ['2'], ['2.35', '2.15'], ['6.7'], ['1.25', '1.25'], ['1']]);
        }""")
        expect(page.get_by_role("button", name="Space", exact=True)).to_have_css("font-size", "12px")
        expect(page.get_by_role("button", name="a", exact=True)).to_have_css("font-size", "15px")
        for height in [300, 400, 500]:
            page.set_viewport_size({"width": 1100, "height": height})
            expect(page.locator("main[aria-label='屏幕键盘'] .native-panel-header")).to_have_css("height", "28px")
            assert page.locator("main[aria-label='屏幕键盘']").evaluate("""panel => {
              const rows = [...panel.querySelectorAll('[data-keyboard-row]')];
              const gap = 7;
              const expectedHeight = (innerHeight - 28 - 7 - gap * (rows.length - 1)) / rows.length;
              return rows.every((row, index) => {
                const expectedY = 28 + index * (expectedHeight + gap);
                return [...row.children].every(key => {
                  const rect = key.getBoundingClientRect();
                  return Math.abs(rect.top - expectedY) < .1 && Math.abs(rect.height - expectedHeight) < .1 && rect.bottom <= innerHeight - 7 + .1;
                });
              }) && document.documentElement.scrollHeight === innerHeight;
            }""")
        page.get_by_role("button", name="a", exact=True).click()
        expect(page.get_by_role("status")).to_contain_text("等待宿主注入能力")
        expect(page.locator("main[aria-label='屏幕键盘'] .native-panel-header")).to_have_css("height", "28px")
        page.evaluate("window.removeKeyboard()")
        # The default skin is now the 跟随系统 global theme, whose keyboard palette is the system one in theme-catalog.json; hover, active and pressed are mixes of the key colour toward the accent, so they are read from the panel's --kb-* tokens instead of being restated here.
        for theme, background in [("dark", "rgb(43, 43, 45)"), ("light", "rgb(209, 212, 219)")]:
            page.evaluate("async theme => { const {mountKeyboard} = await import('/settings.js'); window.removeKeyboard = mountKeyboard(theme); }", theme)
            panel = page.locator("main[aria-label='屏幕键盘']")
            expect(panel).to_have_css("background-color", background)
            tokens = panel.evaluate("""panel => {
              const probe = document.createElement('div'); panel.append(probe);
              const read = name => { probe.style.backgroundColor = `var(${name})`; return getComputedStyle(probe).backgroundColor; };
              const result = {idle: read('--kb-key-fill'), hover: read('--kb-hover'), active: read('--kb-active'), pressed: read('--kb-pressed')};
              probe.remove(); return result;
            }""")
            assert len(set(tokens.values())) == 4, tokens
            shift = page.get_by_role("button", name="Shift", exact=True).first
            page.mouse.move(0, 0)
            expect(shift).to_have_css("background-color", tokens["idle"])
            shift.hover()
            expect(shift).to_have_css("background-color", tokens["hover"])
            page.mouse.down()
            expect(shift).to_have_css("background-color", tokens["pressed"])
            page.mouse.up()
            expect(shift).to_have_attribute("aria-pressed", "true")
            expect(shift).to_have_css("background-color", tokens["active"])
            page.mouse.down()
            expect(shift).to_have_css("background-color", tokens["pressed"])
            page.mouse.up()
            expect(shift).to_have_attribute("aria-pressed", "false")
            expect(shift).to_have_css("background-color", tokens["hover"])
            page.mouse.move(0, 0)
            expect(shift).to_have_css("background-color", tokens["idle"])
            page.evaluate("window.removeKeyboard()")
        expect(page.locator("#root")).to_be_empty()
        print({"appearanceDraftPreview": True, "fontCatalogSearch": True, "fontFamilyFallbackOrder": True, "fullFontSizeRange": True, "independentPreeditFontSize": True, "textColorAndReset": True, "skinPalette": True, "reload": True, "hiddenPreedit": True, "externalPalette": True, "externalDecoration": True, "cleanup": True})
    finally:
        browser.close()
