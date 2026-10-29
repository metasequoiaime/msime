package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.widget.Button;
import android.widget.HorizontalScrollView;
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
 * 「按键布局」子页（键盘页「布局」组进来），自上而下：键盘类型分段控件（26 键 | 14/18 键 | 9 键，只列本版本提供的类型，只有一种时不画），这一类键盘所选样式下的整块键盘预览（{@link KeyLayoutPreview}，与标准不同的键描强调色），一排样式胶囊，当前样式的副作用说明（没有就不画），最后是没有标题的一组里的「顶部数字行」开关（本版本既没有 26 键也没有 14/18 键时不列）。四项都写共享偏好（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_number_row`，{@link TouchKeyStyle}），随账号同步，键盘每秒重读偏好时就地换底行，不打断正在组的字。本版本三类都没有时键盘页不列入口（{@link TouchKeyStyle.Groups}）。哪一类、叫什么、高亮哪些键、说明写什么都在 {@link KeyLayoutStylesPolicy} 里。
 *
 * <p>分段控件默认停在用户当前在用的中文键盘类型；切分段只换预览和胶囊，不写任何偏好。点胶囊立刻换预览再写偏好，保存失败时提示并重新读一遍，拨回磁盘上的值。
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
    private boolean eighteen;
    @Nullable private KeyLayoutPreview preview;
    @Nullable private LinearLayout chips;
    @Nullable private TextView note;

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
        // 顶部数字行只画在 26 键、14 键和 18 键上，本版本两类键面都没有（例如只有九键和笔画）时不列这个开关；它和 iOS、鸿蒙一样放在没有标题的一组里。分段停在 9 键时照样显示，预览里 9 键不画数字行。
        if (!groups.numberRow()) return;
        GroupCard numbers = GroupCard.add(target, null);
        numbers.toggle("顶部数字行", NUMBER_ROW_NOTE, numberRow, checked -> {
            numberRow = checked;
            refreshStyles();
            save(edited -> edited.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, checked));
        });
    }

    /** 分段控件、预览、胶囊和说明：分段只有一种时不画；预览、胶囊和说明由 {@link #refreshStyles} 按当前一类填。 */
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

        // 胶囊横向排开，放不下时横向滚动，不折行。
        HorizontalScrollView strip = new HorizontalScrollView(context);
        strip.setHorizontalScrollBarEnabled(false);
        LinearLayout row = LayoutPolicy.row(context);
        strip.addView(row);
        LinearLayout.LayoutParams stripParams = LayoutPolicy.matchWidthWrapParams();
        stripParams.topMargin = DimensionPolicy.pixels(context, 12);
        target.addView(strip, stripParams);
        chips = row;

        TextView caption = ViewPolicy.styledLabel(context, "", 13, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setPaddingDp(context, caption, Ui.GROUP_TITLE_INSET, 8, Ui.GROUP_TITLE_INSET, 0);
        target.addView(caption, LayoutPolicy.matchWidthWrapParams());
        note = caption;
        refreshStyles();
    }

    /** 按当前一类、所选样式和数字行开关重画预览、胶囊和说明；不读也不写偏好。 */
    private void refreshStyles() {
        KeyLayoutStylesPolicy.Kind shown = kind;
        KeyLayoutPreview board = preview;
        LinearLayout row = chips;
        TextView caption = note;
        if (shown == null || board == null || row == null || caption == null) return;
        TouchKeyStyle.Row selected = shown.selected(styles);
        board.show(KeyLayoutStylesPolicy.keyboard(shown, selected, numberRow, eighteen),
            KeyLayoutStylesPolicy.highlights(shown, selected));
        Context context = requireContext();
        row.removeAllViews();
        for (TouchKeyStyle.Row option : shown.rows) {
            LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
            if (row.getChildCount() > 0) params.setMarginStart(DimensionPolicy.pixels(context, 8));
            row.addView(chip(context, shown, option, option == selected), params);
        }
        // 没有副作用的样式不画这一行，也不留空位。
        ViewPolicy.setTextAndVisibility(caption, KeyLayoutStylesPolicy.note(shown, selected));
    }

    /** 一枚样式胶囊：选中的强调色实底白字，其余浅底；读屏读成按钮，带「已选择」或「未选择」。点已选中的那一枚什么也不做。 */
    private TextView chip(Context context, KeyLayoutStylesPolicy.Kind shown, TouchKeyStyle.Row option, boolean checked) {
        String title = KeyLayoutStylesPolicy.label(shown, option);
        TextView chip = ViewPolicy.pillButton(context, title, Ui.TEXT_BUTTON_SMALL, checked ? 600 : 400,
            checked ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.rowBackground(context),
            checked ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.text(context),
            Ui.BUTTON_PADDING_H, Ui.BUTTON_PADDING_V, Ui.COMPACT_BUTTON_MIN_HEIGHT, 0,
            () -> { if (!checked) choose(shown, option); });
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

    /** 写共享偏好；成功时界面已经是新值，不再重读，失败时提示「保存失败，请重试」并重读，把页面拨回磁盘上的值。 */
    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, null, this::reload);
    }
}
