package app.msime.android.home;

import app.msime.android.ClipboardExport;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.CandidatePreeditStylePolicy;

import app.msime.android.DimensionPolicy;
import app.msime.android.KeyboardFormFactorPolicy;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.ToolbarOrderPolicy;
import app.msime.android.KeyboardSpacingPolicy;
import app.msime.android.KeyboardHeightPolicy;
import app.msime.android.KeyboardTypographyPolicy;
import app.msime.android.NumberPolicy;

import app.msime.android.LayoutPolicy;

import android.content.Context;
import android.graphics.Color;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DrawablePolicy;
import app.msime.android.AppEdition;
import app.msime.android.ClipboardLayoutPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.NineKeySwipePolicy;
import app.msime.android.SchemePreferences;
import app.msime.android.ShuangpinKeyHintPolicy;
import app.msime.android.SplitKeyboardPolicy;
import app.msime.android.SwipeHintPolicy;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.TextTrackpadSettings;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONObject;

/**
 * 键盘页：布局（中文键盘 26 / 9 键、九键字母与数字键盘的左侧符号、双拼键位提示、键盘高度、按键间距、行间距、分离式键盘、竖屏也分离、浮动键盘、键盘底栏）、按键反馈（按键音、按键振动、按键弹出预览、按键动画）、手势（滑动输入符号及其方向、滑行输入、空格滑动移动光标、长按空格语音）、键盘工具栏（预览、显示方式、各按钮）、剪贴板（工具栏显示最近复制、排列）和子页「AI 润色与回复」。
 *
 * <p>键盘与本页读同一批存储：按键间距、行间距、竖屏分离、双拼键位提示和表情/剪贴板/皮肤三个工具栏按钮在共享偏好里（`touch_key_spacing_tenths`、`touch_row_spacing_tenths`、`touch_keyboard_split_portrait`、`touch_shuangpin_key_hints`、`touch_toolbar.*`）；键盘高度、分离式键盘主开关、浮动键盘、物理键盘时候选栏跟随光标、键盘底栏、底部留白、按键弹出预览、按键动画、三个手势、常用语/输入方式/浮动键盘三个工具栏按钮和「显示方式：隐藏」（整行不显示，候选条照常显示）只有 Android 用，在 {@link AndroidLocalSettings} 里。键盘高度按设计以 75–170 % 显示，存的是 dp（{@link KeyboardHeightPolicy#heightPercentToAdjustment}），本地没写过时沿用共享偏好里旧的 `touch_keyboard_height_adjustment`。按键音和按键振动是 Android 一直以来的本地开关（`KeyboardFeedbackStore`），键盘的功能面板改的也是它们。
 */
public class KeyboardOptionsPage extends ReloadingDetailPage {
    private final ClipboardExportActions clipboardExport = new ClipboardExportActions(this, this::reload);

    @Override public void onCreate(@Nullable android.os.Bundle saved) {
        super.onCreate(saved);
        clipboardExport.restore(saved);
    }

    @Override public void onSaveInstanceState(android.os.Bundle saved) {
        clipboardExport.save(saved);
        super.onSaveInstanceState(saved);
    }

    @Override protected void onContentDestroyed() { clipboardExport.destroyed(); }
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("布局", "反馈", "手势", "工具栏", "剪贴板");
    }

    private static final String[] ANIMATIONS = {"bounce", "ripple", "glow", "lift", "none"};
    private static final String[] ANIMATION_LABELS = {"弹起", "涟漪", "发光", "浮起", "无"};
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local,
                         KeyboardFeedbackStore.Settings feedback, KeyboardSkin skin, String exportDirectory,
                         JSONObject trackpadOptions) {}

    @Override protected void reload() {
        boolean dark = AppMode.dark(requireContext());
        HostTask.run(this, context -> {
            JSONObject preferences = KeyboardSheets.preferences(context);
            if (preferences == null) return null;
            return new State(preferences, AndroidLocalSettings.load(context), KeyboardFeedbackStore.load(context),
                HostStore.keyboardSkin(preferences, dark, HostStore.seed(context)), ClipboardExport.directoryName(context),
                HostStore.textTrackpadOptions());
        }, this::render);
    }

    private void render(@Nullable State state) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject preferences = state.preferences();
        AndroidLocalSettings.Snapshot settings = state.local();
        AppEdition edition = AppEdition.current();
        KeyboardScheme current = SchemePreferences.storedScheme(preferences, edition);

        GroupCard layout = GroupCard.add(sectionColumn(target, "布局"), "布局");
        layout.toggle("英文自动大写", "按输入框要求自动大写句首、单词首字母或全部字母；默认关闭，需要大写时按 Shift",
            settings.bool(AndroidLocalSettings.AUTO_CAPITALIZATION),
            checked -> saveLocalSetting(AndroidLocalSettings.AUTO_CAPITALIZATION, checked));
        layout.toggle("交换语言键与数字键", "语言键放到左侧，数字键移到原语言键位置；英文和外语键盘一起调整",
            settings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS),
            checked -> saveLocalSetting(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS, checked));
        KeyboardScheme[] pair = layoutPair(current);
        boolean pairOffered = pair != null;
        if (pair != null) for (KeyboardScheme choice : pair) pairOffered &= choice.offeredBy(edition);
        String note = pairOffered ? null
            : pair == null ? "当前方案只有一种键盘，在「输入」里换方案" : "本版本只有一种键盘";
        layout.nav("中文键盘", note, layoutLabel(current),
            pairOffered ? () -> pickLayout(current, pair) : null);
        String spellingPosition = CandidatePreeditStylePolicy.normalize(
            InputViewValuePolicy.textOr(preferences, "touch_spelling_position", null));
        layout.nav("拼音显示位置", "全拼、双拼、五笔和粤拼可以把正在输入的拼音放在输入框或候选栏",
            spellingPositionLabel(spellingPosition),
            () -> pickSpellingPosition(spellingPosition));
        // 本版本没有任何其他语言键盘（五笔版、拼音版）时不列这一项。
        if (offersOtherLanguage(edition)) {
            layout.toggle("中英键轮换其他语言",
                "中英键依次切换中文、英文和已添加的日语、韩语等键盘，再回到中文；没有添加其他语言时仍只切中英",
                settings.bool(AndroidLocalSettings.LANGUAGE_KEY_CYCLE),
                checked -> saveLocalSetting(AndroidLocalSettings.LANGUAGE_KEY_CYCLE, checked));
        }
        // 左侧符号栏只属于拼音九键和笔画键盘；本版本两者都没有（例如五笔版）时不列这一行，数字键面那一行只看拼音九键。
        boolean quanpinNineKey = KeyboardScheme.QUANPIN_NINE_KEY.offeredBy(edition);
        if (quanpinNineKey || KeyboardScheme.STROKE.offeredBy(edition)) {
            java.util.List<String> sidebar = NineKeySidebarPolicy.letterSymbols(
                settings.text(AndroidLocalSettings.NINE_KEY_SYMBOLS));
            layout.nav("九键左侧符号", "拼音九键和笔画键盘左侧可上下滑动的符号栏",
                NineKeySidebarPolicy.summary(sidebar, 4),
                () -> editSidebarSymbols("九键左侧符号", AndroidLocalSettings.NINE_KEY_SYMBOLS, sidebar));
        }
        if (quanpinNineKey) {
            java.util.List<String> digitSidebar = NineKeySidebarPolicy.digitSymbols(
                settings.text(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS));
            layout.nav("九键数字键盘左侧符号", "拼音九键按 123 切到数字键盘后左侧的符号栏",
                NineKeySidebarPolicy.summary(digitSidebar, 4),
                () -> editSidebarSymbols("九键数字键盘左侧符号", AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS, digitSidebar));
        }
        layout.nav("文本编辑按键顺序", "调整文本编辑面板两侧的七个功能键，方向键与换行位置固定",
            null, () -> new TextEditOrderSheet(this, preferences, this::reload).show());
        layout.nav("符号分类顺序", "调整符号面板底部分类的排列顺序",
            null, () -> new SymbolCategoryOrderSheet(this, settings, this::reload).show());
        // 只有双拼方案的 26 键画键位提示；本版本没有双拼时不列这一行。缺省为关，与键盘和 client-core 一致。
        if (KeyboardScheme.XIAOHE.offeredBy(edition)) {
            layout.toggle("双拼键位提示", "双拼方案的 26 键键盘在字母键底部显示这个键代表的声母和韵母；关闭后键面只留字母",
                preferences.optBoolean(ShuangpinKeyHintPolicy.PREFERENCE_KEY, false),
                checked -> savePreference(ShuangpinKeyHintPolicy.PREFERENCE_KEY, checked));
        }
        int height = KeyboardHeightPolicy.heightAdjustmentToPercent(
            settings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                ? settings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                : NumberPolicy.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
        layout.slider("键盘高度", KeyboardHeightPolicy.MIN_HEIGHT_PERCENT, KeyboardHeightPolicy.MAX_HEIGHT_PERCENT, 1, height,
            KeyboardHeightPolicy::displayPercent,
            percent -> saveLocalSetting(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT,
                KeyboardHeightPolicy.heightPercentToAdjustment(percent)));
        layout.slider("按键间距", KeyboardSpacingPolicy.MIN_KEY_SPACING_TENTHS, KeyboardSpacingPolicy.MAX_KEY_SPACING_TENTHS, 1,
            KeyboardSpacingPolicy.keySpacing(NumberPolicy.strictInt(preferences, "touch_key_spacing_tenths", -1)),
            KeyboardSpacingPolicy::display, value -> savePreference("touch_key_spacing_tenths", value));
        layout.slider("行间距", KeyboardSpacingPolicy.MIN_ROW_SPACING_TENTHS, KeyboardSpacingPolicy.MAX_ROW_SPACING_TENTHS, 1,
            KeyboardSpacingPolicy.rowSpacing(NumberPolicy.strictInt(preferences, "touch_row_spacing_tenths", -1)),
            KeyboardSpacingPolicy::display, value -> savePreference("touch_row_spacing_tenths", value));
        layout.toggle("数字行", "26 键和韩文键盘的字母上方多一行 1–0，键盘整体高出一行；手机横屏时放不下，不显示",
            settings.bool(AndroidLocalSettings.NUMBER_ROW),
            checked -> saveLocalSetting(AndroidLocalSettings.NUMBER_ROW, checked));
        boolean showLargeScreenOptions = KeyboardFormFactorPolicy.showLargeScreenOptions(
            requireContext().getResources().getConfiguration().smallestScreenWidthDp);
        if (showLargeScreenOptions) {
            layout.toggle("分离式键盘", "默认仅在大屏横屏时生效：26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入；右半边也有 G 和 V",
                settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD),
                checked -> saveLocalSetting(AndroidLocalSettings.SPLIT_KEYBOARD, checked));
            layout.toggle("竖屏也分离", "平板或折叠屏内屏竖放时也分离；回到窄外屏后自动使用完整键盘",
                preferences.optBoolean(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, false),
                checked -> savePreference(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, checked))
                .setEnabled(settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD));
        }
        layout.toggle("浮动键盘", "键盘缩小成一块面板悬在应用上面，按住顶部横条拖动位置；也可以在功能面板或工具栏按钮里切换",
            settings.bool(AndroidLocalSettings.FLOATING_KEYBOARD),
            checked -> saveLocalSetting(AndroidLocalSettings.FLOATING_KEYBOARD, checked));
        if (showLargeScreenOptions) {
            layout.toggle("物理键盘时候选栏跟随光标", "接实体键盘打字时，候选栏显示在输入光标旁，靠近屏幕底部时显示在光标上方；不打字时不显示工具栏。应用不提供光标位置时仍停在屏幕底部",
                settings.bool(AndroidLocalSettings.HARDWARE_CARET_CANDIDATES),
                checked -> saveLocalSetting(AndroidLocalSettings.HARDWARE_CARET_CANDIDATES, checked));
        }
        layout.toggle("键盘底栏", "仅在全面屏手势导航下生效：键盘下方加一条栏，左边切换输入法、右边剪贴板，中间左右滑动移动光标，同时把按键抬离屏幕底边",
            settings.bool(AndroidLocalSettings.BOTTOM_BAR),
            checked -> saveLocalSetting(AndroidLocalSettings.BOTTOM_BAR, checked));

        GroupCard feedback = GroupCard.add(sectionColumn(target, "反馈"), "按键反馈");
        KeyboardFeedbackStore.Settings local = state.feedback();
        feedback.toggle("按键音", null, local.soundEnabled(), checked -> saveFeedback(checked, null));
        feedback.toggle("按键振动", null, local.hapticsEnabled(), checked -> saveFeedback(null, checked));
        feedback.toggle("按键弹出预览", "按下时在键上方显示放大的字符",
            settings.bool(AndroidLocalSettings.KEY_POPUP), checked -> saveLocalSetting(AndroidLocalSettings.KEY_POPUP, checked));
        String animation = settings.choice(AndroidLocalSettings.KEY_ANIMATION);
        GroupCard.Row[] animationRow = new GroupCard.Row[1];
        animationRow[0] = feedback.nav("按键动画", null, animationLabel(animation),
            () -> pickAnimation(animationRow[0]));

        GroupCard gestures = GroupCard.add(sectionColumn(target, "手势"), "手势");
        boolean swipeSymbols = settings.bool(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS);
        gestures.toggle("滑动输入符号", "沿设定方向滑动输入角标，中文时反方向打开字母与符号选项；关闭后仍可长按", swipeSymbols,
            checked -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.SWIPE_DOWN_SYMBOLS, checked,
                this::reload, this::reload));
        String swipeDirection = settings.choice(AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION);
        GroupCard.Row[] directionRow = new GroupCard.Row[1];
        directionRow[0] = gestures.nav("滑动方向", "该方向直接输入角标，中文时反方向打开选项；关闭滑动输入符号后两种滑动都不生效",
            swipeDirectionLabel(swipeDirection), () -> pickSwipeDirection(swipeDirection, directionRow[0]));
        directionRow[0].setEnabled(swipeSymbols);
        // 九键的滑动单独一项、默认关闭（#5580）：九键原来没有滑动，和 26 键共用上面那个默认开着的开关会改变老用户的点按。
        if (quanpinNineKey) {
            String nineKeySwipe = settings.choice(AndroidLocalSettings.NINE_KEY_SWIPE);
            GroupCard.Row[] nineKeySwipeRow = new GroupCard.Row[1];
            nineKeySwipeRow[0] = gestures.nav("九键滑动输入数字", "拼音九键上沿选定方向滑动输入键上的数字，往反方向滑动弹出数字和字母",
                nineKeySwipeLabel(nineKeySwipe), () -> pickNineKeySwipe(nineKeySwipe, nineKeySwipeRow[0]));
            // 九键单独的判定距离（#6208）：键高，快速连打时手指往上带一下就会被当成上滑；26 键的「滑动输入符号」阈值不受影响。
            GroupCard.Row distanceRow = gestures.slider("九键滑动距离",
                "手指移动超过这个距离才会触发；距离越大越不容易把快速连打误判成滑动",
                NineKeySwipePolicy.MIN_THRESHOLD_DP, NineKeySwipePolicy.MAX_THRESHOLD_DP,
                NineKeySwipePolicy.THRESHOLD_STEP_DP, settings.integer(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE),
                value -> value + " dp", value -> saveLocalSetting(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE, value));
            distanceRow.setEnabled(!NineKeySwipePolicy.OFF.equals(nineKeySwipe));
        }
        gestures.toggle("滑行输入", "在 26 键上连续滑过拼音的字母，抬手出词；在键上稍作停留可确认经过的键",
            settings.bool(AndroidLocalSettings.GLIDE_TYPING),
            checked -> saveLocalSetting(AndroidLocalSettings.GLIDE_TYPING, checked));
        gestures.toggle("空格键滑动移动光标", null, settings.bool(AndroidLocalSettings.SPACE_CURSOR),
            checked -> saveLocalSetting(AndroidLocalSettings.SPACE_CURSOR, checked));
        gestures.toggle("长按空格语音输入", null, settings.bool(AndroidLocalSettings.SPACE_VOICE),
            checked -> saveLocalSetting(AndroidLocalSettings.SPACE_VOICE, checked));
        gestures.note("退格键左滑可连续删除，保持按住再右滑可逐步恢复，松手后无法恢复；密码、无痕和禁止学习的输入框不支持此手势。上滑仍可快速删除，有备份时长按下滑可撤回。");
        gestures.note("空格键上滑后松手可复制当前输入框的全文，滑回可取消；复制预览遵守下方剪贴板设置。长按语音开始后，上滑仍用于取消语音。");
        renderTrackpad(sectionColumn(target, "手势"), preferences, state.trackpadOptions());

        JSONObject toolbar = preferences.optJSONObject("touch_toolbar");
        if (toolbar == null) toolbar = new JSONObject();
        boolean hidden = settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN);
        GroupCard bar = GroupCard.add(sectionColumn(target, "工具栏"), "键盘工具栏");
        bar.addView(toolbarPreview(requireContext(), toolbar, settings, state.skin()));
        bar.nav("显示方式", null, hidden ? "隐藏" : "输入时显示", () -> pickToolbarMode(hidden));
        bar.nav("按钮顺序", "调整工具按钮的排列；品牌和收起键保留在两端", null,
            hidden ? null : () -> new ToolbarOrderSheet(this, settings, this::reload).show());
        for (ToolbarOrderPolicy.Item button : toolbarOrder(settings)) {
            String member = button.preferenceKey;
            String description = button == ToolbarOrderPolicy.Item.UNDO || button == ToolbarOrderPolicy.Item.REDO
                ? "由当前应用处理；部分输入框不支持，密码框不可用" : null;
            GroupCard.Row row = bar.toggle(button.label, description, toolbarButton(toolbar, settings, member),
                checked -> saveToolbar(member, checked));
            row.setEnabled(!hidden);
        }

        GroupCard clipboard = GroupCard.add(sectionColumn(target, "剪贴板"), "剪贴板");
        clipboard.headerAction(clipboardExport.button());
        clipboard.nav("导出文件夹", "全文保存为 TXT；自选目录只在本机生效", state.exportDirectory(), clipboardExport::choose);
        // 剪贴板历史关着时键盘不显示最近复制，也不为它读剪贴板（RecentClipboardSuggestion.enabled），这一项跟着置灰。
        boolean clipboardHistory = preferences.optBoolean("clipboard_history", false);
        GroupCard.Row recentClipRow = clipboard.toggle("工具栏显示最近复制",
            "复制文字或图片后一分钟内，键盘顶部显示这条内容：文字点按即可粘贴，图片在支持收图片的输入框里点按即可发送；需先在「隐私」里打开剪贴板历史。隐私模式、密码框和系统标为敏感的内容不显示",
            settings.bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION),
            checked -> saveLocalSetting(AndroidLocalSettings.CLIPBOARD_SUGGESTION, checked));
        recentClipRow.setEnabled(clipboardHistory);
        String columns = settings.choice(AndroidLocalSettings.CLIPBOARD_COLUMNS);
        clipboard.nav("排列", "剪贴板面板里一行显示几条记录", clipboardColumnsLabel(columns),
            () -> pickClipboardColumns(columns));
        clipboard.nav("剪贴板分类", "自定义分类名称，在键盘中手动移动记录", "管理",
            () -> SettingsNavigator.open(requireContext(), PageId.CLIPBOARD_CATEGORIES, null));
        int historyLimit = ClipboardHistoryPolicy.limit(preferences);
        clipboard.nav("历史记录上限", "达到上限后，最旧的未固定记录会被新记录替换",
            historyLimit + " 条", () -> pickClipboardLimit(historyLimit), true);

    }

    private static java.util.List<ToolbarOrderPolicy.Item> toolbarOrder(AndroidLocalSettings.Snapshot settings) {
        return ToolbarOrderPolicy.ordered(settings.text(AndroidLocalSettings.TOOLBAR_ORDER));
    }

    /** 工具栏按钮是否开启：本地项读取本地设置，其余读取 `touch_toolbar`；撤销、重做缺键为关。 */
    private static boolean toolbarButton(JSONObject toolbar, AndroidLocalSettings.Snapshot settings, String member) {
        if (AndroidLocalSettings.specs().containsKey(member)) return settings.bool(member);
        return toolbar.optBoolean(member, !member.equals("undo") && !member.equals("redo"));
    }

    /** 工具栏预览：用当前皮肤的底色和图标色，按开关列出会出现的按钮；隐藏时说明只剩候选条。 */
    private static View toolbarPreview(Context context, JSONObject toolbar, AndroidLocalSettings.Snapshot settings,
                                       KeyboardSkin skin) {
        LinearLayout strip = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(strip);
        int pad = DimensionPolicy.pixels(context, 12);
        ViewPolicy.setSymmetricPadding(strip, pad);
        LinearLayout plate = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(plate);
        ViewPolicy.setHorizontalPaddingDp(context, plate, 10);
        ViewPolicy.setBackground(plate, DrawablePolicy.rounded(ColorPolicy.parse(skin.background(), ThemeColorPolicy.page(context)), DimensionPolicy.pixels(context, 12)));
        ViewPolicy.setImportantForAccessibility(plate,
            View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        int icon = ColorPolicy.parse(skin.toolbarIcon(), ThemeColorPolicy.subText(context));
        boolean anyButton = false;
        for (ToolbarOrderPolicy.Item button : ToolbarOrderPolicy.DEFAULT_ORDER)
            anyButton |= toolbarButton(toolbar, settings, button.preferenceKey);
        if (settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN)) {
            TextView note = ViewPolicy.styledLabel(context, "工具栏已隐藏，只显示候选条", 13, 400, icon);
            plate.addView(note);
        } else if (!anyButton) {
            // 键盘在工具按钮全关时不画这一行（MSIMEInputService.toolbarRowHidden），预览跟着说明。
            TextView note = ViewPolicy.styledLabel(context, "没有打开的工具按钮，工具栏不显示，只显示候选条", 13, 400, icon);
            plate.addView(note);
        } else {
            addChip(context, plate, "水杉", ColorPolicy.parse(skin.accentText(), ThemeColorPolicy.accent(context)));
            for (ToolbarOrderPolicy.Item button : toolbarOrder(settings)) {
                if (toolbarButton(toolbar, settings, button.preferenceKey)) addChip(context, plate, button.label, icon);
            }
        }
        strip.addView(plate, LayoutPolicy.matchWidthHeightDp(context, 44));
        strip.setContentDescription("工具栏预览");
        return strip;
    }

    private static void addChip(Context context, LinearLayout plate, String label, int colour) {
        TextView chip = ViewPolicy.styledLabel(context, label, 12, 500, colour);
        ViewPolicy.setSingleLine(chip);
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.setMarginEnd(DimensionPolicy.pixels(context, 12));
        plate.addView(chip, params);
    }

    /**
     * 当前方案可选的几种排法，按 26 键、14 键、9 键排列：全拼是全拼 26 键、全拼 14 键和全拼 9 键（三选一，选中即启用并切换），注音是大千和注音 9 键，日语是日语 26 键和日语 9 键（14 键只给全拼）。双拼、五笔、手写这类只有一种排法的方案没有这一组，返回 null：原来一律给全拼的那一对，小鹤双拼用户在这里点哪一项都会被改成全拼，注音 9 键用户点「26 键」会切到全拼而不是大千。
     */
    @Nullable
    private static boolean offersOtherLanguage(AppEdition edition) {
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            if (scheme.otherLanguage() && scheme.offeredBy(edition)) return true;
        }
        return false;
    }

    private static KeyboardScheme[] layoutPair(KeyboardScheme current) {
        return switch (current) {
            case QUANPIN, QUANPIN_FOURTEEN_KEY, QUANPIN_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.QUANPIN,
                KeyboardScheme.QUANPIN_FOURTEEN_KEY, KeyboardScheme.QUANPIN_NINE_KEY};
            case ZHUYIN, ZHUYIN_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY};
            case JAPANESE, JAPANESE_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.JAPANESE, KeyboardScheme.JAPANESE_NINE_KEY};
            default -> null;
        };
    }

    private void pickLayout(KeyboardScheme current, KeyboardScheme[] choices) {
        OptionSheet sheet = new OptionSheet(requireContext(), "中文键盘", null);
        // 点已选中的那一项什么也不做：OptionSheet 对选中项也会执行动作，重写一遍方案没有意义。
        for (KeyboardScheme choice : choices)
            sheet.option(layoutLabel(choice), choice == current, () -> { if (choice != current) applyScheme(choice); });
        sheet.show();
    }

    /** 「中文键盘」一行和选项上的排法名：按入口存下的触屏布局，`nine_key` 是「9 键」，`fourteen_key` 是「14 键」，其余是「26 键」。 */
    private static String layoutLabel(KeyboardScheme scheme) {
        return switch (scheme.touchKeyboardLayout()) {
            case "nine_key" -> "9 键";
            case "fourteen_key" -> "14 键";
            default -> "26 键";
        };
    }

    private static String spellingPositionLabel(String value) {
        return CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(value) ? "候选栏" : "输入框";
    }

    private void pickSpellingPosition(String current) {
        boolean candidateBar = CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(current);
        OptionSheet sheet = new OptionSheet(requireContext(), "拼音显示位置", null);
        sheet.option("输入框", !candidateBar,
            () -> savePreference("touch_spelling_position", CandidatePreeditStylePolicy.EDITOR));
        sheet.option("候选栏", candidateBar,
            () -> savePreference("touch_spelling_position", CandidatePreeditStylePolicy.CANDIDATE_BAR));
        sheet.show();
    }

    /** 26 / 14 / 9 键是当前方案的那几个触屏方案（{@link #layoutPair}）；与输入页、引导页一样经 {@link SchemePreferences#withScheme} 一起写那几个键。 */
    private void applyScheme(KeyboardScheme scheme) {
        HostTask.run(this, context -> {
            JSONObject snapshot = HostStore.loadPreferences(context);
            JSONObject pending = SchemePreferences.withScheme(snapshot, scheme, null);
            return pending == null ? null : HostStore.savePreferences(context, pending);
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "保存失败，键盘保留当前布局");
            reload();
        });
    }

    /** 编辑左侧符号栏的符号：用空格分开，留空恢复默认；不合规（单个符号太长、太多）时确认键不可点。 */
    private void editSidebarSymbols(String title, String key, java.util.List<String> current) {
        java.util.List<String> defaults = key.equals(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS)
            ? NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS : NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        new NineKeySidebarSheet(this, title, key, current, defaults, this::reload).show();
    }

    private void pickAnimation(GroupCard.Row row) {
        HostTask.run(this, context -> AndroidLocalSettings.load(context).choice(AndroidLocalSettings.KEY_ANIMATION), selected -> {
            if (selected == null) {
                MsToast.show(requireContext(), "读取设置失败，请重试");
                return;
            }
            OptionSheet sheet = new OptionSheet(requireContext(), "按键动画", null);
            for (int index = 0; index < ANIMATIONS.length; index++) {
                String value = ANIMATIONS[index];
                String label = ANIMATION_LABELS[index];
                sheet.option(label, value.equals(selected), () -> {
                    row.setValue(label);
                    saveLocalSetting(AndroidLocalSettings.KEY_ANIMATION, value);
                });
            }
            sheet.show();
        });
    }

    private void pickSwipeDirection(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "滑动方向", null);
        for (String value : new String[] {SwipeHintPolicy.DOWN, SwipeHintPolicy.UP}) {
            sheet.option(swipeDirectionLabel(value), value.equals(selected),
                () -> {
                    row.setValue(swipeDirectionLabel(value));
                    KeyboardSheets.saveLocal(this, AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION, value,
                        this::reload, this::reload);
                });
        }
        sheet.show();
    }

    private void pickClipboardColumns(String selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "剪贴板排列", null);
        for (String value : new String[] {ClipboardLayoutPolicy.ONE_COLUMN, ClipboardLayoutPolicy.TWO_COLUMNS}) {
            sheet.option(clipboardColumnsLabel(value), value.equals(selected),
                () -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.CLIPBOARD_COLUMNS, value,
                    this::reload, this::reload));
        }
        sheet.show();
    }

    private void pickClipboardLimit(int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "历史记录上限", null);
        for (int limit : ClipboardHistoryPolicy.LIMITS) {
            sheet.option(limit + " 条", limit == selected,
                () -> savePreference("clipboard_history_limit", limit));
        }
        sheet.show();
    }

    private static String clipboardColumnsLabel(String value) {
        return ClipboardLayoutPolicy.columns(value) == 2 ? "双列" : "单列";
    }

    private void pickNineKeySwipe(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "九键滑动输入数字", null);
        for (String value : new String[] {NineKeySwipePolicy.OFF, SwipeHintPolicy.UP, SwipeHintPolicy.DOWN}) {
            String label = nineKeySwipeOption(value);
            sheet.option(label, value.equals(selected), () -> {
                row.setValue(nineKeySwipeLabel(value));
                // 存好后重画，下次打开选项单时勾选的是新值。
                KeyboardSheets.saveLocal(this, AndroidLocalSettings.NINE_KEY_SWIPE, value, this::reload, this::reload);
            });
        }
        sheet.show();
    }

    private static String nineKeySwipeLabel(String value) {
        return SwipeHintPolicy.UP.equals(value) ? "上滑" : SwipeHintPolicy.DOWN.equals(value) ? "下滑" : "关闭";
    }

    private static String nineKeySwipeOption(String value) {
        return SwipeHintPolicy.UP.equals(value) ? "上滑输入数字，下滑弹出字母"
            : SwipeHintPolicy.DOWN.equals(value) ? "下滑输入数字，上滑弹出字母" : "关闭";
    }

    private static String swipeDirectionLabel(String value) {
        return SwipeHintPolicy.UP.equals(value) ? "上滑" : "下滑";
    }

    private void pickToolbarMode(boolean hidden) {
        OptionSheet sheet = new OptionSheet(requireContext(), "显示方式", null);
        sheet.option("输入时显示", !hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, false));
        sheet.option("隐藏", hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, true));
        sheet.show();
    }

    private static String animationLabel(String value) {
        for (int index = 0; index < ANIMATIONS.length; index++) {
            if (ANIMATIONS[index].equals(value)) return ANIMATION_LABELS[index];
        }
        return ANIMATION_LABELS[ANIMATION_LABELS.length - 1];
    }

    private void savePreference(String key, Object value) {
        savePreferences(preferences -> preferences.put(key, value));
    }

    private void renderTrackpad(LinearLayout column, JSONObject preferences, JSONObject options) {
        GroupCard group = GroupCard.add(column, "文本编辑触控板");
        TextTrackpadSettings value = TextTrackpadSettings.from(preferences);
        JSONObject horizontal = options == null ? null : options.optJSONObject("horizontal_dp");
        JSONObject vertical = options == null ? null : options.optJSONObject("vertical_dp");
        if (value == null || horizontal == null || vertical == null) {
            group.note("触控板设置尚未就绪，请返回后重试。");
            return;
        }
        group.toggle("优先使用触控板", "打开文本编辑时显示触控板，可在面板中切回方向键",
            value.enabled(), checked -> saveTrackpad("enabled", checked));
        group.toggle("按住工具临时打开", "按住工具栏的文本编辑按钮打开触控板，松手返回之前的面板",
            value.holdToOpen(), checked -> saveTrackpad("hold_to_open", checked));
        group.slider("水平移动距离", "距离越小，左右移动字符越快",
            NumberPolicy.strictInt(horizontal, "minimum", -1), NumberPolicy.strictInt(horizontal, "maximum", -1),
            1, value.horizontalDp(), number -> number + " dp", number -> saveTrackpad("horizontal_dp", number));
        group.slider("垂直移动距离", "距离越小，上下移动行越快",
            NumberPolicy.strictInt(vertical, "minimum", -1), NumberPolicy.strictInt(vertical, "maximum", -1),
            1, value.verticalDp(), number -> number + " dp", number -> saveTrackpad("vertical_dp", number));
        group.toggle("双击选词与三击选行", "编辑器提供足够文字时生效；长按后拖动也可延伸选区",
            value.tapSelection(), checked -> saveTrackpad("tap_selection", checked));
        group.toggle("逐步振动", "跟随键盘振动和系统触感开关", value.haptics(),
            checked -> saveTrackpad("haptics", checked));
        group.toggle("显示移动轨迹", null, value.trail(), checked -> saveTrackpad("trail", checked));
        group.toggle("光标文字放大镜", "显示光标附近文字；密码使用掩码，编辑器不提供文字时显示提示",
            value.magnifier(), checked -> saveTrackpad("magnifier", checked));
    }

    private void saveTrackpad(String key, Object value) {
        savePreferences(preferences -> KeyboardSheets.child(preferences, "android_text_trackpad").put(key, value));
    }

    /** 改一个工具栏开关（`touch_toolbar` 的成员或本地设置），写完重画预览（显示方式还会改变按钮行的可用状态）。 */
    private void saveToolbar(String member, boolean value) {
        if (AndroidLocalSettings.specs().containsKey(member)) {
            KeyboardSheets.saveLocal(this, member, value, this::reload, this::reload);
            return;
        }
        KeyboardSheets.save(this, preferences -> KeyboardSheets.child(preferences, "touch_toolbar").put(member, value),
            this::reload, this::reload);
    }

    /** 本地的按键音 / 按键振动；传 null 的那一项保持不变。 */
    private void saveFeedback(@Nullable Boolean sound, @Nullable Boolean haptics) {
        HostTask.run(this, context -> {
            KeyboardFeedbackStore.Settings current = KeyboardFeedbackStore.load(context);
            KeyboardFeedbackPreferences.HapticStrength strength = current.hapticStrength();
            try {
                KeyboardFeedbackStore.save(context, new KeyboardFeedbackStore.Settings(
                    sound == null ? current.soundEnabled() : sound,
                    haptics == null ? current.hapticsEnabled() : haptics, strength));
                // 按键音和振动随「设置」同步；不标记的话这次改动传不上去，别的设备一改设置，下载时还会把它盖回去。
                SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                return Boolean.TRUE;
            } catch (java.io.IOException error) {
                return null;
            }
        }, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }
}
