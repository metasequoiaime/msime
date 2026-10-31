package app.msime.android;

import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.widget.Button;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 网格辅筛的键盘适配；码位、歧义和排序均由引擎决定。 */
final class ImeGridHelpcode {
    private final MSIMEInputService s;
    private long press;
    private float downY;
    private boolean consumed;
    private long pressedSession, pressedGeneration;
    private boolean editingAtDown;
    ImeGridHelpcode(MSIMEInputService service) { s = service; }
    JSONObject state() { return s.view == null ? null : s.view.optJSONObject("grid_helpcode"); }
    boolean editing() { JSONObject v = state(); return v != null && v.optBoolean("editing"); }
    boolean active() { JSONObject v = state(); JSONArray slots = v == null ? null : v.optJSONArray("slots"); return slots != null && slots.length() > 0; }
    boolean available() {
        JSONObject v = state();
        return v != null && "available".equals(v.optString("availability"))
            && !s.imeCompositionEditor.isOpen();
    }
    String face() { return editing() ? (state().optBoolean("uppercase") ? "辅·大" : "辅·小") : "辅"; }
    String title(String pinyin) {
        return editing() || active() ? pinyin + "|" + state().optString("display") : pinyin;
    }
    void send(String kind, Object value) {
        if (s.view == null) return;
        send(kind, value, s.view.optLong("session"), s.view.optLong("generation"));
    }
    void send(String kind, Object value, long session, long generation) {
        try {
            JSONObject action = new JSONObject().put("kind", kind);
            if (value != null) action.put("value", value);
            String request = new JSONObject().put("session", session).put("generation", generation)
                .put("action", action).toString();
            s.apply(NativeClient.gridHelpcode(s.session, request));
        } catch (JSONException | LinkageError error) { s.fail(error); }
    }
    void tap() { send(editing() ? "toggle_case" : "begin", null); }
    void separator() { send("clear", null); s.character(39, false); }
    private boolean full() {
        JSONArray slots = state() == null ? null : state().optJSONArray("slots");
        if (slots == null || slots.length() < 2) return false;
        s.notice("最多两码");
        return true;
    }
    void group(int key) { if (!full()) send("append_group", key); }
    void exact(char letter) { if (!full()) send("append_exact", (int) Character.toLowerCase(letter)); }
    void label(Button key) {
        key.setText(face());
        if (key instanceof NineKeyDigitButton digit) digit.setDigitText("");
        if (key instanceof KeyHintButton hint) {
            hint.setCornerHint("↑符");
            hint.setHintText(editing() ? "分词·↓退" : "长按分词");
        }
        key.setContentDescription(face() + "；点按" + (editing() ? "切换下一码大小写" : "进入辅码")
            + "；长按分词；上滑符号；下滑退出辅码编辑");
    }
    void clearHints(Button key) {
        if (key instanceof KeyHintButton hint) { hint.setCornerHint(""); hint.setHintText(""); }
    }
    void bind(Button key) {
        key.setOnTouchListener((v, event) -> available() && touch(key, event));
        key.setOnLongClickListener(v -> { if (!available()) return false; separator(); return true; });
    }
    private boolean currentPress() {
        return s.view != null && pressedSession == s.view.optLong("session") && pressedGeneration == s.view.optLong("generation");
    }
    boolean touch(Button key, MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                downY = event.getY(); consumed = false;
                pressedSession = s.view.optLong("session");
                pressedGeneration = s.view.optLong("generation");
                editingAtDown = editing();
                s.countKey(key);
                s.imeKeyFeedback.playFeedback(key);
                long token = ++press;
                long session = s.view.optLong("session"), generation = s.view.optLong("generation");
                key.setPressed(true);
                key.postDelayed(() -> {
                    if (press != token || consumed || s.view == null
                            || generation != s.view.optLong("generation") || session != s.view.optLong("session")) return;
                    consumed = true; key.setPressed(false); separator();
                }, ViewConfiguration.getLongPressTimeout());
            }
            case MotionEvent.ACTION_MOVE -> {
                if (!consumed && currentPress() && Math.abs(event.getY() - downY) >= s.pixels(24)) {
                    consumed = true; ++press; key.setPressed(false);
                    if (event.getY() < downY) s.imePanels.showSymbolPanel();
                    else if (editing()) send("end", null);
                }
            }
            case MotionEvent.ACTION_UP -> {
                ++press; key.setPressed(false);
                if (!consumed && currentPress()) send(editingAtDown ? "toggle_case" : "begin", null, pressedSession, pressedGeneration);
            }
            case MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_POINTER_DOWN, MotionEvent.ACTION_POINTER_UP -> {
                ++press; consumed = true; key.setPressed(false);
            }
            default -> { }
        }
        return true;
    }
}
