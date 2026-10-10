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

/** 分类符号面板：盖在键区上（不盖顶部一行），左列分类、右侧网格（符号五列、颜文字两列），底部返回 / 锁定 / 删除；从 #+= 层的「符号」键进入。颜文字和后面那些分类从 Engine 目录分页读，滚到底再读下一页。 */
public final class SymbolPanelView extends LinearLayout {
    public interface ButtonFactory {
        Button create(String title, String description, Runnable action, boolean actionStyle);
    }

    public interface Listener {
        /** 上屏一个符号。`wholePair` 为 true 是轻点：成对符号的前半个按「自动补全成对标点」决定是否连后半个一起上屏；长按传 false，只上屏这半个。`remember` 为 false 时不记进「常用」（颜文字）。 */
        void insert(String text, boolean wholePair, boolean remember);
        /** 在工作线程上读目录分类 `category` 从 `offset` 起的一页，读完在主线程上调 `pages` 的一个方法。 */
        void loadCatalog(SymbolPanelModel.Category category, int offset, CatalogPages pages);
        void delete();
        void close();
        /** 按当前皮肤重画一个控件：按钮的选中状态变了时必须调，键帽颜色只在上色时读一次 `isSelected()`，只改选中状态的话高亮会一直停在上一次上色时的那个按钮上；换分类后新建的提示文字也要靠它拿到皮肤的字色。 */
        void restyle(View view);
    }

    /** 目录读页的结果；期间换了分类或重新打开了面板的话，面板自己丢掉过期的结果。 */
    public interface CatalogPages {
        void loaded(List<String> items, int nextOffset, boolean complete);
        void failed();
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
    /** 当前显示的目录分类读到哪了；换分类或重新打开时 `catalogGeneration` 加一，之前发出去的读页结果作废。 */
    private List<String> catalogItems = List.of();
    private int catalogNextOffset;
    private boolean catalogComplete;
    private boolean catalogLoading;
    private long catalogGeneration;

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
        gridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            if (scrollY > oldY && !view.canScrollVertically(1)) loadNextCatalogPage();
        });
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
        catalogGeneration++;
        catalogItems = List.of();
        catalogNextOffset = 0;
        catalogComplete = false;
        catalogLoading = false;
        SymbolPanelModel.Category current = values.get(category);
        grid.removeAllViews();
        grid.setColumnCount(current.columns());
        gridScroll.setContentDescription(current.kaomoji() ? "颜文字网格；每行两个" : "符号网格；每行五个");
        if (current.fromCatalog()) {
            showHint("正在加载…", current.columns());
            loadNextCatalogPage();
        } else if (current.symbols().isEmpty()) {
            showHint(SymbolPanelModel.RECENTS_EMPTY_HINT, current.columns());
        } else {
            appendRows(current, current.symbols(), 0);
        }
        gridScroll.scrollTo(0, 0);
    }

    /** 网格里只有一行提示文字（「常用」还没有记录、目录正在读或读不出来）。 */
    private void showHint(String text, int columns) {
        grid.removeAllViews();
        TextView hint = ViewPolicy.centeredText(getContext(), text, 14);
        KeyboardGeometry.setKeyTextSize(hint, 14);
        GridLayout.LayoutParams params = new GridLayout.LayoutParams(GridLayout.spec(0),
            GridLayout.spec(0, columns, 1f));
        params.width = 0;
        params.height = KeyboardGeometry.pixels(getContext(), 92);
        grid.addView(hint, params);
        listener.restyle(hint);
    }

    /** 把 `symbols` 里从 `from` 起的条目接在网格后面；`from` 必须是整行的开头。 */
    private void appendRows(SymbolPanelModel.Category category, List<String> symbols, int from) {
        int columns = category.columns();
        for (int start = from; start < symbols.size(); start += columns) {
            int end = BoundsPolicy.atMost(start + columns, symbols.size());
            for (int index = start; index < end; index++) {
                String symbol = symbols.get(index);
                boolean pairable = PairedPunctuationPolicy.symbolClosing(symbol) != null;
                boolean remember = category.remembers();
                Button button = buttons.create(symbol, "符号 " + symbol + (pairable ? "，长按只输入这半个" : ""),
                    () -> insert(symbol, true, remember), false);
                // 长按成对符号的前半个只上屏这半个（#5608）；处理了长按，系统会自己给一次长按震动。
                if (pairable) button.setOnLongClickListener(ignored -> {
                    insert(symbol, false, remember);
                    return true;
                });
                ViewPolicy.setCenteredKeyTextSizeSp(button, SymbolPanelModel.cellTextSizeSp(category, symbol));
                // 邮箱后缀这类长条目缩小字号后仍可能放不下，单行省略，不折成两行（#6147）。
                if (SymbolPanelModel.singleLineCell(category, symbol)) ViewPolicy.setSingleLineEllipsized(button);
                ViewPolicy.clearPadding(button);
                GridLayout.Spec row = GridLayout.spec(start / columns);
                GridLayout.Spec column = GridLayout.spec(index - start, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = KeyboardGeometry.pixels(getContext(), 46);
                grid.addView(button, params);
            }
            for (int index = end - start; index < columns; index++) {
                View spacer = new View(getContext());
                GridLayout.Spec row = GridLayout.spec(start / columns);
                GridLayout.Spec column = GridLayout.spec(index, 1f);
                GridLayout.LayoutParams params = new GridLayout.LayoutParams(row, column);
                params.width = 0;
                params.height = KeyboardGeometry.pixels(getContext(), 46);
                grid.addView(spacer, params);
            }
        }
    }

    /** 当前是目录分类、还没读完、也没有正在读时，读下一页。 */
    private void loadNextCatalogPage() {
        List<SymbolPanelModel.Category> values = SymbolPanelModel.categories(recents);
        if (selected < 0 || selected >= values.size()) return;
        SymbolPanelModel.Category category = values.get(selected);
        if (!category.fromCatalog() || catalogLoading || catalogComplete) return;
        catalogLoading = true;
        long generation = catalogGeneration;
        listener.loadCatalog(category, catalogNextOffset, new CatalogPages() {
            @Override public void loaded(List<String> items, int nextOffset, boolean complete) {
                catalogPageLoaded(generation, category, items, nextOffset, complete);
            }

            @Override public void failed() {
                if (generation != catalogGeneration) return;
                catalogLoading = false;
                catalogComplete = true;
                if (catalogItems.isEmpty()) showHint("目录暂时不可用；点分类重试", category.columns());
            }
        });
    }

    private void catalogPageLoaded(long generation, SymbolPanelModel.Category category,
            List<String> items, int nextOffset, boolean complete) {
        if (generation != catalogGeneration) return;
        catalogLoading = false;
        int shown = catalogItems.size();
        // 新的一页先补满上一页没排满的那一行：网格按行排，接着排就要从那一行的开头重排。
        int rowStart = shown - shown % category.columns();
        catalogItems = SymbolPanelModel.appendCatalogPage(catalogItems, items);
        catalogNextOffset = nextOffset;
        catalogComplete = complete || SymbolPanelModel.catalogFull(catalogItems);
        if (catalogItems.isEmpty()) {
            if (catalogComplete) showHint("暂无符号", category.columns());
            else loadNextCatalogPage();
            return;
        }
        if (shown == 0) grid.removeAllViews();
        else removeRowsFrom(rowStart, category.columns());
        appendRows(category, catalogItems, rowStart);
        // 一页没有铺满可见区域时滚不动，也就等不到滚到底，接着读。
        if (!catalogComplete) gridScroll.post(() -> {
            if (generation == catalogGeneration && !gridScroll.canScrollVertically(1)) loadNextCatalogPage();
        });
    }

    /** 去掉从第 `rowStart` 个条目所在行起的格子（包括补位的空白格），好让那一行重新排。 */
    private void removeRowsFrom(int rowStart, int columns) {
        int keep = rowStart / columns * columns;
        while (grid.getChildCount() > keep) grid.removeViewAt(grid.getChildCount() - 1);
    }

    private void insert(String symbol, boolean wholePair, boolean remember) {
        listener.insert(symbol, wholePair, remember);
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
