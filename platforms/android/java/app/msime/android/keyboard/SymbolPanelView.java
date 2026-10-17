package app.msime.android;

import app.msime.android.LayoutPolicy;


import android.annotation.SuppressLint;
import android.content.Context;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import android.widget.GridLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import java.util.List;

/** 分类符号面板：全宽网格上下滚动，底栏分类左右滚动，左上返回、右下删除。目录分类从 Engine 分页读，滚到底再读下一页。 */
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
    private final GridLayout grid = new GridLayout(getContext()) {
        @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
            super.onMeasure(widthMeasureSpec, heightMeasureSpec);
            int width = getMeasuredWidth();
            if (width <= 0 || width == measuredGridWidth) return;
            measuredGridWidth = width;
            SymbolPanelModel.Category category = orderedCategories().get(selected);
            if (!category.fromCatalog() && !category.symbols().isEmpty()) {
                // 测量期间按新宽度重排并再次测量，确保本轮布局拿到每个键的尺寸。
                // 若等布局完成后才替换子视图，新键没有经过测量，首次打开就会整片空白。
                removeAllViews();
                appendRows(category, category.symbols(), 0);
                super.onMeasure(widthMeasureSpec, heightMeasureSpec);
            }
        }
    };
    private int measuredGridWidth;
    private final ScrollView gridScroll = new CategorySwipeScrollView(getContext());
    private final HorizontalScrollView categoryScroll = new HorizontalScrollView(getContext()) {
        @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
            super.onSizeChanged(width, height, oldWidth, oldHeight);
            post(SymbolPanelView.this::revealSelectedCategory);
        }
    };
    private final List<Button> categoryButtons =
        new java.util.ArrayList<>(SymbolPanelModel.categories(List.of()).size());
    private List<SymbolCategoryOrderPolicy.Item> categoryOrder = SymbolCategoryOrderPolicy.DEFAULT_ORDER;
    private List<String> recents = List.of();
    private int selected;
    /** 当前显示的目录分类读到哪了；换分类或重新打开时 `catalogGeneration` 加一，之前发出去的读页结果作废。 */
    private List<String> catalogItems = List.of();
    private int catalogNextOffset;
    private boolean catalogComplete;
    private boolean catalogLoading;
    private long catalogGeneration;

    /** 每次打开时，有使用记录先显示「常用」，否则显示「中文」。 */
    public void resetForPresentation(List<String> recents) {
        resetForPresentation(recents, categoryOrder);
    }

    /** 每次打开时同时应用本机分类顺序；「常用」移动后仍按身份选中它。 */
    public void resetForPresentation(List<String> recents,
            List<SymbolCategoryOrderPolicy.Item> order) {
        this.recents = SymbolPanelModel.normalizeRecents(recents);
        setCategoryOrder(order);
        select(SymbolPanelModel.initialCategory(this.recents, categoryOrder));
    }

    /** 设置页保存后无需重启输入法，下一次打开面板即可看到新顺序。 */
    public void setCategoryOrder(List<SymbolCategoryOrderPolicy.Item> order) {
        List<SymbolCategoryOrderPolicy.Item> normalized = order == null
            ? SymbolCategoryOrderPolicy.DEFAULT_ORDER : List.copyOf(order);
        if (categoryOrder.equals(normalized)) return;
        String selectedId = null;
        List<SymbolPanelModel.Category> before = orderedCategories();
        if (selected >= 0 && selected < before.size()) selectedId = before.get(selected).id();
        categoryOrder = normalized;
        rebuildCategoryButtons();
        List<SymbolPanelModel.Category> after = orderedCategories();
        int next = 0;
        if (selectedId != null) for (int index = 0; index < after.size(); index++)
            if (selectedId.equals(after.get(index).id())) { next = index; break; }
        selected = next;
    }

    /** 记录变了只换数据，不重排正在看的网格，避免连续输入时格子挪动；下次点「常用」或重新打开时才按新顺序显示。 */
    public void setRecents(List<String> recents) {
        this.recents = SymbolPanelModel.normalizeRecents(recents);
    }

    public SymbolPanelView(Context context, ButtonFactory buttons, Listener listener) {
        super(context);
        this.buttons = buttons;
        this.listener = listener;
        categories = LayoutPolicy.row(context);
        setOrientation(VERTICAL);
        int panelPadding = DimensionPolicy.pixels(context, 4);
        setPadding(panelPadding, panelPadding, panelPadding, panelPadding);
        setContentDescription("符号面板");
        setFocusable(true);

        LinearLayout title = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(title);
        // 这个键只是关掉面板，回到打开它的那一层（字母、#+= 或手写），不一定是字母键盘。
        Button back = buttons.create("‹", "关闭符号面板", listener::close, true);
        styleHeaderButton(back);
        back.setLayoutParams(LayoutPolicy.linearParams(getContext(), 44, 44));
        title.addView(back);
        addView(title, LayoutPolicy.matchWidthWrapParams());

        grid.setColumnCount(SymbolPanelModel.COLUMNS);
        grid.setUseDefaultMargins(false);
        grid.setAlignmentMode(GridLayout.ALIGN_BOUNDS);
        gridScroll.setVerticalScrollBarEnabled(true);
        gridScroll.setContentDescription("符号网格；每行五个");
        gridScroll.setOnScrollChangeListener((view, scrollX, scrollY, oldX, oldY) -> {
            if (scrollY > oldY && !view.canScrollVertically(1)) loadNextCatalogPage();
        });
        gridScroll.addView(grid, LayoutPolicy.scrollMatchWidthWrapParams());
        addView(gridScroll, LayoutPolicy.weightedWidthParams(1));

        LinearLayout bottom = LayoutPolicy.row(context);
        categoryScroll.setHorizontalScrollBarEnabled(false);
        categoryScroll.setContentDescription("符号分类栏，左右滑动查看更多分类");
        categoryScroll.addView(categories, LayoutPolicy.frameWrapParams());
        bottom.addView(categoryScroll, LayoutPolicy.weightedMatchParentParams(1));
        Button bottomDelete = buttons.create("⌫", "删除", listener::delete, true);
        styleFooterButton(bottomDelete, 20);
        bottom.addView(bottomDelete, footerKeyParams(48));
        addView(bottom, LayoutPolicy.matchWidthHeightPx(DimensionPolicy.pixels(getContext(), 48)));

        rebuildCategoryButtons();
        select(SymbolPanelModel.initialCategory(recents, categoryOrder));
    }

    private List<SymbolPanelModel.Category> orderedCategories() {
        return SymbolPanelModel.categories(recents, categoryOrder);
    }

    private void rebuildCategoryButtons() {
        categories.removeAllViews();
        categoryButtons.clear();
        List<SymbolPanelModel.Category> values = orderedCategories();
        for (int index = 0; index < values.size(); index++) {
            final int category = index;
            Button button = buttons.create(values.get(index).title(),
                "符号分类 " + values.get(index).title(), () -> select(category), true);
            ViewPolicy.setCenteredKeyTextSizeSp(button, 13);
            KeyboardTypographyPolicy.normalizeKeyCap(button);
            ViewPolicy.clearPadding(button);
            if (button instanceof KeyboardPressButton key) key.setKeyboardRole(KeyboardKeyRole.PLAIN);
            listener.restyle(button);
            categoryButtons.add(button);
            button.setMinWidth(DimensionPolicy.pixels(getContext(), 56));
            categories.addView(button, footerKeyParams(LayoutParams.WRAP_CONTENT));
        }
    }

    /** 返回用无键帽图标，底栏只保留固定删除与可滚动分类。 */
    private void styleHeaderButton(Button button) {
        KeyboardTypographyPolicy.normalizeKeyCap(button);
        ViewPolicy.clearPadding(button);
        ViewPolicy.setCenteredKeyTextSizeSp(button, 20);
        if (button instanceof KeyboardPressButton key) key.setKeyboardRole(KeyboardKeyRole.GLYPH);
        listener.restyle(button);
    }

    private void styleFooterButton(Button button, float textSize) {
        KeyboardTypographyPolicy.normalizeKeyCap(button);
        ViewPolicy.clearPadding(button);
        ViewPolicy.setCenteredKeyTextSizeSp(button, textSize);
    }

    /** 底栏与符号网格共用 6 dp 的键间距，圆角键帽不再连成一整块。 */
    private LinearLayout.LayoutParams footerKeyParams(int widthDp) {
        LinearLayout.LayoutParams params = LayoutPolicy.linearParamsPx(widthDp < 0 ? widthDp
            : DimensionPolicy.pixels(getContext(), widthDp), DimensionPolicy.pixels(getContext(), 42));
        int margin = DimensionPolicy.pixels(getContext(), 3);
        params.setMargins(margin, margin, margin, margin);
        return params;
    }

    private void select(int category) {
        List<SymbolPanelModel.Category> values = orderedCategories();
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
        gridScroll.setContentDescription(current.kaomoji() ? "颜文字网格；每行两个" : "符号网格；长条目加宽");
        if (current.fromCatalog()) {
            showHint("正在加载…", current.columns());
            loadNextCatalogPage();
        } else if (current.symbols().isEmpty()) {
            showHint(SymbolPanelModel.RECENTS_EMPTY_HINT, current.columns());
        } else {
            appendRows(current, current.symbols(), 0);
        }
        gridScroll.scrollTo(0, 0);
        categoryScroll.post(this::revealSelectedCategory);
    }

    private void revealSelectedCategory() {
        if (selected < 0 || selected >= categoryButtons.size()) return;
        Button button = categoryButtons.get(selected);
        int left = categoryScroll.getScrollX();
        int right = left + categoryScroll.getWidth();
        if (button.getLeft() < left) categoryScroll.scrollTo(button.getLeft(), 0);
        else if (button.getRight() > right)
            categoryScroll.scrollTo(button.getRight() - categoryScroll.getWidth(), 0);
    }

    /** 横划先取消子键点击，纵划交给系统滚动；一次手势只选定一个方向。 */
    private final class CategorySwipeScrollView extends ScrollView {
        private final int touchSlop;
        private float downX;
        private float downY;
        private int axis;
        private boolean multiplePointers;

        CategorySwipeScrollView(Context context) {
            super(context);
            touchSlop = ViewConfiguration.get(context).getScaledTouchSlop();
        }

        @Override public boolean dispatchTouchEvent(MotionEvent event) {
            if (event.getActionMasked() == MotionEvent.ACTION_DOWN) {
                downX = event.getX();
                downY = event.getY();
                axis = 0;
                multiplePointers = false;
            } else if (event.getActionMasked() == MotionEvent.ACTION_POINTER_DOWN) {
                multiplePointers = true;
            } else if (event.getActionMasked() == MotionEvent.ACTION_MOVE && axis == 0) {
                float dx = Math.abs(event.getX() - downX);
                float dy = Math.abs(event.getY() - downY);
                if (dx > touchSlop && dx > dy * 1.5f) axis = 1;
                else if (dy > touchSlop) axis = 2;
            }
            if (axis == 1) {
                // 子键禁止拦截后仍需撤掉横划中的点击，同时让更外层继续留在本面板。
                super.requestDisallowInterceptTouchEvent(false);
                if (getParent() != null) getParent().requestDisallowInterceptTouchEvent(true);
            }
            boolean handled = super.dispatchTouchEvent(event);
            if (event.getActionMasked() == MotionEvent.ACTION_UP
                    || event.getActionMasked() == MotionEvent.ACTION_CANCEL) axis = 0;
            return handled;
        }

        @Override public boolean onInterceptTouchEvent(MotionEvent event) {
            return axis == 1 || super.onInterceptTouchEvent(event);
        }

        // 点击仍由符号键与分类键处理；这里只消费已经取消子键点击的横向手势。
        @SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(MotionEvent event) {
            if (axis != 1) return super.onTouchEvent(event);
            if (event.getActionMasked() == MotionEvent.ACTION_UP && !multiplePointers) {
                float dx = event.getX() - downX;
                int threshold = Math.max(touchSlop * 2, DimensionPolicy.pixels(getContext(), 40));
                if (Math.abs(dx) >= threshold) select(selected + (dx < 0 ? 1 : -1));
            }
            return true;
        }
    }

    /** 网格里只有一行提示文字（「常用」还没有记录、目录正在读或读不出来）。 */
    private void showHint(String text, int columns) {
        grid.removeAllViews();
        TextView hint = ViewPolicy.centeredText(getContext(), text, 14);
        KeyboardTypographyPolicy.setKeyTextSize(hint, 14);
        GridLayout.LayoutParams params = new GridLayout.LayoutParams(GridLayout.spec(0),
            GridLayout.spec(0, columns, 1f));
        params.width = 0;
        params.height = DimensionPolicy.pixels(getContext(), 92);
        grid.addView(hint, params);
        listener.restyle(hint);
    }

    /** 把 `symbols` 里从 `from` 起的条目接在网格后面；`from` 必须是整行的开头。 */
    private void appendRows(SymbolPanelModel.Category category, List<String> symbols, int from) {
        int columns = category.columns();
        int rowIndex = from / columns, columnIndex = 0;
        float columnWidth = grid.getMeasuredWidth() / (float) columns;
        for (int index = from; index < symbols.size(); index++) {
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
            // 长条目保持单行，跨列提供完整显示所需的宽度。
            if (SymbolPanelModel.singleLineCell(category, symbol)) ViewPolicy.setSingleLineEllipsized(button);
            KeyboardTypographyPolicy.normalizeKeyCap(button);
            ViewPolicy.clearPadding(button);
            // 单个符号按实际墨迹居中，逗号和引号不再分别贴着格子的下沿、上沿。
            if (TextPolicy.codePointLength(symbol) == 1) CenteredGlyphSpan.apply(button, symbol, 1f);
            int span = SymbolPanelModel.cellColumnSpan(category, symbol, button.getPaint().measureText(symbol),
                columnWidth, DimensionPolicy.pixels(getContext(), 12));
            if (columnIndex + span > columns) {
                addGridSpacer(rowIndex, columnIndex, columns - columnIndex);
                rowIndex++;
                columnIndex = 0;
            }
            GridLayout.LayoutParams params = gridCellParams(rowIndex, columnIndex, span);
            grid.addView(button, params);
            columnIndex += span;
            if (columnIndex == columns) {
                rowIndex++;
                columnIndex = 0;
            }
        }
        if (columnIndex > 0) addGridSpacer(rowIndex, columnIndex, columns - columnIndex);
    }

    private GridLayout.LayoutParams gridCellParams(int row, int column, int span) {
        GridLayout.LayoutParams params = new GridLayout.LayoutParams(GridLayout.spec(row),
            GridLayout.spec(column, span, GridLayout.FILL, (float) span));
        params.width = 0;
        params.height = DimensionPolicy.pixels(getContext(), 40);
        int margin = DimensionPolicy.pixels(getContext(), 3);
        params.setMargins(margin, margin, margin, margin);
        return params;
    }

    private void addGridSpacer(int row, int column, int span) {
        // 目录每格对应一个子视图，分页移除尾行时仍按既有的五格行索引处理。
        for (int offset = 0; offset < span; offset++)
            grid.addView(new View(getContext()), gridCellParams(row, column + offset, 1));
    }

    /** 当前是目录分类、还没读完、也没有正在读时，读下一页。 */
    private void loadNextCatalogPage() {
        List<SymbolPanelModel.Category> values = orderedCategories();
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
    }
}
