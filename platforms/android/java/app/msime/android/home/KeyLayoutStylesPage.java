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
import app.msime.android.SchemePreferences;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/**
 * 「按键布局」子页（键盘页「布局」组进来），自上而下：键盘类型分段控件（26 键 | 14/18 键 | 9 键，只列本版本提供的类型，只有一种时不画），这一类键盘所选样式下的整块键盘预览（{@link KeyLayoutPreview}，与标准不同的键描强调色），一排样式胶囊，当前样式的副作用说明（没有就不画），26 键分段时再加一行「26 键「符」与「123」互换」开关；接着是「数字键盘」一组空格位置的胶囊（空格在右列、在 0 右边、在 0 左边）和组末说明；最后是没有标题的一组里的「顶部数字行」开关（本版本既没有 26 键也没有 14/18 键时不列）。各项都写共享偏好（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_twenty_six_symbol_first`、`touch_number_pad_space`、`touch_number_row`，{@link TouchKeyStyle}），随账号同步，键盘每秒重读偏好时就地换底行，不打断正在组的字。本版本三类都没有时键盘页不列入口（{@link TouchKeyStyle.Groups}）。哪一类、叫什么、高亮哪些键、说明写什么都在 {@link KeyLayoutStylesPolicy} 里。
 *
 * <p>分段控件默认停在用户当前在用的中文键盘类型；切分段只换预览、胶囊和互换开关的显隐，不写任何偏好。点胶囊或开关立刻换预览再写偏好，保存失败时提示并重新读一遍，拨回磁盘上的值。
 *
 * <p>页面打开时先把本机旧的「长空格布局」和「数字行」设置迁进共享偏好（{@link KeyLayoutStyleMigration}），显示的就是迁移后的选择。
 */
public class KeyLayoutStylesPage extends ReloadingDetailPage {
    // 「顶部数字行」的说明，iOS、鸿蒙写的是同样的文字。
    static final String NUMBER_ROW_NOTE = "在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。";

    /** 分段控件停在哪一类；第一次读到偏好时按当前键盘定，之后保存后重读时保持用户切到的那一类。 */
    @Nullable private KeyLayoutStylesPolicy.Kind kind;
    private TouchKeyStyle.Styles styles = TouchKeyStyle.Styles.DEFAULT;
    private boolean numberRow;
    private boolean symbolFirst;
    private TouchKeyStyle.NumberPadSpace numberPadSpace = TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN;
    private boolean eighteen;
    @Nullable private KeyLayoutPreview preview;
    @Nullable private LinearLayout chips;
    @Nullable private TextView note;
    @Nullable private View symbolFirstGroup;
    @Nullable private LinearLayout numberPadChips;

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
        symbolFirstGroup = null;
        numberPadChips = null;
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
        // 没选过时选中 Android 原来的「空格在右列」，点哪一枚写哪一枚的写法。
        numberPadSpace = TouchKeyStyle.NumberPadSpace.parse(
            JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE))).resolved();
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
        if (kind != null) styleSection(context, target, kinds);
        // 九宫格数字键盘三端都从各类中文键面进得去（自己的数字层或借来的数字九宫格），本版本有任一类键盘就列这一组。
        if (groups.any()) numberPadSection(context, target);
        // 顶部数字行只画在 26 键、14 键和 18 键上，本版本两类键面都没有（例如只有九键和笔画）时不列这个开关；它和 iOS、鸿蒙一样放在没有标题的一组里。分段停在 9 键时照样显示，预览里 9 键不画数字行。
        if (!groups.numberRow()) return;
        GroupCard numbers = GroupCard.add(target, null);
        numbers.toggle("顶部数字行", NUMBER_ROW_NOTE, numberRow, checked -> {
            numberRow = checked;
            refreshStyles();
            save(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked));
        });
    }

    /** 分段控件、预览、胶囊、说明和 26 键的互换开关：分段只有一种时不画；预览、胶囊、说明和开关的显隐由 {@link #refreshStyles} 按当前一类填。 */
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

        // 「符」与「123」互换只作用于 26 键：开关接在说明后面，与页面其他开关同样式，只在 26 键分段显示；打开时预览底行把 符 排到 123 左边并高亮这两键（refreshStyles）。
        if (kinds.contains(KeyLayoutStylesPolicy.Kind.TWENTY_SIX)) {
            GroupCard swap = GroupCard.add(target, null);
            swap.toggle(TouchKeyStyle.SYMBOL_FIRST_TITLE, TouchKeyStyle.SYMBOL_FIRST_NOTE, symbolFirst, checked -> {
                symbolFirst = checked;
                refreshStyles();
                save(edited -> edited.put(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, checked));
            });
            symbolFirstGroup = swap.view();
        }
        refreshStyles();
    }

    /** 按当前一类、所选样式、数字行开关和「符」与「123」互换重画预览、胶囊和说明，并按分段显隐互换开关；不读也不写偏好。 */
    private void refreshStyles() {
        KeyLayoutStylesPolicy.Kind shown = kind;
        KeyLayoutPreview board = preview;
        LinearLayout row = chips;
        TextView caption = note;
        if (shown == null || board == null || row == null || caption == null) return;
        TouchKeyStyle.Row selected = shown.selected(styles);
        board.show(KeyLayoutStylesPolicy.keyboard(shown, selected, numberRow, eighteen, symbolFirst),
            KeyLayoutStylesPolicy.highlights(shown, selected, symbolFirst));
        Context context = requireContext();
        row.removeAllViews();
        for (TouchKeyStyle.Row option : shown.rows) {
            boolean checked = option == selected;
            addChip(context, row, chip(context, KeyLayoutStylesPolicy.label(shown, option), checked,
                () -> choose(shown, option)));
        }
        // 没有副作用的样式不画这一行，也不留空位。
        ViewPolicy.setTextAndVisibility(caption, KeyLayoutStylesPolicy.note(shown, selected));
        View swap = symbolFirstGroup;
        if (swap != null) ViewPolicy.setVisible(swap, shown == KeyLayoutStylesPolicy.Kind.TWENTY_SIX);
    }

    /** 「数字键盘」一组：空格位置的等宽胶囊（与样式胶囊同一个 {@link #chip}），胶囊下面一行组末说明；不画预览。 */
    private void numberPadSection(Context context, LinearLayout target) {
        GroupCard numberPad = GroupCard.add(target, TouchKeyStyle.NUMBER_PAD_SPACE_GROUP);
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setRowPadding(context, row);
        numberPad.addView(row);
        numberPadChips = row;
        numberPad.footer(TouchKeyStyle.NUMBER_PAD_SPACE_NOTE);
        refreshNumberPad();
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
