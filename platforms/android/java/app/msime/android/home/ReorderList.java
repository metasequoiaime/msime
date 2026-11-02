package app.msime.android.home;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.drawable.Drawable;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewParent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.animation.DecelerateInterpolator;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.core.view.ViewCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import java.util.function.IntConsumer;

/**
 * 排序面板里一列可拖动排序的行的控制器（#10392、#10546），「文本编辑按键顺序」「符号分类顺序」「按钮顺序」和「九键左侧符号」编辑页共用：按住把手立即拖动，被拖的行浮起跟着手指走，其余行滑开让位，序号随位次实时变化，贴近面板上下边缘时面板自动滚动；读屏用户在把手的操作菜单里用「上移」「下移」，实体键盘和方向键用户让把手拿到焦点后按 Alt+↑ / Alt+↓。
 *
 * <p>控制器只管拖动，不管行里画什么：行由调用方的 {@link RowFactory} 画好，交回整行、行里的把手（用 {@link #addHandle} 加进行里）和一个按位次改写可见序号与读屏描述的回调。只有名称的行用 {@link #ReorderList(Context, List, Function)}，画成「序号. 名称 + 把手」。
 *
 * <p>行的视图在拖动中留在原位、只改 translationY，落点按拿起时各行的布局中线算（{@link ReorderPolicy#dragTarget}），松手才把草稿按 {@link ReorderPolicy#moved} 挪好、重画。列表放在 {@link SettingsSheet} 的 `NestedScrollView` 里、自身不滚动；手指停在滚动区上下边缘带里时每帧滚动那个 `NestedScrollView`，滚过的距离算进跟手位移，被拖行不离开手指。读屏模型：可见的序号不念，行的读屏描述念「第 N 项，名称」（由调用方的回调写），把手的状态念「第 N 项」，「上移」「下移」挂在把手上。
 */
final class ReorderList<T> {
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

    /**
     * 调用方画好的一行。
     *
     * @param row 整行，拖动时浮起、平移的就是它
     * @param params 整行加进列表时的布局参数，行高由调用方定
     * @param handle 行里的拖动把手（{@link #addHandle} 加的那个），触摸拖动、读屏操作、位次状态和 Alt+↑ / Alt+↓ 都挂在它上面
     * @param renumber 按这一行此刻的位次（从 0 数）改写可见序号和读屏描述；拖动途中随落点反复调用
     */
    record Row(LinearLayout row, LinearLayout.LayoutParams params, ImageView handle, IntConsumer renumber) {}

    /** 画第 `index` 项的一行。整表变了或锁定状态变了都会重画，每次重画逐项调用一次。 */
    interface RowFactory<T> {
        Row create(T item, int index);
    }

    private final Context context;
    private final RowFactory<T> factory;
    private final LinearLayout rows;
    private final List<Row> rowViews = new ArrayList<>();
    private final Runnable autoScroll = this::autoScrollStep;
    private List<T> items;
    private boolean locked;
    /** 正在拖的那一行，没有拖动时为 null；拖动中 {@link #dragFrom} 是它在表里的原位，{@link #dragTo} 是此刻的落点。 */
    private Row dragged;
    private int dragFrom;
    private int dragTo;
    private float downRawY;
    private float lastRawY;
    private int rowsTopAtDown;
    /** 拿起时各行在布局里的中线，判断落点只看它们。 */
    private float[] centers = new float[0];
    private NestedScrollView scroller;

    /**
     * 每行是「序号. 名称 + 把手」，行高 48 dp。
     *
     * @param label 一项的显示名，行首序号由这里加
     */
    ReorderList(Context context, List<T> items, Function<T, String> label) {
        this(context, items, labelled(context, label));
    }

    /** @param factory 画行；行里的序号、读屏描述和其余控件都由它管 */
    ReorderList(Context context, List<T> items, RowFactory<T> factory) {
        this.context = context;
        this.factory = factory;
        this.items = List.copyOf(items);
        rows = LayoutPolicy.column(context);
        render();
    }

    /** 往行里加一个拖动把手并返回它：读屏描述是「拖动调整 名称 的顺序」，触控区与原来的箭头一样宽 48 dp、高占满整行；用 wrap 会缩成图标加内边距，约 43 dp。 */
    static ImageView addHandle(Context context, LinearLayout row, String label) {
        ImageView handle = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_drag_handle,
            ThemeColorPolicy.subText(context), "拖动调整 " + label + " 的顺序", 48, null);
        row.addView(handle, new LinearLayout.LayoutParams(DimensionPolicy.pixels(context, 48),
            LinearLayout.LayoutParams.MATCH_PARENT));
        return handle;
    }

    /** 只有名称的一行：「1. 复制」加把手。可见的序号不念，名称念「第 1 项，复制」。 */
    private static <T> RowFactory<T> labelled(Context context, Function<T, String> label) {
        return (item, index) -> {
            String text = label.apply(item);
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            ViewPolicy.setPaddingDp(context, row, 12, 0, 0, 0);
            TextView title = ViewPolicy.styledLabel(context, "", Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
            row.addView(title, LayoutPolicy.weightedWrapParams(1));
            ImageView handle = addHandle(context, row, text);
            return new Row(row, LayoutPolicy.matchWidthHeightDp(context, 48), handle, position -> {
                title.setText(ReorderPolicy.ordinal(position, text));
                title.setContentDescription(ReorderPolicy.ordinalDescription(position, text));
            });
        };
    }

    /** 加进面板的视图，宽度占满、高度随内容。 */
    View view() { return rows; }

    /** 当前草稿顺序（不可变）。 */
    List<T> items() { return items; }

    /** 整表换成 `next`，例如「恢复默认」、删除或添加一项。 */
    void setItems(List<T> next) {
        items = List.copyOf(next);
        render();
    }

    /** 保存进行中锁住排序：把手变灰、不能拖，读屏的「上移」「下移」也撤掉。 */
    void setLocked(boolean locked) {
        this.locked = locked;
        render();
    }

    private void render() {
        // 重画时作废进行中的拖动（多指拖动时另一指点了保存或重置也走到这里）。
        dragged = null;
        rows.removeCallbacks(autoScroll);
        rows.removeAllViews();
        rowViews.clear();
        for (int index = 0; index < items.size(); index++) addRow(index);
        renumber();
    }

    @SuppressLint("ClickableViewAccessibility")
    private void addRow(int index) {
        Row views = factory.create(items.get(index), index);
        ImageView handle = views.handle();
        ViewPolicy.setEnabledWithAlpha(handle, !locked, 0.38f);
        rows.addView(views.row(), views.params());
        rowViews.add(views);
        handle.setOnTouchListener((touched, event) -> handleDrag(views, event));
        // 把手只有触摸拖动；读屏另给「上移」「下移」两个操作，实体键盘在把手上按 Alt+↑ / Alt+↓（见 onKey），三条路径同一个落点。
        handle.setOnKeyListener((focused, keyCode, event) -> onKey(views, keyCode, event));
        if (!locked && ReorderPolicy.canMove(index, -1, items.size()))
            ViewCompat.addAccessibilityAction(handle, "上移", (view, arguments) -> moveBy(views, -1));
        if (!locked && ReorderPolicy.canMove(index, 1, items.size()))
            ViewCompat.addAccessibilityAction(handle, "下移", (view, arguments) -> moveBy(views, 1));
    }

    /** 按每行此刻的位次写序号和读屏描述：平时就是行号，拖动途中按 {@link ReorderPolicy#dragSlot} 跟着手指变。 */
    private void renumber() {
        for (int index = 0; index < rowViews.size(); index++) {
            Row views = rowViews.get(index);
            int position = dragged == null ? index : ReorderPolicy.dragSlot(index, dragFrom, dragTo);
            views.renumber().accept(position);
            ViewCompat.setStateDescription(views.handle(), ReorderPolicy.positionDescription(position));
        }
    }

    /** 实体键盘和方向键：焦点在把手上时 Alt+↑ / Alt+↓ 挪一格，和读屏的「上移」「下移」走同一条路径。不带 Alt 的方向键照常移动焦点。 */
    private boolean onKey(Row views, int keyCode, KeyEvent event) {
        if (event.getAction() != KeyEvent.ACTION_DOWN || !event.isAltPressed()) return false;
        int delta = keyCode == KeyEvent.KEYCODE_DPAD_UP ? -1 : keyCode == KeyEvent.KEYCODE_DPAD_DOWN ? 1 : 0;
        if (delta == 0) return false;
        moveBy(views, delta);
        return true;
    }

    /** 读屏的「上移」「下移」和 Alt+↑ / Alt+↓：挪一格后重画，键盘焦点和读屏焦点都放回挪过去的那一行的把手上，读屏念出新的位次。 */
    private boolean moveBy(Row views, int delta) {
        int from = rowViews.indexOf(views);
        if (locked || dragged != null || from < 0 || !ReorderPolicy.canMove(from, delta, items.size())) return false;
        boolean focused = views.handle().isFocused();
        items = ReorderPolicy.moved(items, from, from + delta);
        render();
        View handle = rowViews.get(from + delta).handle();
        if (focused) handle.requestFocus();
        handle.post(() -> handle.performAccessibilityAction(AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null));
        return true;
    }

    private boolean handleDrag(Row views, MotionEvent event) {
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

    /** 按住把手拿起这一行：行浮起（不透明底色加阴影，盖在别的行上面），之后跟着手指走；请父容器不拦截触摸，面板和滚动区不会跟着手指滑走。 */
    private boolean startDrag(Row views, float rawY) {
        if (locked || dragged != null || views.row().getParent() != rows) return false;
        int from = rowViews.indexOf(views);
        if (from < 0) return false;
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
        // 不拖出列表：拖到首尾就夹住。
        offset = Math.max(-row.getTop(), Math.min(rows.getHeight() - row.getBottom(), offset));
        row.setTranslationY(offset);
        int target = ReorderPolicy.dragTarget(centers[dragFrom] + offset, dragFrom, centers);
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
            float shift = (ReorderPolicy.dragSlot(index, dragFrom, dragTo) - index) * height;
            View row = rowViews.get(index).row();
            if (row.getTranslationY() == shift) continue;
            row.animate().translationY(shift).setDuration(ANIMATION_MILLIS)
                .setInterpolator(new DecelerateInterpolator()).start();
        }
    }

    /** 松手：草稿按挪动语义改好再重画，其余行此刻已经在新位置上，重画后不动；被拖行从手指处滑进自己的格子、落下阴影。 */
    private void drop() {
        Row views = dragged;
        int from = dragFrom;
        int to = dragTo;
        rows.removeCallbacks(autoScroll);
        rows.requestDisallowInterceptTouchEvent(false);
        float height = views.row().getHeight();
        float remaining = views.row().getTranslationY() - (to - from) * height;
        float lift = views.row().getTranslationZ();
        dragged = null;
        items = ReorderPolicy.moved(items, from, to);
        render();
        if (to >= rowViews.size()) return;
        LinearLayout landed = rowViews.get(to).row();
        // 落定后换回调用方给行的底色（没有底色，或点按的水波纹），浮起用的圆角卡片只在动画里出现。
        Drawable resting = landed.getBackground();
        ViewPolicy.setBackground(landed, DrawablePolicy.rounded(ThemeColorPolicy.rowBackground(context),
            DimensionPolicy.pixels(context, LIFTED_RADIUS_DP)));
        landed.setTranslationY(remaining);
        landed.setTranslationZ(lift);
        landed.animate().translationY(0).translationZ(0).setDuration(ANIMATION_MILLIS)
            .setInterpolator(new DecelerateInterpolator())
            .withEndAction(() -> ViewPolicy.setBackground(landed, resting)).start();
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
        int top = screenTop(scroller);
        int step = ReorderPolicy.edgeScrollStep(lastRawY, top, top + scroller.getHeight(),
            DimensionPolicy.pixels(context, EDGE_DP), DimensionPolicy.pixels(context, EDGE_STEP_DP));
        // 下面还有操作卡片，滚动区往往还能滚；被拖行已经夹在列表首尾时不再滚，免得列表带着它离开手指。
        LinearLayout row = dragged.row();
        step = ReorderPolicy.scrollTowardRoom(step, row.getTranslationY(), -row.getTop(),
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
}
