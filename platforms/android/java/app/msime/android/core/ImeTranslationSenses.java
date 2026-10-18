package app.msime.android;

import android.os.Build;
import app.msime.android.core.InputViewValuePolicy;
import android.widget.Button;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 实体键盘 Ctrl+Enter 打开的义项页（#7079）：高亮候选的译文有几个义项时，候选行换成这些义项，用 1–9 选第几项，空格或回车上屏高亮的那项，方向键移动高亮，Esc 只关页。
 *
 * <p>页面属于打开它时的那一次 Engine 回复：会话或 generation 一变（继续打字、点选候选、换输入框，`onUpdateSelection` 把 view 置空），{@link #active()} 就自动变成 false，不用每条路径各自记得关页。异步到达的释义经 `applyCandidateGlosses` 换掉 view 时 generation 不变，页面保留。云候选和 AI 候选异步到达时 runtime 会推进 generation，那条路径先调 {@link #followOnlineRefresh}：组字和高亮候选都没变，页面就跟到新的 generation 上，免得用户正在选义项时被一个迟到的在线回复关页，下一个数字落到候选条上选了中文候选。
 */
final class ImeTranslationSenses {
    private final MSIMEInputService s;
    private final List<Button> buttons = new ArrayList<>();
    private List<String> senses = List.of();
    private int highlight;
    private long session = -1;
    private long generation = -1;
    /** 开页时的组字和高亮候选原文，用来判断在线刷新之后页面是否还属于同一次组字。 */
    private String editingText = "";
    private String candidateText = "";

    ImeTranslationSenses(MSIMEInputService s) {
        this.s = s;
    }

    /** 页面这时是否该显示：有义项，而且还是打开它时的那次组字。 */
    boolean active() {
        return !senses.isEmpty() && s.view != null && s.session != 0 && s.session == session
            && CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1) == generation
            && s.hasEngineComposition();
    }

    int size() {
        return senses.size();
    }

    /** 按当前 view 打开义项页，高亮第一项；`candidate` 是义项所属的高亮候选原文。调用方随后重画。 */
    void open(List<String> next, String candidate) {
        senses = List.copyOf(next);
        highlight = 0;
        session = s.session;
        generation = s.view == null ? -1 : CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
        editingText = s.view == null ? "" : InputViewValuePolicy.editingText(s.view);
        candidateText = candidate;
    }

    /** 在线结果把 view 换成 `next` 之前调用：同一会话里组字和高亮候选原文都没变时，页面改认 `next` 的 generation；否则不动，页面随旧 generation 失效。 */
    void followOnlineRefresh(JSONObject next) {
        if (!active() || next == null
                || CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != session) return;
        int slot = MSIMEInputService.highlightedCandidateSlot(next);
        JSONArray entries = next.optJSONArray("candidates");
        JSONObject candidate = slot < 0 || entries == null ? null : entries.optJSONObject(slot);
        String nextCandidate = candidate == null ? "" : InputViewValuePolicy.textOr(candidate, "text", "");
        if (!HardwareTranslationCommitPolicy.pageSurvivesRefresh(editingText, candidateText,
                InputViewValuePolicy.editingText(next), nextCandidate)) return;
        generation = CandidateGlossPolicy.strictOr(next.opt("generation"), -1);
    }

    /** 关页，不重画。 */
    void clear() {
        senses = List.of();
        highlight = 0;
        session = -1;
        generation = -1;
        editingText = "";
        candidateText = "";
    }

    /** 高亮移到 `index`（越界时停在两头）并重画。 */
    void moveTo(int index) {
        if (senses.isEmpty()) return;
        highlight = Math.max(0, Math.min(senses.size() - 1, index));
        s.render();
    }

    void move(int delta) {
        moveTo(highlight + delta);
    }

    void commitHighlighted() {
        commit(highlight);
    }

    /** 上屏第 `index` 项：先关页，再由服务上屏并结束组字。页面已经失效时什么也不做。 */
    void commit(int index) {
        if (!active() || index < 0 || index >= senses.size()) return;
        String sense = senses.get(index);
        clear();
        s.commitTranslationSense(sense);
    }

    /** 把义项按钮接到候选行里：前面标 1–9，高亮那项画成选中，样式由 render 末尾的皮肤通道按候选上色。 */
    void render(LinearLayout activeCandidates, boolean horizontal) {
        for (int slot = 0; slot < senses.size(); slot++) {
            while (buttons.size() <= slot) buttons.add(makeButton(buttons.size()));
            Button button = buttons.get(slot);
            String sense = senses.get(slot);
            boolean highlighted = slot == highlight;
            ViewPolicy.show(button);
            button.setText(s.candidateLabel((slot + 1) + " ", sense, "", highlighted));
            ViewPolicy.setFixedLines(button, 1);
            s.configureCandidateTextLayout(button, 1);
            KeyboardTypographyPolicy.setKeyTextSize(button, s.candidateFontSize);
            ViewPolicy.setSelected(button, highlighted);
            button.setContentDescription("译文 " + (slot + 1) + "：" + sense);
            if (Build.VERSION.SDK_INT >= 30) button.setStateDescription(highlighted ? "已选中" : "未选中");
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                horizontal ? LinearLayout.LayoutParams.WRAP_CONTENT : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = senses.size(); slot < buttons.size(); slot++) ViewPolicy.hide(buttons.get(slot));
    }

    private Button makeButton(int slot) {
        Button button = ViewPolicy.newPressButton(s);
        s.bindCandidateRenderRelease(button);
        ViewPolicy.setAllCapsFalse(button);
        button.setLongClickable(false);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            commit(slot);
        });
        return button;
    }
}
