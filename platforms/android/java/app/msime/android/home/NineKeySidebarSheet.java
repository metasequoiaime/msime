package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.view.MotionEvent;
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
import java.util.Collections;
import java.util.List;
import org.json.JSONArray;

/**
 * 一张左侧符号表的编辑页（「按键布局」里的「九键左侧符号」「数字键盘左侧符号」共用，参数不同）：当前表逐项列出，可删除、按住把手拖动排序；可从常用符号里点选加到末尾，也可手动输入一项；「恢复默认」回到本端出厂表。整张表不合规（空表、超过 30 项、合起来超过 200 个字符）时保存不可点。
 *
 * <p>保存只写这一张表的共享偏好（`touch_nine_key_sidebar_symbols` 或 `touch_nine_key_digit_sidebar_symbols`），按修订号读改写（{@link KeyboardSheets#save}）；与出厂表逐项相同时写空数组，否则写用户的表。保存失败时提示并留在编辑页，偏好不变。
 */
final class NineKeySidebarSheet {
    private final Fragment owner;
    private final String key;
    private final List<String> defaults;
    private final SettingsSheet sheet;
    private final LinearLayout rows;
    private final GroupCard.Row resetRow;
    private final GroupCard.Row addRow;
    private final GroupCard.Row typeRow;
    private final GroupCard.Row saveRow;
    private final Runnable saved;
    private List<String> symbols;
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
        this.symbols = new ArrayList<>(current);
        this.saved = saved;
        Context context = owner.requireContext();
        sheet = new SettingsSheet(context, title, note);
        rows = LayoutPolicy.column(context);
        GroupCard orderCard = GroupCard.add(sheet.content(), "当前顺序");
        orderCard.card().addView(rows, LayoutPolicy.matchWidthWrapParams());
        GroupCard actions = GroupCard.add(sheet.content(), null);
        resetRow = actions.button("恢复默认符号", null, "恢复默认", () -> {
            symbols = new ArrayList<>(this.defaults);
            render();
        });
        addRow = actions.button("添加符号", null,
            "添加", this::addSymbol).help("从常用符号中选择，最多 " + NineKeySidebarPolicy.MAX_SYMBOLS + " 个");
        typeRow = actions.button("输入符号", null,
            "输入", this::typeSymbol).help("一项最多 " + NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS + " 个字符，如 …… 或 :-)");
        saveRow = actions.button("保存符号栏", null, "保存", this::save);
        actions.button("放弃本次调整", null, "取消", sheet::dismiss);
        render();
    }

    void show() { sheet.show(); }

    private void render() {
        rows.removeAllViews();
        Context context = owner.requireContext();
        if (symbols.isEmpty()) {
            TextView empty = ViewPolicy.styledLabel(context, "当前没有符号。至少加入一个，或点「恢复默认」。",
                Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
            ViewPolicy.setPaddingDp(context, empty, 16, 12, 16, 12);
            rows.addView(empty, LayoutPolicy.matchWidthWrapParams());
        } else {
            for (int index = 0; index < symbols.size(); index++) addRow(index);
        }
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

    private void addRow(int index) {
        Context context = owner.requireContext();
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        ViewPolicy.setRowMinimumHeight(context, row);
        ViewPolicy.setRowPadding(context, row);
        ViewPolicy.setBackground(row, ViewPolicy.ripple(context));
        TextView symbol = ViewPolicy.styledLabel(context, symbols.get(index), Ui.TEXT_BAR_TITLE, 400,
            ThemeColorPolicy.text(context));
        symbol.setContentDescription("符号：" + symbols.get(index));
        ViewPolicy.setCentered(symbol);
        ViewPolicy.setHorizontalPaddingDp(context, symbol, 12);
        ViewPolicy.setBackground(symbol, DrawablePolicy.rounded(ThemeColorPolicy.accentSoft(context),
            DimensionPolicy.pixels(context, 10)));
        LinearLayout symbolSlot = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(symbolSlot);
        symbolSlot.addView(symbol, LayoutPolicy.wrapParams());
        row.addView(symbolSlot, LayoutPolicy.weightedWrapParams(1));
        row.addView(dragHandle(row, symbols.get(index)), LayoutPolicy.wrapParams());
        ImageView delete = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_delete,
            ThemeColorPolicy.subText(context), "删除 " + symbols.get(index), 48,
            () -> remove(row));
        row.addView(delete, LayoutPolicy.wrapParams());
        rows.addView(row, LayoutPolicy.matchWidthWrapParams());
    }

    private ImageView dragHandle(LinearLayout row, String symbol) {
        Context context = owner.requireContext();
        ImageView handle = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_drag_handle,
            ThemeColorPolicy.subText(context), "拖动调整 " + symbol + " 的顺序", 48, null);
        ViewPolicy.setEnabledWithAlpha(handle, !saving, 0.38f);
        handle.setOnTouchListener((view, event) -> handleDrag(row, event));
        return handle;
    }

    private boolean handleDrag(LinearLayout row, MotionEvent event) {
        if (saving) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (row.getParent() != rows) return false;
                rows.requestDisallowInterceptTouchEvent(true);
                row.setAlpha(0.72f);
                ViewPolicy.setElevation(row, DimensionPolicy.pixels(owner.requireContext(), 8));
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (row.getParent() != rows) return false;
                int target = dragTarget(event.getRawY());
                int current = rows.indexOfChild(row);
                if (target != current) reorderRow(row, current, target);
                return true;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                rows.requestDisallowInterceptTouchEvent(false);
                row.setAlpha(1f);
                ViewPolicy.clearElevation(row);
                return true;
            }
            default -> {
                return true;
            }
        }
    }

    private int dragTarget(float rawY) {
        for (int index = 0; index < rows.getChildCount(); index++) {
            View child = rows.getChildAt(index);
            int[] location = new int[2];
            child.getLocationOnScreen(location);
            if (rawY < location[1] + child.getHeight() / 2f) return index;
        }
        return Math.max(0, rows.getChildCount() - 1);
    }

    private void reorderRow(LinearLayout row, int current, int target) {
        Collections.swap(symbols, current, target);
        rows.removeViewAt(current);
        rows.addView(row, target);
    }

    private void remove(LinearLayout row) {
        if (saving) return;
        int index = rows.indexOfChild(row);
        if (index < 0 || index >= symbols.size()) return;
        symbols.remove(index);
        render();
    }

    private void addSymbol() {
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
                if (saving || NineKeySidebarPolicy.addable(symbols, symbol) == null) return;
                symbols.add(symbol);
                render();
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
        if (saving || symbols.size() >= NineKeySidebarPolicy.MAX_SYMBOLS) return;
        InputDialog dialog = new InputDialog(owner.requireContext(), "输入符号",
            "一项最多 " + NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS + " 个字符，不含空格，不能与已有的重复");
        dialog.addField("符号", "", InputType.TYPE_CLASS_TEXT).setContentDescription("要加入符号栏的符号");
        dialog.setValidator(values -> NineKeySidebarPolicy.addable(symbols, values.get(0)) != null);
        dialog.setPrimary("加入", values -> {
            String symbol = NineKeySidebarPolicy.addable(symbols, values.get(0));
            if (saving || symbol == null) return;
            symbols.add(symbol);
            render();
        });
        dialog.show();
    }

    private void save() {
        if (saving || !NineKeySidebarPolicy.valid(symbols)) return;
        saving = true;
        render();
        // 与出厂表相同时写空数组，键盘和别的设备都按自己的出厂表画；只改这一个字段。
        List<String> stored = NineKeySidebarPolicy.toPreference(symbols, defaults);
        KeyboardSheets.save(owner, edited -> edited.put(key, new JSONArray(stored)),
            () -> {
                sheet.dismiss();
                saved.run();
            }, () -> {
                saving = false;
                render();
            });
    }
}
