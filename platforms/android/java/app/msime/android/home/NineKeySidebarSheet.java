package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.fragment.app.Fragment;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;

/**
 * 一张左侧符号表的编辑页（「按键布局」里的「九键左侧符号」「数字键盘左侧符号」共用，参数不同）：当前表逐项列出，每项前有序号 1..N，可删除、按住把手拖动排序（行浮起跟着手指走，其余行滑开让位，序号随位次实时变化，贴近面板上下边缘时自动滚动；读屏用「上移」「下移」，实体键盘用 Alt+↑ / Alt+↓）；可从常用符号里点选加到末尾，也可手动输入一项；「恢复默认」回到本端出厂表。整张表不合规（空表、超过 30 项、合起来超过 200 个字符）时保存不可点。
 *
 * <p>拖动和读屏操作由 {@link ReorderList} 管，与其余排序面板同一个控制器（#10546）；这里只画行（序号、符号牌、把手、删除键）和面板其余的按钮。
 *
 * <p>保存只写这一张表的共享偏好（`touch_nine_key_sidebar_symbols` 或 `touch_nine_key_digit_sidebar_symbols`），按修订号读改写（{@link KeyboardSheets#save}）；与出厂表逐项相同时写空数组，否则写用户的表。保存失败时提示并留在编辑页，偏好不变。
 */
final class NineKeySidebarSheet {
    /** 序号一栏的宽度，放得下「30.」。 */
    private static final int ORDINAL_WIDTH_DP = 32;

    private final Fragment owner;
    private final String key;
    private final List<String> defaults;
    private final SettingsSheet sheet;
    private final ReorderList<String> rows;
    private final TextView empty;
    private final GroupCard.Row resetRow;
    private final GroupCard.Row addRow;
    private final GroupCard.Row typeRow;
    private final GroupCard.Row saveRow;
    private final Runnable saved;
    private boolean saving;

    /**
     * @param key 这张表的共享偏好名
     * @param current 当前显示的表（共享偏好里是空表时传出厂表）
     * @param defaults 本端出厂表，「恢复默认」和「等于出厂表写空数组」都按它比
     */
    NineKeySidebarSheet(Fragment owner, String title, String note, String key, List<String> current,
            List<String> defaults, Runnable saved) {
        this.owner = owner;
        this.key = key;
        this.defaults = List.copyOf(defaults);
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, title, note);
        rows = new ReorderList<>(context, current, this::symbolRow);
        GroupCard orderCard = GroupCard.add(sheet.content(), "当前顺序");
        orderCard.card().addView(rows.view(), LayoutPolicy.matchWidthWrapParams());
        // 空表提示放在列表外面，列表只放可拖动的行。
        empty = ViewPolicy.styledLabel(context, "当前没有符号。至少加入一个，或点「恢复默认」。",
            Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setPaddingDp(context, empty, 16, 12, 16, 12);
        orderCard.card().addView(empty, LayoutPolicy.matchWidthWrapParams());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认符号", null, "恢复默认", () -> setSymbols(this.defaults));
        addRow = actions.button("添加符号", null,
            "添加", this::addSymbol).help("从常用符号中选择，最多 " + NineKeySidebarPolicy.MAX_SYMBOLS + " 个");
        typeRow = actions.button("输入符号", null,
            "输入", this::typeSymbol).help("一项最多 " + NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS + " 个字符，如 …… 或 :-)");
        saveRow = actions.button("保存符号栏", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
        refreshActions();
    }

    void show() { sheet.show(); }

    /** 整表换成 `next`（恢复默认、删除、添加），列表重画，按钮跟着更新。拖动排序不改表的长度和内容，不经过这里。 */
    private void setSymbols(List<String> next) {
        rows.setItems(next);
        refreshActions();
    }

    /** 按当前的表和保存状态更新空表提示和四个按钮。 */
    private void refreshActions() {
        List<String> symbols = rows.items();
        ViewPolicy.setVisible(empty, symbols.isEmpty());
        boolean valid = NineKeySidebarPolicy.valid(symbols);
        boolean room = symbols.size() < NineKeySidebarPolicy.MAX_SYMBOLS;
        resetRow.setEnabled(!saving);
        addRow.setEnabled(!saving && room);
        typeRow.setEnabled(!saving && room);
        // 整表校验与 NineKeySidebarPolicy.parse 同一套规则；不合规时保存不可点，并说明缺了什么。
        saveRow.setEnabled(!saving && valid);
        saveRow.setSubtitle(valid ? null : symbols.isEmpty() ? "至少保留一个符号"
            : "最多 " + NineKeySidebarPolicy.MAX_SYMBOLS + " 个，合起来不超过 "
                + NineKeySidebarPolicy.MAX_TEXT_LENGTH + " 个字符");
    }

    /** 「当前顺序」的一行：序号、符号牌、拖动把手、删除键。拖动和读屏操作由 {@link ReorderList} 挂到把手上。 */
    private ReorderList.Row symbolRow(String text, int index) {
        Context context = owner.requireContext();
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        ViewPolicy.setRowMinimumHeight(context, row);
        ViewPolicy.setRowPadding(context, row);
        ViewPolicy.setBackground(row, ViewPolicy.ripple(context));
        // 序号按两位数留宽、右对齐、等宽数字，1. 到 30. 的符号牌左沿对齐；位置由符号牌的读屏描述念，序号本身不念。
        TextView ordinal = ViewPolicy.styledLabel(context, NineKeySidebarPolicy.ordinal(index), Ui.TEXT_ROW_TITLE, 400,
            ThemeColorPolicy.subText(context));
        ordinal.setGravity(Gravity.END | Gravity.CENTER_VERTICAL);
        ordinal.setFontFeatureSettings("tnum");
        ordinal.setMinWidth(DimensionPolicy.pixels(context, ORDINAL_WIDTH_DP));
        ordinal.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams ordinalParams = LayoutPolicy.wrapParams();
        ordinalParams.rightMargin = DimensionPolicy.pixels(context, 12);
        row.addView(ordinal, ordinalParams);
        TextView symbol = ViewPolicy.styledLabel(context, text, Ui.TEXT_BAR_TITLE, 400,
            ThemeColorPolicy.text(context));
        ViewPolicy.setCentered(symbol);
        ViewPolicy.setHorizontalPaddingDp(context, symbol, 12);
        ViewPolicy.setBackground(symbol, DrawablePolicy.rounded(ThemeColorPolicy.accentSoft(context),
            DimensionPolicy.pixels(context, 10)));
        LinearLayout symbolSlot = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(symbolSlot);
        symbolSlot.addView(symbol, LayoutPolicy.wrapParams());
        row.addView(symbolSlot, LayoutPolicy.weightedWrapParams(1));
        ImageView handle = ReorderList.addHandle(context, row, text);
        ImageView delete = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_delete,
            ThemeColorPolicy.subText(context), "删除 " + text, 48,
            () -> remove(index));
        row.addView(delete, LayoutPolicy.wrapParams());
        return new ReorderList.Row(row, LayoutPolicy.matchWidthWrapParams(), handle, position -> {
            ordinal.setText(NineKeySidebarPolicy.ordinal(position));
            symbol.setContentDescription(ReorderPolicy.ordinalDescription(position, "符号：" + text));
        });
    }

    /** 删掉第 `index` 项。每次改表都会重画，行里记下的下标就是它此刻在表里的位置。 */
    private void remove(int index) {
        List<String> symbols = rows.items();
        if (saving || index < 0 || index >= symbols.size()) return;
        List<String> next = new ArrayList<>(symbols);
        next.remove(index);
        setSymbols(next);
    }

    /** 把 `symbol` 接在表末尾。 */
    private void append(String symbol) {
        List<String> next = new ArrayList<>(rows.items());
        next.add(symbol);
        setSymbols(next);
    }

    private void addSymbol() {
        List<String> symbols = rows.items();
        if (saving || symbols.size() >= NineKeySidebarPolicy.MAX_SYMBOLS) return;
        Context context = owner.requireContext();
        SettingsSheet choices = new SettingsSheet(context, "添加符号", "点选一个加入当前顺序");
        ChipGroup grid = new ChipGroup(context);
        grid.setSingleLine(false);
        grid.setChipSpacingHorizontal(DimensionPolicy.pixels(context, 8));
        grid.setChipSpacingVertical(DimensionPolicy.pixels(context, 8));
        ViewPolicy.setPaddingDp(context, grid, 16, 8, 16, 8);
        for (String symbol : NineKeySidebarPolicy.addableSymbols(symbols)) {
            if (NineKeySidebarPolicy.addable(symbols, symbol) == null) continue;
            Chip chip = new Chip(context);
            chip.setText(symbol);
            chip.setTextSize(Ui.TEXT_ROW_TITLE);
            chip.setCheckable(false);
            chip.setMinHeight(DimensionPolicy.pixels(context, 48));
            chip.setContentDescription("添加 " + symbol);
            ViewPolicy.bindClick(chip, () -> {
                choices.dismiss();
                if (saving || NineKeySidebarPolicy.addable(rows.items(), symbol) == null) return;
                append(symbol);
            });
            grid.addView(chip, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        }
        choices.add(grid);
        GroupCard actions = GroupCard.add(choices.content(), null);
        actions.button("关闭选择", null, "取消", choices::dismiss);
        choices.show();
    }

    /** 手动输入一项：去掉首尾空白后必须是一个合规的符号、表里还没有，接上后整张表仍合规，否则确认键不可点。 */
    private void typeSymbol() {
        if (saving || rows.items().size() >= NineKeySidebarPolicy.MAX_SYMBOLS) return;
        InputDialog dialog = new InputDialog(owner.requireContext(), "输入符号",
            "一项最多 " + NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS + " 个字符，不含空格，不能与已有的重复");
        dialog.addField("符号", "", InputType.TYPE_CLASS_TEXT).setContentDescription("要加入符号栏的符号");
        dialog.setValidator(values -> NineKeySidebarPolicy.addable(rows.items(), values.get(0)) != null);
        dialog.setPrimary("加入", values -> {
            String symbol = NineKeySidebarPolicy.addable(rows.items(), values.get(0));
            if (saving || symbol == null) return;
            append(symbol);
        });
        dialog.show();
    }

    private void save() {
        List<String> symbols = rows.items();
        if (saving || !NineKeySidebarPolicy.valid(symbols)) return;
        saving = true;
        rows.setLocked(true);
        refreshActions();
        // 与出厂表相同时写空数组，键盘和别的设备都按自己的出厂表画；只改这一个字段。
        List<String> stored = NineKeySidebarPolicy.toPreference(symbols, defaults);
        KeyboardSheets.save(owner, edited -> edited.put(key, new JSONArray(stored)),
            () -> {
                sheet.dismiss();
                saved.run();
            }, () -> {
                saving = false;
                rows.setLocked(false);
                refreshActions();
            });
    }
}
