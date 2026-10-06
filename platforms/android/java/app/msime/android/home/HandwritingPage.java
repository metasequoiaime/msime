package app.msime.android.home;

import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;

/**
 * 手写输入页：书写模式、识别等待时间、识别后显示拼音、笔迹颜色和笔迹粗细。它们只有 Android 用，存在 {@link AndroidLocalSettings} 的 `platform.android.handwriting_*` 里（不在共享偏好里），键盘的手写面板读同一个文件。
 *
 * <p>范围与步长（等待 200–1500 ms 步长 100，粗细 1–8 px）由本地设置的校验给出，与同步时 client-core 接受的范围相同。
 */
public final class HandwritingPage extends DetailPage {
    private static final String[] MODES = {"single", "overlap", "line"};
    private static final String[] MODE_LABELS = {"单字", "叠写", "行写"};
    private static final String[] COLORS = {"follow_skin", "black", "white", "blue"};
    private static final String[] COLOR_LABELS = {"跟随皮肤", "黑色", "白色", "蓝色"};

    @Nullable private LinearLayout column;
    /** 当前页面显示的设置，选择面板据此打 ✓。 */
    private AndroidLocalSettings.Snapshot shown = AndroidLocalSettings.defaults();

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
        HostTask.run(this, AndroidLocalSettings::load, this::render);
    }

    private void render(AndroidLocalSettings.Snapshot settings) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        shown = settings;
        String mode = settings.choice(AndroidLocalSettings.HANDWRITING_MODE);
        int delay = settings.integer(AndroidLocalSettings.HANDWRITING_DELAY_MS);
        String color = settings.choice(AndroidLocalSettings.HANDWRITING_STROKE_COLOR);
        int width = settings.integer(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH);
        AndroidLocalSettings.Spec delaySpec = AndroidLocalSettings.spec(AndroidLocalSettings.HANDWRITING_DELAY_MS);
        AndroidLocalSettings.Spec widthSpec = AndroidLocalSettings.spec(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH);

        GroupCard writing = GroupCard.add(target, "书写");
        GroupCard.Row[] modeRow = new GroupCard.Row[1];
        modeRow[0] = writing.nav("书写模式", null, label(MODES, MODE_LABELS, mode, 1), () -> pick("书写模式",
            MODES, MODE_LABELS, AndroidLocalSettings.HANDWRITING_MODE, modeRow[0]));
        writing.slider("识别等待时间", delaySpec.min, delaySpec.max, delaySpec.step, delay, value -> value + "ms",
            value -> save(AndroidLocalSettings.HANDWRITING_DELAY_MS, value));
        writing.toggle("识别后显示拼音", null, settings.bool(AndroidLocalSettings.HANDWRITING_SHOW_PINYIN),
            checked -> save(AndroidLocalSettings.HANDWRITING_SHOW_PINYIN, checked));

        GroupCard stroke = GroupCard.add(target, "笔迹");
        GroupCard.Row[] colorRow = new GroupCard.Row[1];
        colorRow[0] = stroke.nav("笔迹颜色", null, label(COLORS, COLOR_LABELS, color, 0), () -> pick("笔迹颜色",
            COLORS, COLOR_LABELS, AndroidLocalSettings.HANDWRITING_STROKE_COLOR, colorRow[0]));
        stroke.slider("笔迹粗细", widthSpec.min, widthSpec.max, widthSpec.step, width, value -> value + "px",
            value -> save(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH, value));
    }

    private void pick(String title, String[] values, String[] labels, String key, GroupCard.Row row) {
        String selected = shown.choice(key);
        OptionSheet sheet = new OptionSheet(requireContext(), title, null);
        for (int index = 0; index < values.length; index++) {
            String value = values[index];
            String text = labels[index];
            sheet.option(text, value.equals(selected), () -> {
                row.setValue(text);
                save(key, value);
            });
        }
        sheet.show();
    }

    private void save(String key, Object value) {
        HostTask.run(this, context -> KeyboardSheets.writeLocal(context, key, value)
            ? AndroidLocalSettings.load(context) : null, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            } else {
                shown = saved;
            }
        });
    }

    private static String label(String[] values, String[] labels, String value, int fallback) {
        for (int index = 0; index < values.length; index++) if (values[index].equals(value)) return labels[index];
        return labels[fallback];
    }
}
