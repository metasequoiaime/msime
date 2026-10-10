package app.msime.android;

import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.ScrollView;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 展开面板里的完整候选网格：先建第一批格子，滚到接近底部时再追加，规则在 {@link CandidateGridBatchPolicy}。整面网格（{@link ImeCandidates#renderExpandedCandidates}）和全拼九键三栏面板的中栏各持有一个，各自对应一个纵向滚动视图。
 */
final class ImeCandidateGrid {
    private final MSIMEInputService s;
    private final ScrollView scroll;
    /** 网格所在子树在样式通道里是否按候选上色：整面网格在 `expandedCandidates` 下面，是；九键中栏不是，格子靠「候选 」开头的描述被认出来。追加的格子按同样的方式上色。 */
    private final boolean candidateContext;
    private CandidateWrapLayout list;
    private JSONArray entries;
    private int built;
    private long session = Long.MIN_VALUE;
    private long generation = Long.MIN_VALUE;
    private boolean appendPosted;

    ImeCandidateGrid(MSIMEInputService s, ScrollView scroll, boolean candidateContext) {
        this.s = s;
        this.scroll = scroll;
        this.candidateContext = candidateContext;
        scroll.setOnScrollChangeListener((view, x, y, oldX, oldY) -> appendIfNeeded());
    }

    boolean drives(ScrollView candidate) {
        return scroll == candidate;
    }

    /** 下一次建网格从第一批开始、滚动位置回到顶部，即使还是同一代候选。 */
    void reset() {
        session = Long.MIN_VALUE;
        generation = Long.MIN_VALUE;
    }

    /**
     * 为这份完整候选新建网格并先建第一批格子；同一代的重画建回上次已建的数量，换了一代把滚动位置拉回顶部。返回的网格由调用方挂进滚动内容，格子的样式仍由随后的整树样式通道一次画好。
     */
    CandidateWrapLayout build(JSONObject snapshot, JSONArray candidates, String description) {
        long nextSession = CandidateGlossPolicy.strictOr(snapshot.opt("session"), Long.MIN_VALUE);
        long nextGeneration = CandidateGlossPolicy.strictOr(snapshot.opt("generation"), -1);
        boolean sameGeneration = nextSession == session && nextGeneration == generation;
        session = nextSession;
        generation = nextGeneration;
        entries = candidates;
        int total = candidates == null ? 0 : candidates.length();
        int previouslyBuilt = built;
        built = 0;
        list = new CandidateWrapLayout(s, s.pixels(6));
        list.setContentDescription(description);
        // 布局完成后看一眼：第一批没填够一屏多（候选很短、面板很高）时接着追加，不等用户滚动。
        list.addOnLayoutChangeListener((view, left, top, right, bottom,
                oldLeft, oldTop, oldRight, oldBottom) -> postAppendCheck());
        addCells(CandidateGridBatchPolicy.initialCount(total, previouslyBuilt, sameGeneration), false);
        if (!sameGeneration) scroll.scrollTo(0, 0);
        return list;
    }

    private void postAppendCheck() {
        if (appendPosted) return;
        appendPosted = true;
        scroll.post(() -> {
            appendPosted = false;
            appendIfNeeded();
        });
    }

    private void appendIfNeeded() {
        if (list == null || entries == null || !list.isAttachedToWindow() || !scroll.isShown())
            return;
        int total = entries.length();
        if (!CandidateGridBatchPolicy.shouldAppend(built, total, scroll.getScrollY(),
                scroll.getHeight(), gridBottom())) return;
        addCells(CandidateGridBatchPolicy.nextCount(built, total), true);
    }

    private void addCells(int target, boolean style) {
        while (built < target) {
            JSONObject candidate = entries.optJSONObject(built++);
            if (candidate == null) continue;
            Button button = s.imeCandidates.expandedCandidateButton(candidate);
            list.addView(button, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, s.pixels(44)));
            // 打开和重画时随后的整树样式通道会画这些格子；滚动中追加的格子没有那一遍，在这里按同一条规则画。
            if (style) s.imeStyler.applySkinToView(button, candidateContext, s.skin);
        }
    }

    /** 网格底边在滚动内容里的纵坐标。 */
    private int gridBottom() {
        int top = 0;
        View node = list;
        while (node != scroll && node.getParent() instanceof View parent) {
            top += node.getTop();
            node = parent;
        }
        return top + list.getHeight();
    }
}
