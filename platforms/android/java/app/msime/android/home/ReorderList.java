package app.msime.android.home;

import android.annotation.SuppressLint;
import android.content.Context;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import androidx.recyclerview.widget.ItemTouchHelper;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.List;
import java.util.function.Function;

/**
 * 排序面板里的一列「序号. 名称 + 拖动把手」（#10392）：按住把手立即拖动，被拖的行浮起成圆角卡片跟着手指走，其余行让位，行首序号随位置实时改写；读屏用户在每一行的操作菜单里用「上移」「下移」，实体键盘和方向键用户让行拿到焦点后按 Alt+↑ / Alt+↓。
 *
 * <p>拖动交给 `ItemTouchHelper`：它逐格回调 {@link DragCallback#onMove}，草稿和列表都按「挪动」改（{@link ReorderPolicy#moved} 配 `notifyItemMoved`），屏幕上的顺序就是保存的顺序。拖动中只原地改写已绑定行的文字，不整表重建，被拖的行不会被销毁、手势不会断。
 *
 * <p>列表放在 {@link SettingsSheet} 的 `NestedScrollView` 里：自身不滚动（`setNestedScrollingEnabled(false)`、高度随内容），整张面板一起滚。拖动开始时 `ItemTouchHelper` 请父容器不拦截触摸，面板和滚动区都不会跟着手指走。
 */
final class ReorderList<T> {
    private static final int LIFT_MILLIS = 150;

    private final Context context;
    private final Function<T, String> label;
    private final RecyclerView view;
    private final RowAdapter adapter;
    private final ItemTouchHelper touch;
    private List<T> items;
    private boolean locked;

    /** @param label 一项的显示名，行首序号由这里加 */
    ReorderList(Context context, List<T> items, Function<T, String> label) {
        this.context = context;
        this.label = label;
        this.items = List.copyOf(items);
        view = new RecyclerView(context);
        view.setLayoutManager(new LinearLayoutManager(context));
        view.setNestedScrollingEnabled(false);
        view.setOverScrollMode(View.OVER_SCROLL_NEVER);
        adapter = new RowAdapter();
        view.setAdapter(adapter);
        touch = new ItemTouchHelper(new DragCallback());
        touch.attachToRecyclerView(view);
    }

    /** 加进面板的视图，宽度占满、高度随内容。 */
    View view() { return view; }

    /** 当前草稿顺序（不可变）。 */
    List<T> items() { return items; }

    /** 整表换成 `next`，例如「恢复默认」。 */
    @SuppressLint("NotifyDataSetChanged")
    void setItems(List<T> next) {
        items = List.copyOf(next);
        adapter.notifyDataSetChanged();
    }

    /** 保存进行中锁住排序：把手变灰、不能拖，读屏的「上移」「下移」也撤掉。 */
    void setLocked(boolean locked) {
        this.locked = locked;
        refreshRows();
    }

    private void move(int from, int to) {
        items = ReorderPolicy.moved(items, from, to);
        adapter.notifyItemMoved(from, to);
        refreshRows();
    }

    private boolean moveBy(Holder holder, int delta) {
        int from = holder.getBindingAdapterPosition();
        if (locked || from == RecyclerView.NO_POSITION || !ReorderPolicy.canMove(from, delta, items.size()))
            return false;
        move(from, from + delta);
        return true;
    }

    /** 实体键盘和方向键：焦点在行上时 Alt+↑ / Alt+↓ 挪一格，和读屏的「上移」「下移」走同一条路径。不带 Alt 的方向键照常在行间移动焦点。 */
    private boolean onKey(Holder holder, int keyCode, KeyEvent event) {
        if (event.getAction() != KeyEvent.ACTION_DOWN || !event.isAltPressed()) return false;
        int delta = keyCode == KeyEvent.KEYCODE_DPAD_UP ? -1 : keyCode == KeyEvent.KEYCODE_DPAD_DOWN ? 1 : 0;
        if (delta == 0) return false;
        moveBy(holder, delta);
        return true;
    }

    /** 按每一行此刻的位置改写序号、读屏描述和可用的操作；`notifyItemMoved` 之后位置已经是新的。 */
    private void refreshRows() {
        for (int index = 0; index < view.getChildCount(); index++) {
            Holder holder = (Holder) view.getChildViewHolder(view.getChildAt(index));
            int position = holder.getBindingAdapterPosition();
            if (position != RecyclerView.NO_POSITION) bind(holder, position);
        }
    }

    private void bind(Holder holder, int position) {
        String text = ReorderPolicy.ordinal(position, label.apply(items.get(position)));
        holder.title.setText(text);
        holder.itemView.setContentDescription(text);
        ViewPolicy.setEnabledWithAlpha(holder.handle, !locked, 0.38f);
        if (holder.upAction != View.NO_ID) ViewCompat.removeAccessibilityAction(holder.itemView, holder.upAction);
        if (holder.downAction != View.NO_ID) ViewCompat.removeAccessibilityAction(holder.itemView, holder.downAction);
        holder.upAction = !locked && ReorderPolicy.canMove(position, -1, items.size())
            ? ViewCompat.addAccessibilityAction(holder.itemView, "上移", (row, arguments) -> moveBy(holder, -1))
            : View.NO_ID;
        holder.downAction = !locked && ReorderPolicy.canMove(position, 1, items.size())
            ? ViewCompat.addAccessibilityAction(holder.itemView, "下移", (row, arguments) -> moveBy(holder, 1))
            : View.NO_ID;
    }

    /** 被拖的行：圆角卡片底色，再抬高一层投影。 */
    private void lift(View row) {
        ViewPolicy.setBackground(row, DrawablePolicy.rounded(ThemeColorPolicy.accentSoft(context),
            DimensionPolicy.pixels(context, 12)));
        row.animate().translationZ(DimensionPolicy.pixels(context, 6)).setDuration(LIFT_MILLIS);
    }

    private void drop(View row) {
        row.animate().cancel();
        row.setTranslationZ(0);
        ViewPolicy.setBackground(row, null);
    }

    @SuppressLint("ClickableViewAccessibility")
    private Holder newHolder() {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        ViewPolicy.setPaddingDp(context, row, 12, 0, 0, 0);
        // 整行是读屏的一个焦点，读「序号. 名称」，排序走这一行的「上移」「下移」；整行也是键盘焦点，排序走 Alt+↑ / Alt+↓（见 onKey）。
        row.setScreenReaderFocusable(true);
        row.setFocusable(true);
        TextView title = ViewPolicy.styledLabel(context, "", Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        row.addView(title, LayoutPolicy.weightedWrapParams(1));
        ImageView handle = ImageViewPolicy.iconButton(context, R.drawable.ic_ms_drag_handle,
            ThemeColorPolicy.subText(context), null, 48, null);
        // 把手只给手指用：读屏和键盘都按住拖不了，排序走整行的操作和按键，所以把手既不进读屏也不停键盘焦点。
        handle.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        handle.setFocusable(false);
        // 触控区与原来的箭头一样宽 48 dp、高占满整行；用 wrap 会缩成图标加内边距，约 43 dp。
        row.addView(handle, new LinearLayout.LayoutParams(DimensionPolicy.pixels(context, 48),
            LinearLayout.LayoutParams.MATCH_PARENT));
        row.setLayoutParams(new RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            DimensionPolicy.pixels(context, 48)));
        Holder holder = new Holder(row, title, handle);
        row.setOnKeyListener((focused, keyCode, event) -> onKey(holder, keyCode, event));
        // 按下把手就开始拖，不等长按；返回 false 让 RecyclerView 照常收到这次按下，ItemTouchHelper 靠它定起点。
        handle.setOnTouchListener((touched, event) -> {
            if (event.getActionMasked() == MotionEvent.ACTION_DOWN && !locked) touch.startDrag(holder);
            return false;
        });
        return holder;
    }

    private static final class Holder extends RecyclerView.ViewHolder {
        final TextView title;
        final ImageView handle;
        int upAction = View.NO_ID;
        int downAction = View.NO_ID;

        Holder(View row, TextView title, ImageView handle) {
            super(row);
            this.title = title;
            this.handle = handle;
        }
    }

    private final class RowAdapter extends RecyclerView.Adapter<Holder> {
        @NonNull @Override public Holder onCreateViewHolder(@NonNull ViewGroup parent, int viewType) {
            return newHolder();
        }

        @Override public void onBindViewHolder(@NonNull Holder holder, int position) { bind(holder, position); }

        @Override public int getItemCount() { return items.size(); }
    }

    private final class DragCallback extends ItemTouchHelper.Callback {
        @Override public int getMovementFlags(@NonNull RecyclerView list, @NonNull RecyclerView.ViewHolder holder) {
            return makeMovementFlags(locked ? 0 : ItemTouchHelper.UP | ItemTouchHelper.DOWN, 0);
        }

        @Override public boolean isLongPressDragEnabled() { return false; }

        @Override public boolean isItemViewSwipeEnabled() { return false; }

        @Override public boolean onMove(@NonNull RecyclerView list, @NonNull RecyclerView.ViewHolder source,
                @NonNull RecyclerView.ViewHolder target) {
            int from = source.getBindingAdapterPosition();
            int to = target.getBindingAdapterPosition();
            if (locked || from == RecyclerView.NO_POSITION || to == RecyclerView.NO_POSITION) return false;
            move(from, to);
            return true;
        }

        // 滑动方向的标志是 0，这个回调不会来。
        @Override public void onSwiped(@NonNull RecyclerView.ViewHolder holder, int direction) {}

        @Override public void onSelectedChanged(@Nullable RecyclerView.ViewHolder holder, int actionState) {
            super.onSelectedChanged(holder, actionState);
            if (actionState == ItemTouchHelper.ACTION_STATE_DRAG && holder != null) lift(holder.itemView);
        }

        @Override public void clearView(@NonNull RecyclerView list, @NonNull RecyclerView.ViewHolder holder) {
            super.clearView(list, holder);
            drop(holder.itemView);
        }
    }
}
