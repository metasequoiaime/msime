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
        /** 上屏一个符号。`wholePair` 为 true 是轻点：成对符号的前半个按「自动补全成对标点」决定是否连后半个一起上屏；长按传 false，只上屏这半个。 */
        void insert(String text, boolean wholePair);
        void delete();
        void close();
        /** 按当前皮肤重画一个控件：按钮的选中状态变了时必须调，键帽颜色只在上色时读一次 `isSelected()`，只改选中状态的话高亮会一直停在上一次上色时的那个按钮上；换分类后新建的提示文字也要靠它拿到皮肤的字色。 */
        void restyle(View view);
    }

    private final ButtonFactory buttons;
    private final Listener listener;
    private final LinearLayout categories;
    private final GridLayout grid = new GridLayout(getContext());
    private final ScrollView gridScroll = new ScrollView(getContext());
    private final List<Button> categoryButtons =
        new java.util.ArrayList<>(SymbolPanelModel.categories(List.of()).size());
    private final Button lockButton;
    private List<String> recents = List.of();
    private int selected;
    private boolean locked;

    /** 每次打开都回到单次输入；有使用记录时先显示「常用」，否则显示「中文」。 */
    public void resetForPresentation(List<String> recents) {
        this.recents = SymbolPanelModel.normalizeRecents(recents);
        locked = false;
        lockButton.setText("锁定");
        ViewPolicy.setSelected(lockButton, false);
        listener.restyle(lockButton);
        lockButton.setContentDescription("锁定，连续输入符号");
        select(SymbolPanelModel.initialCategory(this.recents));
    }

    /** 记录变了（刚上屏了一个符号）：只换数据，不重排正在看的网格，免得锁定连续输入时格子在手指底下挪动；下次点「常用」或重新打开时才按新顺序显示。 */
    public void setRecents(List<String> recents) {
        this.recents = SymbolPanelModel.normalizeRecents(recents);
    }

    public SymbolPanelView(Context context, ButtonFactory buttons, Listener listener) {
        super(context);
        this.buttons = buttons;
        this.listener = listener;
        categories = KeyboardGeometry.column(context);
        setOrientation(VERTICAL);
        setContentDescription("符号面板");
        setFocusable(true);

        LinearLayout title = KeyboardGeometry.row(context);
        ViewPolicy.setCenteredVertically(title);
        // 这个键只是关掉面板，回到打开它的那一层（字母、#+= 或手写），不一定是字母键盘。
        Button back = buttons.create("‹", "关闭符号面板", listener::close, true);
        back.setLayoutParams(KeyboardGeometry.linearParams(getContext(), 56, 42));
        title.addView(back);
        TextView heading = ViewPolicy.centeredText(context, "符号", 17);
        KeyboardGeometry.setKeyTextSize(heading, 17);
        title.addView(heading, KeyboardGeometry.weightedHeightParams(getContext(), 42, 1));
        Button delete = buttons.create("⌫", "删除", listener::delete, true);
        delete.setLayoutParams(KeyboardGeometry.linearParams(getContext(), 56, 42));
        title.addView(delete);
        addView(title, KeyboardGeometry.matchWidthWrapParams());

        LinearLayout body = KeyboardGeometry.row(context);
        ViewPolicy.setGravity(categories, Gravity.TOP);
        // 空白网格的根因：body 是横排 LinearLayout，权重只分宽度；这里和网格原先写的高度 0 是字面上的 0 像素，分类列和网格都被测成零高，面板中间于是什么都没有（面板本身又没底色，透出底下的字母键）。高度要铺满 body。
        // 分类列放进可滚动的容器、每类固定 40 dp：键盘区扣掉标题和底栏只剩百来 dp，五类按权重平分时每类二十来 dp，按钮默认的 48 dp 最小高度和内边距把字挤没了，只剩选中那块底色。
        ScrollView categoryScroll = new ScrollView(context);
        categoryScroll.setVerticalScrollBarEnabled(false);
        categoryScroll.addView(categories, KeyboardGeometry.scrollMatchWidthWrapParams());
        body.addView(categoryScroll, KeyboardGeometry.linearParamsPx(
            KeyboardGeometry.pixels(getContext(), 76), LayoutParams.MATCH_PARENT));
        grid.setColumnCount(SymbolPanelModel.COLUMNS);
        grid.setUseDefaultMargins(false);
        grid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        gridScroll.setVerticalScrollBarEnabled(true);
        gridScroll.setContentDescription("符号网格；每行五个");
        gridScroll.addView(grid, KeyboardGeometry.scrollMatchWidthWrapParams());
        body.addView(gridScroll, KeyboardGeometry.weightedMatchParentParams(1));
        addView(body, KeyboardGeometry.weightedWidthParams(1));

        LinearLayout bottom = KeyboardGeometry.row(context);
        Button bottomBack = buttons.create("返回", "返回键盘", listener::close, true);
        bottom.addView(bottomBack, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        lockButton = buttons.create("锁定", "连续输入符号", this::toggleLock, true);
        bottom.addView(lockButton, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        Button bottomDelete = buttons.create("⌫", "删除", listener::delete, true);
        bottom.addView(bottomDelete, KeyboardGeometry.weightedHeightParams(getContext(), 48, 1));
        addView(bottom, KeyboardGeometry.matchWidthHeightPx(KeyboardGeometry.pixels(getContext(), 48)));

        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories(recents);
        for (int index = 0; index < values.size(); index++) {
            final int category = index;
            Button button = buttons.create(values.get(index).title(),
                "符号分类 " + values.get(index).title(), () -> select(category), true);
            ViewPolicy.setCenteredTextSizeSp(button, 13);
            ViewPolicy.clearMinimumHeight(button);
            ViewPolicy.clearPadding(button);
            KeyboardGeometry.setKeyTextSize(button, 13);
            categoryButtons.add(button);
            categories.addView(button, KeyboardGeometry.matchWidthHeightPx(
                KeyboardGeometry.pixels(getContext(), 40)));
        }
        select(SymbolPanelModel.initialCategory(recents));
    }

    private void select(int category) {
        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories(recents);
        if (category < 0 || category >= values.size()) return;
        selected = category;
        for (int index = 0; index < categoryButtons.size(); index++) {
            Button button = categoryButtons.get(index);
            ViewPolicy.setSelected(button, index == selected);
            listener.restyle(button);
            button.setContentDescription("符号分类 " + values.get(index).title()
                + (index == selected ? "，已选中" : ""));
        }
        grid.removeAllViews();
        List<String> symbols = values.get(category).symbols();
        if (symbols.isEmpty()) {
            TextView hint = ViewPolicy.centeredText(getContext(), SymbolPanelModel.RECENTS_EMPTY_HINT, 14);
            KeyboardGeometry.setKeyTextSize(hint, 14);
            GridLayout.LayoutParams params = new GridLayout.LayoutParams(GridLayout.spec(0),
                GridLayout.spec(0, SymbolPanelModel.COLUMNS, 1f));
            params.width = 0;
            params.height = KeyboardGeometry.pixels(getContext(), 92);
            grid.addView(hint, params);
            listener.restyle(hint);
        }
        for (int start = 0; start < symbols.size(); start += SymbolPanelModel.COLUMNS) {
            int end = BoundsPolicy.atMost(start + SymbolPanelModel.COLUMNS, symbols.size());
            for (int index = start; index < end; index++) {
                String symbol = symbols.get(index);
                boolean pairable = PairedPunctuationPolicy.symbolClosing(symbol) != null;
                Button button = buttons.create(symbol, "符号 " + symbol + (pairable ? "，长按只输入这半个" : ""),
                    () -> insert(symbol, true), false);
                // 长按成对符号的前半个只上屏这半个（#5608）；处理了长按，系统会自己给一次长按震动。
                if (pairable) button.setOnLongClickListener(ignored -> {
                    insert(symbol, false);
                    return true;
                });
                ViewPolicy.setCenteredKeyTextSizeSp(button, 18);
                ViewPolicy.clearPadding(button);
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

    private void insert(String symbol, boolean wholePair) {
        listener.insert(symbol, wholePair);
        if (SymbolPanelModel.closesAfterInsert(locked)) listener.close();
    }

    private void toggleLock() {
        locked = !locked;
        lockButton.setText(locked ? "已锁定" : "锁定");
        ViewPolicy.setSelected(lockButton, locked);
        listener.restyle(lockButton);
        lockButton.setContentDescription(locked ? "已锁定，连续输入符号" : "锁定，连续输入符号");
    }
}
