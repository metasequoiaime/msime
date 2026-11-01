package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import app.msime.android.AppEdition;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyLayoutStyleMigration;
import app.msime.android.KeyboardScheme;
import app.msime.android.LayoutPolicy;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.SchemePreferences;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/**
 * 「按键布局」子页（键盘页「布局」组进来），自上而下：键盘类型分段控件（26 键 | 14/18 键 | 9 键，只列本版本提供的类型，只有一种时不画），这一类键盘所选样式下的整块键盘预览（{@link KeyLayoutPreview}，与标准不同的键描强调色），一排样式胶囊，当前样式的副作用说明（没有就不画），说明下面一张卡片里是开关：26 键分段先是「26 键「符」与「123」互换」，每个分段都有「中/英键放在空格左边」，9 键分段时再加一行「九键左侧符号」；接着是「数字键盘」一组空格位置的胶囊（空格在右列、在 0 右边、在 0 左边）、组末说明和一行「数字键盘左侧符号」；最后是没有标题的一组里的「顶部数字行」开关（本版本既没有 26 键也没有 14/18 键时不列）。各项都写共享偏好（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_twenty_six_symbol_first`、`touch_language_key_side`、`touch_nine_key_sidebar_symbols`、`touch_number_pad_space`、`touch_nine_key_digit_sidebar_symbols`、`touch_number_row`，{@link TouchKeyStyle}、{@link NineKeySidebarPolicy}），随账号同步，键盘每秒重读偏好时就地换底行和符号栏，不打断正在组的字。本版本三类都没有时键盘页不列入口（{@link TouchKeyStyle.Groups}）。哪一类、叫什么、高亮哪些键、说明写什么都在 {@link KeyLayoutStylesPolicy} 里。
 *
 * <p>分段控件默认停在用户当前在用的中文键盘类型；切分段只换预览、胶囊和互换开关的显隐，不写任何偏好；「中/英键放在空格左边」三个分段是同一项偏好，切分段时开关状态不变。点胶囊或开关立刻换预览再写偏好，保存失败时提示并重新读一遍，拨回磁盘上的值。
 *
 * <p>两行左侧符号点开是同一个编辑页（{@link NineKeySidebarSheet}），确认后只写那一张表；字母表是自定义的时 9 键预览的左列画用户的表并整条描强调色。键盘上长按符号栏带着 {@link NineKeySidebarPolicy#EDIT_ARGUMENT} 打开这一页，读到偏好后直接打开那张表的编辑页（字母表同时切到 9 键分段）。
 *
 * <p>页面打开时先把本机旧的「长空格布局」「数字行」和两张九键左侧符号表迁进共享偏好（{@link KeyLayoutStyleMigration}），显示的就是迁移后的选择。
 */
public class KeyLayoutStylesPage extends ReloadingDetailPage {
    // 「顶部数字行」的说明，iOS、鸿蒙写的是同样的文字。
    static final String NUMBER_ROW_NOTE = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。";

    /** 分段控件停在哪一类；第一次读到偏好时按当前键盘定，之后保存后重读时保持用户切到的那一类。 */
    @Nullable private KeyLayoutStylesPolicy.Kind kind;
    private TouchKeyStyle.Styles styles = TouchKeyStyle.Styles.DEFAULT;
    private boolean numberRow;
    private boolean symbolFirst;
    private boolean languageLeft;
    private TouchKeyStyle.NumberPadSpace numberPadSpace = TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN;
    private boolean eighteen;
    @Nullable private KeyLayoutPreview preview;
    @Nullable private LinearLayout chips;
    @Nullable private TextView note;
    @Nullable private GroupCard.Row symbolFirstRow;
    @Nullable private LinearLayout numberPadChips;
    /** 两张左侧符号表（共享偏好里存的，空表表示出厂表）。 */
    private List<String> nineKeySidebar = List.of();
    private List<String> digitSidebar = List.of();
    @Nullable private View nineKeySidebarGroup;
    /** 键盘长按符号栏带来的那张表的共享偏好名；打开过编辑页后清掉，重读时不再弹出。 */
    @Nullable private String pendingSidebarEdit;

    @Override public void onCreate(@Nullable android.os.Bundle saved) {
        super.onCreate(saved);
        android.os.Bundle args = getArguments();
        String edit = saved != null || args == null ? null : args.getString(NineKeySidebarPolicy.EDIT_ARGUMENT);
        if (NineKeySidebarPolicy.LETTER_PREFERENCE.equals(edit) || NineKeySidebarPolicy.DIGIT_PREFERENCE.equals(edit))
            pendingSidebarEdit = edit;
    }

    @Override protected void reload() {
        HostTask.run(this, context -> {
            KeyLayoutStyleMigration.migrate(context, HostStore.directory(context));
            return KeyboardSheets.preferences(context);
        }, this::render);
    }

    private void render(@Nullable JSONObject preferences) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        preview = null;
        chips = null;
        note = null;
        symbolFirstRow = null;
        numberPadChips = null;
        nineKeySidebarGroup = null;
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();
        AppEdition edition = AppEdition.current();
        styles = TouchKeyStyle.Styles.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.FOURTEEN_KEY_PREFERENCE)),
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NINE_KEY_PREFERENCE)));
        numberRow = preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false);
        symbolFirst = preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false);
        languageLeft = TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE));
        // 没选过时选中 Android 原来的「空格在右列」，点哪一枚写哪一枚的写法。
        numberPadSpace = TouchKeyStyle.NumberPadSpace.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE))).resolved();
        nineKeySidebar = NineKeySidebarPolicy.fromPreference(preferences.opt(NineKeySidebarPolicy.LETTER_PREFERENCE));
        digitSidebar = NineKeySidebarPolicy.fromPreference(preferences.opt(NineKeySidebarPolicy.DIGIT_PREFERENCE));
        // 当前中文键盘与键盘页「中文键盘」那一行读的是同一批偏好：入口、双拼的排法和五笔的排法。
        KeyboardScheme current = SchemePreferences.storedScheme(preferences, edition);
        String shuangpinLayout = InputViewValuePolicy.textOr(preferences, "touch_shuangpin_layout", "twenty_six_key");
        String wubiLayout = InputViewValuePolicy.textOr(preferences, "touch_wubi_layout", "twenty_six_key");
        eighteen = KeyLayoutStylesPolicy.eighteen(current, shuangpinLayout);
        // 本版本提供哪几类由 TouchKeyStyle.Groups 决定（双拼和五笔也有 14 键，所以有双拼或五笔时同样列出 14/18 键），键盘页的入口按同一条规则显示。
        TouchKeyStyle.Groups groups = TouchKeyStyle.Groups.offeredBy(AppEdition.current());
        List<KeyLayoutStylesPolicy.Kind> kinds = KeyLayoutStylesPolicy.offered(groups);
        if (kind == null || !kinds.contains(kind))
            kind = KeyLayoutStylesPolicy.initialKind(kinds, current, shuangpinLayout, wubiLayout);
        // 从键盘长按字母表那一栏进来时停在 9 键分段，关掉编辑页就能在预览里看到改过的左列。
        if (NineKeySidebarPolicy.LETTER_PREFERENCE.equals(pendingSidebarEdit) && kinds.contains(KeyLayoutStylesPolicy.Kind.NINE))
            kind = KeyLayoutStylesPolicy.Kind.NINE;
        openPendingSidebarEdit(kinds, groups.any());
        if (kind != null) styleSection(context, target, kinds);
        // 每种键面按 123 都进九宫格数字键盘（自己的数字层或借来的数字九宫格），本版本有任一类键盘就列这一组；没有任何一类时整页入口都不列。
        if (groups.any()) numberPadSection(context, target);
        // 顶部数字行只画在 26 键、14 键和 18 键上，本版本两类键面都没有（例如只有九键和笔画）时不列这个开关；它和 iOS、鸿蒙一样放在没有标题的一组里。分段停在 9 键时照样显示，预览里 9 键不画数字行。
        if (!groups.numberRow()) return;
        GroupCard numbers = GroupCard.add(target, null);
        numbers.toggle("顶部数字行", null, numberRow, checked -> {
            numberRow = checked;
            refreshStyles();
            save(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked));
        }).help(NUMBER_ROW_NOTE);
    }

    /** 第一次读到偏好时打开键盘长按带来的那张表的编辑页，之后重读不再打开。本版本不列那一行（字母表要有 9 键分段，数字表要有任一类键盘）时什么也不做。 */
    private void openPendingSidebarEdit(List<KeyLayoutStylesPolicy.Kind> kinds, boolean numberPad) {
        String edit = pendingSidebarEdit;
        if (edit == null) return;
        pendingSidebarEdit = null;
        if (NineKeySidebarPolicy.LETTER_PREFERENCE.equals(edit) && kinds.contains(KeyLayoutStylesPolicy.Kind.NINE))
            editSidebar(false);
        else if (NineKeySidebarPolicy.DIGIT_PREFERENCE.equals(edit) && numberPad)
            editSidebar(true);
    }

    /** 打开一张表的编辑页：标题、说明、当前表（空表时是出厂表）和出厂表都按 `digits` 取；确认后重读页面。 */
    private void editSidebar(boolean digits) {
        List<String> defaults = digits ? NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS : NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        List<String> stored = digits ? digitSidebar : nineKeySidebar;
        new NineKeySidebarSheet(this,
            digits ? KeyLayoutStylesPolicy.DIGIT_SIDEBAR_TITLE : KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_TITLE,
            digits ? KeyLayoutStylesPolicy.DIGIT_SIDEBAR_NOTE : KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_NOTE,
            digits ? NineKeySidebarPolicy.DIGIT_PREFERENCE : NineKeySidebarPolicy.LETTER_PREFERENCE,
            NineKeySidebarPolicy.symbols(stored, defaults), defaults, this::reload).show();
    }

    /** 分段控件、预览、胶囊、说明、说明下面的开关（26 键的互换、各分段共用的中/英位置）和 9 键的「九键左侧符号」：分段只有一种时不画；预览、胶囊、说明、互换开关和那一行的显隐由 {@link #refreshStyles} 按当前一类填。 */
    private void styleSection(Context context, LinearLayout target, List<KeyLayoutStylesPolicy.Kind> kinds) {
        if (kinds.size() > 1) {
            SegmentedControl segments = new SegmentedControl(context);
            segments.setFillWidth(true);
            segments.setMinimumHeight(DimensionPolicy.pixels(context, 40));
            ViewPolicy.setBackground(segments, DrawablePolicy.pill(ThemeColorPolicy.card(context)));
            List<String> titles = new ArrayList<>(kinds.size());
            for (KeyLayoutStylesPolicy.Kind option : kinds) titles.add(option.title);
            segments.setOptions(titles, kinds.indexOf(kind));
            segments.setContentDescription("键盘类型");
            segments.setOnSelect(index -> {
                kind = kinds.get(index);
                refreshStyles();
            });
            target.addView(segments, LayoutPolicy.matchWidthWrapParams());
        }
        KeyLayoutPreview board = new KeyLayoutPreview(context);
        LinearLayout.LayoutParams boardParams = LayoutPolicy.matchWidthWrapParams();
        if (target.getChildCount() > 0) boardParams.topMargin = DimensionPolicy.pixels(context, 16);
        target.addView(board, boardParams);
        preview = board;

        // 胶囊等宽铺满一行，不横向滚动：每类最多四种、名字最长五个字，等分后放得下；系统字号很大时字跟着缩小（chip 里的 setAutoSizeSp）。
        LinearLayout row = LayoutPolicy.row(context);
        LinearLayout.LayoutParams rowParams = LayoutPolicy.matchWidthWrapParams();
        rowParams.topMargin = DimensionPolicy.pixels(context, 12);
        target.addView(row, rowParams);
        chips = row;

        TextView caption = ViewPolicy.styledLabel(context, "", 13, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setPaddingDp(context, caption, Ui.GROUP_TITLE_INSET, 8, Ui.GROUP_TITLE_INSET, 0);
        target.addView(caption, LayoutPolicy.matchWidthWrapParams());
        note = caption;

        // 说明后面一张卡片放开关，与页面其他开关同样式。「符」与「123」互换只作用于 26 键，只在 26 键分段显示；打开时预览底行把 符 排到 123 左边并高亮这两键（refreshStyles）。
        GroupCard toggles = GroupCard.add(target, null);
        if (kinds.contains(KeyLayoutStylesPolicy.Kind.TWENTY_SIX)) {
            symbolFirstRow = toggles.toggle(TouchKeyStyle.SYMBOL_FIRST_TITLE, null, symbolFirst, checked -> {
                symbolFirst = checked;
                refreshStyles();
                save(edited -> edited.put(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, checked));
            }).help(TouchKeyStyle.SYMBOL_FIRST_NOTE);
        }
        // 「中/英键放在空格左边」三类键面都有中/英，每个分段都显示（26 键排在互换开关之后），三个分段是同一项偏好；打开时预览底行把中/英排到空格左边并高亮它。
        toggles.toggle(TouchKeyStyle.LANGUAGE_LEFT_TITLE, null, languageLeft, checked -> {
            languageLeft = checked;
            refreshStyles();
            save(edited -> edited.put(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE, TouchKeyStyle.languageKeySideWire(checked)));
        }).help(TouchKeyStyle.LANGUAGE_LEFT_NOTE);
        // 九键左侧符号接在说明后面，与互换开关同一种写法，只在 9 键分段显示；右侧是当前字母表的摘要，点开进编辑页。
        if (kinds.contains(KeyLayoutStylesPolicy.Kind.NINE)) {
            GroupCard sidebar = GroupCard.add(target, null);
            sidebar.nav(KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_TITLE, null,
                KeyLayoutStylesPolicy.sidebarSummary(nineKeySidebar, NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS),
                () -> editSidebar(false));
            nineKeySidebarGroup = sidebar.view();
        }
        refreshStyles();
    }

    /** 按当前一类、所选样式、数字行开关、「符」与「123」互换和中/英位置重画预览、胶囊和说明，并按分段显隐互换开关；不读也不写偏好。 */
    private void refreshStyles() {
        KeyLayoutStylesPolicy.Kind shown = kind;
        KeyLayoutPreview board = preview;
        LinearLayout row = chips;
        TextView caption = note;
        if (shown == null || board == null || row == null || caption == null) return;
        TouchKeyStyle.Row selected = shown.selected(styles);
        board.show(KeyLayoutStylesPolicy.keyboard(shown, selected, numberRow, eighteen, symbolFirst, languageLeft, nineKeySidebar),
            KeyLayoutStylesPolicy.highlights(shown, selected, symbolFirst, languageLeft, nineKeySidebar));
        Context context = requireContext();
        row.removeAllViews();
        for (TouchKeyStyle.Row option : shown.rows) {
            boolean checked = option == selected;
            addChip(context, row, chip(context, KeyLayoutStylesPolicy.label(shown, option), checked,
                () -> choose(shown, option)));
        }
        // 没有副作用的样式不画这一行，也不留空位。
        ViewPolicy.setTextAndVisibility(caption, KeyLayoutStylesPolicy.note(shown, selected));
        GroupCard.Row swap = symbolFirstRow;
        if (swap != null) swap.setVisible(shown == KeyLayoutStylesPolicy.Kind.TWENTY_SIX);
        View sidebar = nineKeySidebarGroup;
        if (sidebar != null) ViewPolicy.setVisible(sidebar, shown == KeyLayoutStylesPolicy.Kind.NINE);
    }

    /** 「数字键盘」一组：空格位置的等宽胶囊（与样式胶囊同一个 {@link #chip}），胶囊下面一行组末说明，再下面是「数字键盘左侧符号」一行；不画预览。 */
    private void numberPadSection(Context context, LinearLayout target) {
        GroupCard numberPad = GroupCard.add(target, TouchKeyStyle.NUMBER_PAD_SPACE_GROUP);
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setRowPadding(context, row);
        numberPad.addView(row);
        numberPadChips = row;
        numberPad.help(TouchKeyStyle.NUMBER_PAD_SPACE_NOTE);
        refreshNumberPad();
        // 数字表作用于所有九宫格数字键盘（各键面自己的数字层和按 123 借来的那一块），跟着这一组显示，接在组末说明后面。
        GroupCard sidebar = GroupCard.add(target, null);
        sidebar.nav(KeyLayoutStylesPolicy.DIGIT_SIDEBAR_TITLE, null,
            KeyLayoutStylesPolicy.sidebarSummary(digitSidebar, NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS),
            () -> editSidebar(true));
    }

    /** 按当前空格位置重画「数字键盘」的胶囊；不读也不写偏好。 */
    private void refreshNumberPad() {
        LinearLayout row = numberPadChips;
        if (row == null) return;
        Context context = requireContext();
        row.removeAllViews();
        for (TouchKeyStyle.NumberPadSpace choice : TouchKeyStyle.NumberPadSpace.CHOICES)
            addChip(context, row, chip(context, choice.title, choice == numberPadSpace, () -> chooseNumberPadSpace(choice)));
    }

    /** 胶囊等宽铺满一行，相邻两枚隔 6dp。 */
    private static void addChip(Context context, LinearLayout row, TextView chip) {
        LinearLayout.LayoutParams params = LayoutPolicy.weightedWrapParams(1f);
        if (row.getChildCount() > 0) params.setMarginStart(DimensionPolicy.pixels(context, 6));
        row.addView(chip, params);
    }

    /** 一枚胶囊：选中的强调色实底白字，其余浅底；读屏读成按钮，带「已选择」或「未选择」。点已选中的那一枚什么也不做。 */
    private static TextView chip(Context context, String title, boolean checked, Runnable choose) {
        TextView chip = ViewPolicy.pillButton(context, title, Ui.TEXT_BUTTON_SMALL, checked ? 600 : 400,
            checked ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.rowBackground(context),
            checked ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.text(context),
            6, Ui.BUTTON_PADDING_V, Ui.COMPACT_BUTTON_MIN_HEIGHT, 0,
            () -> { if (!checked) choose.run(); });
        ViewPolicy.setAutoSizeSp(chip, 9, Math.round(Ui.TEXT_BUTTON_SMALL), 1);
        ViewPolicy.setSelected(chip, checked);
        ViewCompat.setAccessibilityDelegate(chip, new AccessibilityDelegateCompat() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                info.setContentDescription(title);
                info.setStateDescription(checked ? "已选择" : "未选择");
            }
        });
        return chip;
    }

    /** 选一种样式：先就地换预览和胶囊，再写这一类的共享偏好。 */
    private void choose(KeyLayoutStylesPolicy.Kind shown, TouchKeyStyle.Row option) {
        styles = shown.with(styles, option);
        refreshStyles();
        save(edited -> edited.put(shown.preference, option.wire));
    }

    /** 选一种空格位置：先就地换胶囊，再写 `touch_number_pad_space`。 */
    private void chooseNumberPadSpace(TouchKeyStyle.NumberPadSpace choice) {
        numberPadSpace = choice;
        refreshNumberPad();
        save(edited -> edited.put(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE, choice.wire));
    }

    /** 写共享偏好；成功时界面已经是新值，不再重读，失败时提示「保存失败，请重试」并重读，把页面拨回磁盘上的值。 */
    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, null, this::reload);
    }
}
