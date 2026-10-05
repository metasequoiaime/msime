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

/** Android equivalent of Apple's full-screen, categorized symbol surface. */
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
    private final List<Button> categoryButtons = new java.util.ArrayList<>();
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
        Button back = buttons.create("‹", "切换字母键盘", listener::close, true);
        back.setLayoutParams(new LinearLayout.LayoutParams(dp(56), dp(42)));
        title.addView(back);
        TextView heading = new TextView(context);
        heading.setText("符号");
        heading.setTextSize(17);
        heading.setGravity(Gravity.CENTER);
        title.addView(heading, new LinearLayout.LayoutParams(0, dp(42), 1));
        Button delete = buttons.create("⌫", "删除", listener::delete, true);
        delete.setLayoutParams(new LinearLayout.LayoutParams(dp(56), dp(42)));
        title.addView(delete);
        addView(title, new LinearLayout.LayoutParams(
            LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));

        LinearLayout body = new LinearLayout(context);
        body.setOrientation(HORIZONTAL);
        categories.setOrientation(VERTICAL);
        categories.setGravity(Gravity.CENTER);
        body.addView(categories, new LinearLayout.LayoutParams(dp(76), 0, 1));
        grid.setColumnCount(SymbolPanelModel.COLUMNS);
        grid.setUseDefaultMargins(false);
        grid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        gridScroll.setVerticalScrollBarEnabled(true);
        gridScroll.setContentDescription("符号网格；每行五个");
        gridScroll.addView(grid, new ScrollView.LayoutParams(
            LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT));
        body.addView(gridScroll, new LinearLayout.LayoutParams(0, 0, 4));
        addView(body, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, 0, 1));

        LinearLayout bottom = new LinearLayout(context);
        bottom.setOrientation(HORIZONTAL);
        Button bottomBack = buttons.create("返回", "返回键盘", listener::close, true);
        bottom.addView(bottomBack, new LinearLayout.LayoutParams(0, dp(48), 1));
        lockButton = buttons.create("锁定", "连续输入符号", this::toggleLock, true);
        bottom.addView(lockButton, new LinearLayout.LayoutParams(0, dp(48), 1));
        Button bottomDelete = buttons.create("⌫", "删除", listener::delete, true);
        bottom.addView(bottomDelete, new LinearLayout.LayoutParams(0, dp(48), 1));
        addView(bottom, new LinearLayout.LayoutParams(LayoutParams.MATCH_PARENT, dp(48)));

        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories();
        for (int index = 0; index < values.size(); index++) {
            final int category = index;
            Button button = buttons.create(values.get(index).title(),
                "符号分类 " + values.get(index).title(), () -> select(category), true);
            button.setGravity(Gravity.CENTER);
            categoryButtons.add(button);
            categories.addView(button, new LinearLayout.LayoutParams(
                LayoutParams.MATCH_PARENT, 0, 1));
        }
        select(0);
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
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
            int end = Math.min(start + SymbolPanelModel.COLUMNS, symbols.size());
            for (int index = start; index < end; index++) {
                String symbol = symbols.get(index);
                Button button = buttons.create(symbol, "符号 " + symbol,
                    () -> insert(symbol), false);
                button.setTextSize(18);
                button.setGravity(Gravity.CENTER);
                button.setPadding(0, 0, 0, 0);
                GridLayout.Spec row = GridLayout.spec(start / SymbolPanelModel.COLUMNS);
                GridLayout.Spec column = GridLayout.spec(index - start, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = dp(46);
                grid.addView(button, params);
            }
            for (int index = end - start; index < SymbolPanelModel.COLUMNS; index++) {
                View spacer = new View(getContext());
                GridLayout.Spec row = GridLayout.spec(start / SymbolPanelModel.COLUMNS);
                GridLayout.Spec column = GridLayout.spec(index, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = dp(46);
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
