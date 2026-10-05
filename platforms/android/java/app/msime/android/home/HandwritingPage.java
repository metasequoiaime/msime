package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 手写输入页：书写模式、识别等待时间、识别后显示拼音、笔迹颜色和笔迹粗细，全部存在共享偏好的 `touch_handwriting` 里，键盘的手写面板读同一份。
 *
 * <p>`touch_handwriting` 在 client-core 里是 `deny_unknown_fields` 的结构，这里只写它已有的五个成员；范围与步长（等待 200–1500 ms 步长 100，粗细 1–8 px）与 client-core 的校验一致，越界的值保存会被拒绝。
 */
public final class HandwritingPage extends DetailPage {
    private static final String KEY = "touch_handwriting";
    private static final String[] MODES = {"single", "overlap", "line"};
    private static final String[] MODE_LABELS = {"单字", "叠写", "行写"};
    private static final String[] COLORS = {"follow_skin", "black", "white", "blue"};
    private static final String[] COLOR_LABELS = {"跟随皮肤", "黑色", "白色", "蓝色"};
    private static final int DELAY_MIN = 200;
    private static final int DELAY_MAX = 1500;
    private static final int DELAY_STEP = 100;
    private static final int DELAY_DEFAULT = 600;
    private static final int WIDTH_MIN = 1;
    private static final int WIDTH_MAX = 8;
    private static final int WIDTH_DEFAULT = 3;

    @Nullable private LinearLayout column;
    /** 当前页面显示的 `touch_handwriting`，选择面板据此打 ✓。 */
    private JSONObject shown = new JSONObject();

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, HostStore::loadPreferences, this::render);
    }

    private void render(@Nullable JSONObject snapshot) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject handwriting = preferences.optJSONObject(KEY);
        if (handwriting == null) handwriting = new JSONObject();
        String mode = handwriting.optString("mode", "overlap");
        int delay = clamp(handwriting.optInt("recognition_delay_ms", DELAY_DEFAULT), DELAY_MIN, DELAY_MAX);
        boolean pinyin = handwriting.optBoolean("show_pinyin", true);
        String color = handwriting.optString("stroke_color", "follow_skin");
        int width = clamp(handwriting.optInt("stroke_width", WIDTH_DEFAULT), WIDTH_MIN, WIDTH_MAX);

        GroupCard writing = GroupCard.add(target, "书写");
        GroupCard.Row[] modeRow = new GroupCard.Row[1];
        modeRow[0] = writing.nav("书写模式", null, label(MODES, MODE_LABELS, mode, 1), () -> pick("书写模式",
            MODES, MODE_LABELS, "mode", current("mode", "overlap"), modeRow[0]));
        writing.slider("识别等待时间", DELAY_MIN, DELAY_MAX, DELAY_STEP, delay, value -> value + "ms",
            value -> save("recognition_delay_ms", value));
        writing.toggle("识别后显示拼音", null, pinyin, checked -> save("show_pinyin", checked));

        GroupCard stroke = GroupCard.add(target, "笔迹");
        GroupCard.Row[] colorRow = new GroupCard.Row[1];
        colorRow[0] = stroke.nav("笔迹颜色", null, label(COLORS, COLOR_LABELS, color, 0), () -> pick("笔迹颜色",
            COLORS, COLOR_LABELS, "stroke_color", current("stroke_color", "follow_skin"), colorRow[0]));
        stroke.slider("笔迹粗细", WIDTH_MIN, WIDTH_MAX, 1, width, value -> value + "px",
            value -> save("stroke_width", value));
        shown = handwriting;
    }

    private String current(String member, String fallback) {
        return shown.optString(member, fallback);
    }

    private void pick(String title, String[] values, String[] labels, String member, String selected,
            GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), title, null);
        for (int index = 0; index < values.length; index++) {
            String value = values[index];
            String text = labels[index];
            sheet.option(text, value.equals(selected), () -> {
                row.setValue(text);
                save(member, value);
            });
        }
        sheet.show();
    }

    private void save(String member, Object value) {
        try {
            shown.put(member, value);
        } catch (JSONException ignored) {
            return;
        }
        HostTask.run(this, context -> write(context, member, value), saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }

    @Nullable private static JSONObject write(Context context, String member, Object value) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        if (snapshot == null) return null;
        try {
            JSONObject preferences = snapshot.getJSONObject("preferences");
            JSONObject handwriting = preferences.optJSONObject(KEY);
            if (handwriting == null) {
                handwriting = new JSONObject();
                preferences.put(KEY, handwriting);
            }
            handwriting.put(member, value);
        } catch (JSONException error) {
            return null;
        }
        return HostStore.savePreferences(context, snapshot);
    }

    private static String label(String[] values, String[] labels, String value, int fallback) {
        for (int index = 0; index < values.length; index++) if (values[index].equals(value)) return labels[index];
        return labels[fallback];
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
