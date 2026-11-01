package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewParent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.animation.DecelerateInterpolator;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.core.view.ViewCompat;
import androidx.core.widget.NestedScrollView;
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
 * 一张左侧符号表的编辑页（「按键布局」里的「九键左侧符号」「数字键盘左侧符号」共用，参数不同）：当前表逐项列出，每项前有序号 1..N，可删除、按住把手拖动排序（行浮起跟着手指走，其余行滑开让位，序号随位次实时变化，贴近面板上下边缘时自动滚动；读屏用「上移」「下移」）；可从常用符号里点选加到末尾，也可手动输入一项；「恢复默认」回到本端出厂表。整张表不合规（空表、超过 30 项、合起来超过 200 个字符）时保存不可点。
 *
 * <p>保存只写这一张表的共享偏好（`touch_nine_key_sidebar_symbols` 或 `touch_nine_key_digit_sidebar_symbols`），按修订号读改写（{@link KeyboardSheets#save}）；与出厂表逐项相同时写空数组，否则写用户的表。保存失败时提示并留在编辑页，偏好不变。
 */
final class NineKeySidebarSheet {
    /** 序号一栏的宽度，放得下「30.」。 */
    private static final int ORDINAL_WIDTH_DP = 32;
    /** 拿起的行浮起多高。 */
    private static final int LIFT_DP = 8;
    /** 拿起的行换成带圆角的不透明底色，盖在让位的行上面。 */
    private static final int LIFTED_RADIUS_DP = 12;
    /** 让位、落位和浮起的动画时长。 */
    private static final long ANIMATION_MILLIS = 150;
    /** 滚动区上下各多高的一条边缘带里开始自动滚动。 */
    private static final int EDGE_DP = 56;
    /** 自动滚动每帧最多滚多少。 */
    private static final int EDGE_STEP_DP = 12;

    /** 一行里拖动和重排序号时要改的几个视图。 */
    private record RowViews(LinearLayout row, TextView ordinal, TextView symbol, ImageView handle, String text) {}

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
    private final List<RowViews> rowViews = new ArrayList<>();
    private final Runnable autoScroll = this::autoScrollStep;
    private List<String> symbols;
    private boolean saving;
    /** 正在拖的那一行，没有拖动时为 null；拖动中 {@link #dragFrom} 是它在表里的原位，{@link #dragTo} 是此刻的落点。 */
    private RowViews dragged;
    private int dragFrom;
    private int dragTo;
    private float downRawY;
    private float lastRawY;
    private int rowsTopAtDown;
    /** 拿起时各行在布局里的中线，判断落点只看它们。 */
    private float[] centers = new float[0];
    private NestedScrollView scroller;

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
        // 重画时作废进行中的拖动（多指拖动时另一指点了删除也走到这里）。
        dragged = null;
        rows.removeCallbacks(autoScroll);
        rows.removeAllViews();
        rowViews.clear();
        Context context = owner.requireContext();
        if (symbols.isEmpty()) {
            TextView empty = ViewPolicy.styledLabel(context, "当前没有符号。至少加入一个，或点「恢复默认」。",
                Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
            ViewPolicy.setPaddingDp(context, empty, 16, 12, 16, 12);
            rows.addView(empty, LayoutPolicy.matchWidthWrapParams());
        } else {
            for (int index = 0; index < symbols.size(); index++) addRow(index);
            renumber();
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
        String text = symbols.get(index);
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
        ImageView handle = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_drag_handle,
            ThemeColorPolicy.subText(context), "拖动调整 " + text + " 的顺序", 48, null);
        ViewPolicy.setEnabledWithAlpha(handle, !saving, 0.38f);
        row.addView(handle, LayoutPolicy.wrapParams());
        ImageView delete = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_delete,
            ThemeColorPolicy.subText(context), "删除 " + text, 48,
            () -> remove(row));
        row.addView(delete, LayoutPolicy.wrapParams());
        rows.addView(row, LayoutPolicy.matchWidthWrapParams());
        RowViews views = new RowViews(row, ordinal, symbol, handle, text);
        rowViews.add(views);
        handle.setOnTouchListener((view, event) -> handleDrag(views, event));
        // 把手只有触摸拖动，读屏另给「上移」「下移」两个操作，与共享设置页每行视觉隐藏的「上移」「下移」按钮对应。
        if (!saving && index > 0) ViewCompat.addAccessibilityAction(handle, "上移", (view, arguments) -> moveBy(views, -1));
        if (!saving && index < symbols.size() - 1) {
            ViewCompat.addAccessibilityAction(handle, "下移", (view, arguments) -> moveBy(views, 1));
        }
    }

    /** 按每行此刻的位次写序号和读屏描述：平时就是行号，拖动途中按 {@link NineKeySidebarPolicy#dragSlot} 跟着手指变。 */
    private void renumber() {
        for (int index = 0; index < rowViews.size(); index++) {
            RowViews views = rowViews.get(index);
            int position = dragged == null ? index : NineKeySidebarPolicy.dragSlot(index, dragFrom, dragTo);
            views.ordinal().setText(NineKeySidebarPolicy.ordinal(position));
            views.symbol().setContentDescription(NineKeySidebarPolicy.ordinalDescription(position, views.text()));
            ViewCompat.setStateDescription(views.handle(), NineKeySidebarPolicy.positionDescription(position));
        }
    }

    private boolean handleDrag(RowViews views, MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                return startDrag(views, event.getRawY());
            }
            case MotionEvent.ACTION_MOVE -> {
                if (dragged != views) return false;
                lastRawY = event.getRawY();
                followFinger();
                return true;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (dragged == views) drop();
                return true;
            }
            default -> {
                return dragged == views;
            }
        }
    }

    /**
     * 按住把手拿起这一行：行浮起（不透明底色加阴影，盖在别的行上面），之后跟着手指走。行的视图在拖动中一直留在原位，只改 translationY：落点按各行拿起时的布局中线算（{@link NineKeySidebarPolicy#dragTarget}），不受跟手和让位位移影响，松手才把表挪好、重画。
     */
    private boolean startDrag(RowViews views, float rawY) {
        if (saving || dragged != null || views.row().getParent() != rows) return false;
        int from = rowViews.indexOf(views);
        if (from < 0) return false;
        Context context = owner.requireContext();
        centers = new float[rowViews.size()];
        for (int index = 0; index < rowViews.size(); index++) {
            LinearLayout row = rowViews.get(index).row();
            // 上一次松手的落位动画可能还没走完，先归位，各行都从布局位置起算。
            row.animate().cancel();
            row.setTranslationY(0);
            row.setTranslationZ(0);
            centers[index] = row.getTop() + row.getHeight() / 2f;
        }
        dragged = views;
        dragFrom = from;
        dragTo = from;
        downRawY = rawY;
        lastRawY = rawY;
        rowsTopAtDown = screenTop(rows);
        scroller = enclosingScroller();
        rows.requestDisallowInterceptTouchEvent(true);
        LinearLayout row = views.row();
        ViewPolicy.setBackground(row, DrawablePolicy.rounded(ThemeColorPolicy.rowBackground(context),
            DimensionPolicy.pixels(context, LIFTED_RADIUS_DP)));
        row.animate().translationZ(DimensionPolicy.pixels(context, LIFT_DP)).setDuration(ANIMATION_MILLIS).start();
        return true;
    }

    /** 被拖行跟着手指（含拖动中滚动区滚过的距离），越过别的行的中线时那些行让位、序号跟着变；贴近滚动区上下边缘时接着自动滚动。 */
    private void followFinger() {
        if (dragged == null) return;
        LinearLayout row = dragged.row();
        float scrolled = rowsTopAtDown - screenTop(rows);
        float offset = lastRawY - downRawY + scrolled;
        // 不拖出「当前顺序」这张卡片：卡片裁掉圆角外的内容，拖出去就看不见了。
        offset = Math.max(-row.getTop(), Math.min(rows.getHeight() - row.getBottom(), offset));
        row.setTranslationY(offset);
        int target = NineKeySidebarPolicy.dragTarget(centers[dragFrom] + offset, dragFrom, centers);
        if (target != dragTo) {
            dragTo = target;
            makeWay();
            renumber();
        }
        scheduleAutoScroll();
    }

    /** 夹在原位和落点之间的行朝原位那一侧让出被拖行的高度，其余的回到布局位置；都用短动画滑过去。 */
    private void makeWay() {
        float height = dragged.row().getHeight();
        for (int index = 0; index < rowViews.size(); index++) {
            if (index == dragFrom) continue;
            float shift = (NineKeySidebarPolicy.dragSlot(index, dragFrom, dragTo) - index) * height;
            View row = rowViews.get(index).row();
            if (row.getTranslationY() == shift) continue;
            row.animate().translationY(shift).setDuration(ANIMATION_MILLIS)
                .setInterpolator(new DecelerateInterpolator()).start();
        }
    }

    /** 松手：表按挪动语义改好再重画，其余行此刻已经在新位置上，重画后不动；被拖行从手指处滑进自己的格子、落下阴影。 */
    private void drop() {
        RowViews views = dragged;
        int from = dragFrom;
        int to = dragTo;
        rows.removeCallbacks(autoScroll);
        rows.requestDisallowInterceptTouchEvent(false);
        float height = views.row().getHeight();
        float remaining = views.row().getTranslationY() - (to - from) * height;
        float lift = views.row().getTranslationZ();
        dragged = null;
        symbols = NineKeySidebarPolicy.moved(symbols, from, to);
        render();
        if (to >= rowViews.size()) return;
        Context context = owner.requireContext();
        LinearLayout landed = rowViews.get(to).row();
        ViewPolicy.setBackground(landed, DrawablePolicy.rounded(ThemeColorPolicy.rowBackground(context),
            DimensionPolicy.pixels(context, LIFTED_RADIUS_DP)));
        landed.setTranslationY(remaining);
        landed.setTranslationZ(lift);
        landed.animate().translationY(0).translationZ(0).setDuration(ANIMATION_MILLIS)
            .setInterpolator(new DecelerateInterpolator())
            .withEndAction(() -> ViewPolicy.setBackground(landed, ViewPolicy.ripple(context))).start();
    }

    /** 读屏的「上移」「下移」：挪一格后重画，焦点放回挪过去的那一行的把手上，读屏念出新的位次。 */
    private boolean moveBy(RowViews views, int step) {
        if (saving || dragged != null) return false;
        int from = rowViews.indexOf(views);
        int to = from + step;
        if (from < 0 || to < 0 || to >= symbols.size()) return false;
        symbols = NineKeySidebarPolicy.moved(symbols, from, to);
        render();
        View handle = rowViews.get(to).handle();
        handle.post(() -> handle.performAccessibilityAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null));
        return true;
    }

    /** 手指在滚动区上下边缘带里时，下一帧滚一步并重新跟手；滚到尽头、被拖行到了列表首尾或离开边缘带就停。 */
    private void scheduleAutoScroll() {
        rows.removeCallbacks(autoScroll);
        if (edgeScrollStep() != 0) rows.postOnAnimation(autoScroll);
    }

    private void autoScrollStep() {
        if (dragged == null || scroller == null) return;
        int step = edgeScrollStep();
        if (step == 0) return;
        int before = scroller.getScrollY();
        scroller.scrollBy(0, step);
        if (scroller.getScrollY() == before) return;
        followFinger();
    }

    private int edgeScrollStep() {
        if (scroller == null || dragged == null) return 0;
        Context context = owner.requireContext();
        int top = screenTop(scroller);
        int step = NineKeySidebarPolicy.edgeScrollStep(lastRawY, top, top + scroller.getHeight(),
            DimensionPolicy.pixels(context, EDGE_DP), DimensionPolicy.pixels(context, EDGE_STEP_DP));
        // 下面还有操作卡片，滚动区往往还能滚；被拖行已经夹在「当前顺序」首尾时不再滚，免得列表带着它离开手指。
        LinearLayout row = dragged.row();
        step = NineKeySidebarPolicy.scrollTowardRoom(step, row.getTranslationY(), -row.getTop(),
            rows.getHeight() - row.getBottom());
        return scroller.canScrollVertically(step) ? step : 0;
    }

    /** 面板的滚动区（{@link SettingsSheet} 的 NestedScrollView）；找不到时不自动滚动。 */
    private NestedScrollView enclosingScroller() {
        for (ViewParent parent = rows.getParent(); parent != null; parent = parent.getParent()) {
            if (parent instanceof NestedScrollView scroll) return scroll;
        }
        return null;
    }

    private static int screenTop(View view) {
        int[] location = new int[2];
        view.getLocationOnScreen(location);
        return location[1];
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
