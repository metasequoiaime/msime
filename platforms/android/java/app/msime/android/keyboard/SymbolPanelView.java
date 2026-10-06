package app.msime.android;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.GridLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import java.util.List;

/** 分类符号面板：盖在键区上（不盖顶部一行），左列分类、右侧五列网格，底部返回 / 锁定 / 删除；从 #+= 层的「符号」键进入。 */
public final class SymbolPanelView extends LinearLayout {
    public interface ButtonFactory {
        Button create(String title, String description, Runnable action, boolean actionStyle);
    }

    public interface Listener {
        void insert(String text);
        void delete();
        void close();
    }

    private final ButtonFactory buttons;
    private final Listener listener;
    private final LinearLayout categories = new LinearLayout(getContext());
    private final GridLayout grid = new GridLayout(getContext());
    private final ScrollView gridScroll = new ScrollView(getContext());
    private final List<Button> categoryButtons =
        new java.util.ArrayList<>(SymbolPanelModel.categories().size());
    private final Button lockButton;
    private int selected;
    private boolean locked;

    /** Reopens with Apple's default category and one-shot insertion behavior. */
    public void resetForPresentation() {
        locked = false;
        lockButton.setText("锁定");
        lockButton.setSelected(false);
        lockButton.setContentDescription("锁定，连续输入符号");
        select(0);
    }

    public SymbolPanelView(Context context, ButtonFactory buttons, Listener listener) {
        super(context);
        this.buttons = buttons;
        this.listener = listener;
        setOrientation(VERTICAL);
        setContentDescription("符号面板");
        setFocusable(true);

        LinearLayout title = new LinearLayout(context);
        title.setGravity(Gravity.CENTER_VERTICAL);
        // 这个键只是关掉面板，回到打开它的那一层（字母、#+= 或手写），不一定是字母键盘。
        Button back = buttons.create("‹", "关闭符号面板", listener::close, true);
        back.setLayoutParams(KeyboardGeometry.linearParams(getContext(), 56, 42));
        title.addView(back);
        TextView heading = new TextView(context);
        heading.setText("符号");
        KeyboardGeometry.setKeyTextSize(heading, 17);
        heading.setGravity(Gravity.CENTER);
        title.addView(heading, KeyboardGeometry.weightedHeightParams(getContext(), 42, 1));
        Button delete = buttons.create("⌫", "删除", listener::delete, true);
        delete.setLayoutParams(KeyboardGeometry.linearParams(getContext(), 56, 42));
        title.addView(delete);
        addView(title, KeyboardGeometry.matchWidthWrapParams());

        LinearLayout body = new LinearLayout(context);
        body.setOrientation(HORIZONTAL);
        categories.setOrientation(VERTICAL);
        categories.setGravity(Gravity.TOP);
        // 空白网格的根因：body 是横排 LinearLayout，权重只分宽度；这里和网格原先写的高度 0 是字面上的 0 像素，分类列和网格都被测成零高，面板中间于是什么都没有（面板本身又没底色，透出底下的字母键）。高度要铺满 body。
        // 分类列放进可滚动的容器、每类固定 40 dp：键盘区扣掉标题和底栏只剩百来 dp，五类按权重平分时每类二十来 dp，按钮默认的 48 dp 最小高度和内边距把字挤没了，只剩选中那块底色。
        ScrollView categoryScroll = new ScrollView(context);
        categoryScroll.setVerticalScrollBarEnabled(false);
        categoryScroll.addView(categories, new ScrollView.LayoutParams(
            LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));
        body.addView(categoryScroll, new LinearLayout.LayoutParams(KeyboardGeometry.pixels(getContext(), 76), LayoutParams.MATCH_PARENT));
        grid.setColumnCount(SymbolPanelModel.COLUMNS);
        grid.setUseDefaultMargins(false);
        grid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        gridScroll.setVerticalScrollBarEnabled(true);
        gridScroll.setContentDescription("符号网格；每行五个");
        gridScroll.addView(grid, new ScrollView.LayoutParams(
            LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));
        body.addView(gridScroll, KeyboardGeometry.weightedMatchParentParams(1));
        addView(body, KeyboardGeometry.weightedWidthParams(1));

        LinearLayout bottom = new LinearLayout(context);
        bottom.setOrientation(HORIZONTAL);
        Button bottomBack = buttons.create("返回", "返回键盘", listener::close, true);
        bottom.addView(bottomBack, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        lockButton = buttons.create("锁定", "连续输入符号", this::toggleLock, true);
        bottom.addView(lockButton, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        Button bottomDelete = buttons.create("⌫", "删除", listener::delete, true);
        bottom.addView(bottomDelete, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        addView(bottom, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, KeyboardGeometry.pixels(getContext(), 48)));

        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories();
        for (int index = 0; index < values.size(); index++) {
            final int category = index;
            Button button = buttons.create(values.get(index).title(),
                "符号分类 " + values.get(index).title(), () -> select(category), true);
            button.setGravity(Gravity.CENTER);
            button.setMinHeight(0);
            button.setMinimumHeight(0);
            button.setPadding(0, 0, 0, 0);
            KeyboardGeometry.setKeyTextSize(button, 13);
            categoryButtons.add(button);
            categories.addView(button, new LinearLayout.LayoutParams(
                LayoutParams.MATCH_PARENT, KeyboardGeometry.pixels(getContext(), 40)));
        }
        select(0);
    }

    private void select(int category) {
        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories();
        if (category < 0 || category >= values.size()) return;
        selected = category;
        for (int index = 0; index < categoryButtons.size(); index++) {
            Button button = categoryButtons.get(index);
            button.setSelected(index == selected);
            button.setContentDescription("符号分类 " + values.get(index).title()
                + (index == selected ? "，已选中" : ""));
        }
        grid.removeAllViews();
        List<String> symbols = values.get(category).symbols();
        for (int start = 0; start < symbols.size(); start += SymbolPanelModel.COLUMNS) {
            int end = BoundsPolicy.atMost(start + SymbolPanelModel.COLUMNS, symbols.size());
            for (int index = start; index < end; index++) {
                String symbol = symbols.get(index);
                Button button = buttons.create(symbol, "符号 " + symbol,
                    () -> insert(symbol), false);
                KeyboardGeometry.setKeyTextSize(button, 18);
                button.setGravity(Gravity.CENTER);
                button.setPadding(0, 0, 0, 0);
                GridLayout.Spec row = GridLayout.spec(start / SymbolPanelModel.COLUMNS);
                GridLayout.Spec column = GridLayout.spec(index - start, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = KeyboardGeometry.pixels(getContext(), 46);
                grid.addView(button, params);
            }
            for (int index = end - start; index < SymbolPanelModel.COLUMNS; index++) {
                View spacer = new View(getContext());
                GridLayout.Spec row = GridLayout.spec(start / SymbolPanelModel.COLUMNS);
                GridLayout.Spec column = GridLayout.spec(index, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = KeyboardGeometry.pixels(getContext(), 46);
                grid.addView(spacer, params);
            }
        }
        gridScroll.scrollTo(0, 0);
    }

    private void insert(String symbol) {
        listener.insert(symbol);
        if (SymbolPanelModel.closesAfterInsert(locked)) listener.close();
    }

    private void toggleLock() {
        locked = !locked;
        lockButton.setText(locked ? "已锁定" : "锁定");
        lockButton.setSelected(locked);
        lockButton.setContentDescription(locked ? "已锁定，连续输入符号" : "锁定，连续输入符号");
    }
}
