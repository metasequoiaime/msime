package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 触屏横向完整候选条；保留全局身份，接近尾部才继续建按钮。 */
final class ImeCandidateStrip {
    private final MSIMEInputService s;
    private JSONObject snapshot;
    private JSONObject sourceView;
    private boolean enabled;
    private LinearLayout list;
    private LinearLayout observedList;
    private HorizontalScrollView scroll;
    private JSONArray entries;
    private int built;
    private int visible;
    private boolean appendPosted;
    private long renderedSession = Long.MIN_VALUE;
    private long renderedGeneration = Long.MIN_VALUE;

    ImeCandidateStrip(MSIMEInputService service) { s = service; }

    /** 先取完整候选再画首帧，防止补齐尾部时换代使按钮身份过期。 */
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
            snapshot = s.currentCandidateSnapshot();
            sourceView = s.view;
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
            list.addOnLayoutChangeListener((view, left, top, right, bottom,
                    oldLeft, oldTop, oldRight, oldBottom) -> postAppendCheck());
        }
        int total = entries == null ? 0 : entries.length();
        add(CandidateGridBatchPolicy.initialCount(total, previous, same), false);
        if (!same) scroll.scrollTo(0, 0);
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
        if (!CandidateGridBatchPolicy.shouldAppend(built, entries.length(), scroll.getScrollX(),
                scroll.getWidth(), list.getWidth())) return;
        add(CandidateGridBatchPolicy.nextCount(built, entries.length()), true);
    }

    private void add(int target, boolean style) {
        while (built < target) {
            JSONObject candidate = entries.optJSONObject(built++);
            if (candidate == null || !AiPendingCandidatePolicy.isVisible(
                    InputViewValuePolicy.integer(candidate, "source", -1))) continue;
            Button button = s.imeCandidates.stripCandidateButton(snapshot, candidate, visible++);
            list.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            if (style) s.imeStyler.applySkinToView(button, true, s.skin);
        }
    }
}
