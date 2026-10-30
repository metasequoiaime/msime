package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 触屏横向完整候选条；保留全局身份，每次只取建得出来的那一批，接近尾部才继续取、继续建按钮。 */
final class ImeCandidateStrip {
    private final MSIMEInputService s;
    private JSONObject snapshot;
    private JSONObject sourceView;
    private boolean enabled;
    private LinearLayout list;
    private LinearLayout observedList;
    private HorizontalScrollView scroll;
    /** 这一代取来的候选，座位从 0 起连续；只是这一代的前一段，滚近尾部时再取下一批接上。 */
    private JSONArray entries;
    /** 这一代取到放出来的末尾之后又要过一次，引擎没有更多可放。 */
    private boolean exhausted;
    /** 追加时引擎放出了暂扣的尾部、换了一代：新的一代取回这么多行、建回原来的按钮数，滚动位置不回到开头。只对记下的这一代有效。 */
    private int carriedCount;
    private long carriedSession = Long.MIN_VALUE;
    private long carriedGeneration = Long.MIN_VALUE;
    private int built;
    private int visible;
    private boolean appendPosted;
    private long renderedSession = Long.MIN_VALUE;
    private long renderedGeneration = Long.MIN_VALUE;
    /**
     * 候选条按钮按位置复用。render 前候选行照旧 `removeAllViews`，这里只把同一批按钮按顺序挂回去、换上这一代的内容和点击目标，不再每次整排新建：每键几十个 Button 连同各自的监听、底图和文字测量，在低端机上是一两帧的主线程时间，旧按钮又全成了垃圾。换了键盘视图就整池丢掉。
     */
    private final java.util.ArrayList<KeyboardPressButton> pool = new java.util.ArrayList<>();

    ImeCandidateStrip(MSIMEInputService service) { s = service; }

    /**
     * 画首帧前先取这一代的第一批候选（{@link NativeClient#candidateRange}），身份和整代复制里同一座位的那一行相同。
     *
     * <p>以前这里每键取整代候选（{@code allCandidates}）：每键都在主线程上解析一整份候选 JSON（几百到几千行），首字母打下去还要逼引擎把暂扣的整张声母表查出来再换一代，而条上一次只建 48 个按钮。现在每键只取一批，暂扣的尾部等用户滚到放出来的末尾时再放（{@link #appendIfNeeded}）。
     */
    JSONObject prepare(boolean enabled) {
        this.enabled = enabled;
        if (!enabled || s.view == null || s.session == 0
                || s.view.optJSONArray("candidates") == null
                || s.view.optJSONArray("candidates").length() == 0) {
            snapshot = null;
            entries = null;
            renderedSession = Long.MIN_VALUE;
            renderedGeneration = Long.MIN_VALUE;
            return null;
        }
        if (sourceView == s.view && MSIMEInputService.sameCandidateVersion(snapshot, s.view)) return snapshot;
        try {
            // 同一代换了视图（释义晚到）取回已经建出来的数量，滚动位置不被截掉；追加时放出尾部换的代取回记下的数量；其余换代只取第一批。
            int wanted = CandidateGridBatchPolicy.BATCH;
            if (MSIMEInputService.sameCandidateVersion(snapshot, s.view))
                wanted = BoundsPolicy.atLeast(built, CandidateGridBatchPolicy.BATCH);
            else if (CandidateGlossPolicy.strictOr(s.view.opt("session"), Long.MIN_VALUE) == carriedSession
                    && CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1) == carriedGeneration)
                wanted = carriedCount;
            snapshot = s.candidateRangeSnapshot(0, wanted);
            sourceView = s.view;
            exhausted = false;
            return snapshot;
        } catch (JSONException | LinkageError error) {
            snapshot = null;
            entries = null;
            s.fail(error);
            return null;
        }
    }

    void render(JSONObject current, LinearLayout target, HorizontalScrollView viewport) {
        long nextSession = CandidateGlossPolicy.strictOr(current.opt("session"), Long.MIN_VALUE);
        long nextGeneration = CandidateGlossPolicy.strictOr(current.opt("generation"), -1);
        boolean same = nextSession == renderedSession && nextGeneration == renderedGeneration;
        // 追加时放出尾部换的代和同一代重画一样：建回原来的数量，滚动位置不回到开头。
        boolean carried = !same && nextSession == carriedSession && nextGeneration == carriedGeneration;
        carriedSession = Long.MIN_VALUE;
        carriedGeneration = Long.MIN_VALUE;
        renderedSession = nextSession;
        renderedGeneration = nextGeneration;
        int previous = built;
        list = target;
        scroll = viewport;
        entries = current.optJSONArray("candidates");
        built = 0;
        visible = 0;
        scroll.setOnScrollChangeListener((view, x, y, oldX, oldY) -> appendIfNeeded());
        // 每个键盘视图只装一次布局监听；同代重画仍使用最新的列表和快照。
        if (observedList != list) {
            observedList = list;
            pool.clear();
            list.addOnLayoutChangeListener((view, left, top, right, bottom,
                    oldLeft, oldTop, oldRight, oldBottom) -> postAppendCheck());
        }
        int total = entries == null ? 0 : entries.length();
        // 换了一代只先建第一批；之前滚远时多出来的按钮留两批备用，其余放掉，不长期占着内存。
        if (!same && !carried) {
            int retained = 2 * CandidateGridBatchPolicy.BATCH;
            if (pool.size() > retained) pool.subList(retained, pool.size()).clear();
        }
        add(CandidateGridBatchPolicy.initialCount(total, previous, same || carried), false);
        if (!same && !carried) scroll.scrollTo(0, 0);
    }

    private void postAppendCheck() {
        if (appendPosted || scroll == null) return;
        appendPosted = true;
        scroll.post(() -> { appendPosted = false; appendIfNeeded(); });
    }

    private void appendIfNeeded() {
        if (entries == null || list == null || scroll == null || !scroll.isShown()
                || !list.isAttachedToWindow() || !enabled || s.hardwareKeyboardMode
                || !MSIMEInputService.sameCandidateVersion(snapshot, s.view)) return;
        int loaded = entries.length();
        // 取来的都建完了而这一代还可能有更多（放出来的没取完，或还没问过引擎有没有暂扣的）时多算一格，滚近尾部就去取下一批。
        int known = built < loaded || exhausted ? loaded : loaded + 1;
        if (!CandidateGridBatchPolicy.shouldAppend(built, known, scroll.getScrollX(),
                scroll.getWidth(), list.getWidth())) return;
        if (built == loaded && !fetchMore()) return;
        add(CandidateGridBatchPolicy.nextCount(built, entries.length()), true);
    }

    /** 取下一批接到 {@link #entries} 后面；取到了返回真。取到放出来的末尾之后再要会让引擎放出暂扣的尾部，那会换一代、座位可能重排，这时按新的一代重画。 */
    private boolean fetchMore() {
        int loaded = entries.length();
        JSONObject next;
        try {
            next = s.candidateRangeSnapshot(loaded, CandidateGridBatchPolicy.BATCH);
        } catch (JSONException | LinkageError error) {
            s.fail(error);
            return false;
        }
        if (!MSIMEInputService.sameCandidateVersion(next, snapshot)) {
            // candidateRangeSnapshot 已经把视图换成新的一代，已建按钮的身份全部过期（点了也不会选）。等这次滚动回调走完再按新的一代重画，取回并建回这么多。
            carriedSession = CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE);
            carriedGeneration = CandidateGlossPolicy.strictOr(next.opt("generation"), -1);
            carriedCount = loaded + CandidateGridBatchPolicy.BATCH;
            scroll.post(s::render);
            return false;
        }
        JSONArray rows = next.optJSONArray("candidates");
        if (rows == null || rows.length() == 0) {
            exhausted = true;
            return false;
        }
        for (int index = 0; index < rows.length(); index++) entries.put(rows.opt(index));
        return true;
    }

    private void add(int target, boolean style) {
        while (built < target) {
            JSONObject candidate = entries.optJSONObject(built++);
            if (candidate == null || !AiPendingCandidatePolicy.isVisible(
                    InputViewValuePolicy.integer(candidate, "source", -1))) continue;
            if (visible == pool.size()) pool.add(s.imeCandidates.newStripCandidateButton());
            KeyboardPressButton button = pool.get(visible);
            s.imeCandidates.bindStripCandidateButton(button, snapshot, candidate, visible++);
            // render 前候选行已经 removeAllViews，池里的按钮这时都没有父视图；万一还挂在别处，先摘下来再挂。
            if (button.getParent() instanceof android.view.ViewGroup parent) parent.removeView(button);
            list.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            if (style) s.imeStyler.applySkinToView(button, true, s.skin);
        }
    }
}
