package app.msime.android.test;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.text.Spanned;
import android.text.style.ForegroundColorSpan;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import java.io.File;
import java.io.FileOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.function.Consumer;
import org.json.JSONObject;

/**
 * 云候选与 AI 候选的来源标记（`CandidateSourceMark`）在安装产品里的样子：用真实会话的第一个候选，把来源改成 2、3 等交给候选条和展开面板的真实建按钮路径，核对标签里的标记、读屏说明、两个偏好、占位行，以及带标记的候选与不带标记的一样高；最后把一排候选条按钮按当前皮肤画进 PNG，留作截图。
 *
 * <p>同一处也核对双拼触点纠错的标记（#10018）仍经两条建按钮路径画出来：坐上首选的纠错行候选字后面有 *、候选字盖标记色、读屏补「，已纠正」，普通候选都没有。这一条原先是 `check-host.sh` 里钉住两段源码文字的守卫，改成在真实按钮上看结果。
 *
 * <p>由 `StaleCandidateDeviceSmoke -e source-marks true` 进入：API 35 的 instrumentation 已到 100 项上限，不另加入口。
 */
final class CandidateSourceMarkDeviceChecks {
    private static final String SPAN = "app.msime.android.CandidateSourceMarkSpan";

    private CandidateSourceMarkDeviceChecks() {}

    static void run(Context target, ClassLoader loader, Class<?> type, Object service, JSONObject view,
            Consumer<String> stage) throws Exception {
        field(type, "view").set(service, view);
        JSONObject first = view.getJSONArray("candidates").getJSONObject(0);
        Object candidates = field(type, "imeCandidates").get(service);
        Method newStrip = candidates.getClass().getDeclaredMethod("newStripCandidateButton");
        newStrip.setAccessible(true);
        Method update = type.getDeclaredMethod("updateCandidateButton", Button.class, JSONObject.class, int.class, int.class);
        update.setAccessible(true);
        Method expanded = candidates.getClass().getDeclaredMethod("expandedCandidateButton", JSONObject.class);
        expanded.setAccessible(true);

        stage.accept("correction marks on strip and panel");
        Object palette = field(type, "candidateAppearance").get(service);
        Method correctionFor = palette.getClass().getDeclaredMethod("correctionFor", boolean.class);
        correctionFor.setAccessible(true);
        JSONObject confident = new JSONObject(first.toString())
            .put("confident_correction", true).put("corrected", true);
        int markColour = (Integer) correctionFor.invoke(palette, first.optBoolean("highlighted", false));
        Button stripCorrected = strip(newStrip, update, candidates, service, confident, 0);
        Button panelCorrected = (Button) expanded.invoke(candidates, withSource(confident, 0));
        for (Button button : new Button[] {stripCorrected, panelCorrected}) {
            String word = first.getString("text");
            check(button.getText().toString().startsWith(word + "*"), "纠错行的候选字后面没有 *：" + button.getText());
            check(wordColour(button, word.length()) == markColour, "纠错行的候选字没有盖标记色");
            check(spoken(button).contains("，已纠正"), "纠错行的读屏没有说已纠正：" + spoken(button));
        }
        Button stripPlain = strip(newStrip, update, candidates, service, first, 0);
        Button panelPlain = (Button) expanded.invoke(candidates, withSource(first, 0));
        for (Button button : new Button[] {stripPlain, panelPlain}) {
            check(!button.getText().toString().contains("*"), "普通候选多了 *：" + button.getText());
            check(wordColour(button, first.getString("text").length()) == null, "普通候选的候选字被盖了颜色");
            check(!spoken(button).contains("已纠正"), "普通候选被说成已纠正");
        }

        stage.accept("strip marks and spoken source");
        Button plain = strip(newStrip, update, candidates, service, first, 0);
        Button cloud = strip(newStrip, update, candidates, service, first, 2);
        Button ai = strip(newStrip, update, candidates, service, first, 3);
        Button placeholder = strip(newStrip, update, candidates, service, first, 12);
        check(marks(plain) == 0 && marks(cloud) == 1 && marks(ai) == 1 && marks(placeholder) == 0,
            "来源标记数不对：" + marks(plain) + "/" + marks(cloud) + "/" + marks(ai) + "/" + marks(placeholder));
        check(spoken(cloud).contains("，云候选") && spoken(ai).contains("，AI 候选"), "读屏没有说来源：" + spoken(cloud));
        check(!spoken(plain).contains("云候选") && !spoken(plain).contains("AI 候选"), "普通候选被说成在线来源");
        check(spoken(placeholder).endsWith("AI 候选等待中") || spoken(placeholder).endsWith("AI 候选没有结果"),
            "占位行没有念状态：" + spoken(placeholder));

        stage.accept("mark keeps the line height");
        int plainHeight = measuredHeight(plain), cloudHeight = measuredHeight(cloud);
        check(plainHeight == cloudHeight, "带标记的候选变高：" + plainHeight + " → " + cloudHeight);
        // 单字候选的按钮被最小宽度撑着，比的是文字排出来的宽度。
        check(cloud.getLayout().getLineWidth(0) > plain.getLayout().getLineWidth(0), "标记没有占宽度");

        stage.accept("expanded panel marks");
        Button panelCloud = (Button) expanded.invoke(candidates, withSource(first, 2));
        check(marks(panelCloud) == 1 && spoken(panelCloud).contains("，云候选"), "展开面板的云候选没有标记");

        stage.accept("preferences");
        Object robot = drawable(ai);
        field(type, "candidateAiBadgeStyle").set(service, "whale");
        Button whale = strip(newStrip, update, candidates, service, first, 3);
        check(marks(whale) == 1 && drawable(whale) != robot, "小鲸鱼样式没有换图形");
        field(type, "candidateCloudBadge").setBoolean(service, false);
        Button hidden = strip(newStrip, update, candidates, service, first, 2);
        check(marks(hidden) == 0 && spoken(hidden).contains("，云候选"), "关掉云标记后应只隐藏图形");
        Button aiStill = strip(newStrip, update, candidates, service, first, 3);
        check(marks(aiStill) == 1, "关掉云标记不该影响 AI 标记");

        stage.accept("render strip sample");
        field(type, "candidateCloudBadge").setBoolean(service, true);
        field(type, "candidateAiBadgeStyle").set(service, "robot");
        // 样张里只有第一格是选中的首选，其余三格按未选中画，标记用未选中的释义色。
        JSONObject unselected = new JSONObject(first.toString()).put("highlighted", false);
        field(type, "candidateAiBadgeStyle").set(service, "whale");
        Button whaleSample = strip(newStrip, update, candidates, service, unselected, 3);
        field(type, "candidateAiBadgeStyle").set(service, "robot");
        render(target, type, service, new Button[] {
            strip(newStrip, update, candidates, service, first, 0),
            strip(newStrip, update, candidates, service, unselected, 2),
            strip(newStrip, update, candidates, service, unselected, 3),
            whaleSample});
    }

    private static Button strip(Method newStrip, Method update, Object candidates, Object service,
            JSONObject first, int source) throws Exception {
        Button button = (Button) newStrip.invoke(candidates);
        update.invoke(service, button, withSource(first, source), 0, 0);
        return button;
    }

    private static JSONObject withSource(JSONObject candidate, int source) throws Exception {
        return new JSONObject(candidate.toString()).put("source", source);
    }

    private static int marks(Button button) {
        if (!(button.getText() instanceof Spanned text)) return 0;
        int count = 0;
        for (Object span : text.getSpans(0, text.length(), Object.class))
            if (span.getClass().getName().equals(SPAN)) count++;
        return count;
    }

    /** 盖在候选字（标签开头 {@code length} 个字符）上的前景色；没有时为 null。 */
    private static Integer wordColour(Button button, int length) {
        if (!(button.getText() instanceof Spanned text)) return null;
        for (ForegroundColorSpan span : text.getSpans(0, length, ForegroundColorSpan.class))
            if (text.getSpanStart(span) == 0 && text.getSpanEnd(span) >= length) return span.getForegroundColor();
        return null;
    }

    private static Object drawable(Button button) throws Exception {
        Spanned text = (Spanned) button.getText();
        for (Object span : text.getSpans(0, text.length(), Object.class)) {
            if (!span.getClass().getName().equals(SPAN)) continue;
            Field drawable = span.getClass().getDeclaredField("drawable");
            drawable.setAccessible(true);
            return drawable.get(span);
        }
        throw new AssertionError("没有来源标记");
    }

    private static String spoken(Button button) {
        return String.valueOf(button.getContentDescription());
    }

    private static int measuredHeight(Button button) {
        int unspecified = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        button.measure(unspecified, unspecified);
        return button.getMeasuredHeight();
    }

    /** 按当前皮肤给按钮上色，排成一行画进位图，存到产品的外部文件目录（`source-marks/strip.png`）。 */
    private static void render(Context target, Class<?> type, Object service, Button[] buttons) throws Exception {
        Object styler = field(type, "imeStyler").get(service);
        Method apply = styler.getClass().getDeclaredMethod("applySkinToView", View.class);
        apply.setAccessible(true);
        LinearLayout row = new LinearLayout(target);
        row.setOrientation(LinearLayout.HORIZONTAL);
        for (Button button : buttons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            row.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
            apply.invoke(styler, button);
            // 候选条按钮横向滚动排字，靠挂上窗口后的 onPreDraw 把居中的文字滚进来；离屏画样张时没有这一步，关掉横向滚动让文字按居中直接排在按钮里。
            button.setHorizontallyScrolling(false);
        }
        int unspecified = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        row.measure(unspecified, unspecified);
        row.layout(0, 0, row.getMeasuredWidth(), row.getMeasuredHeight());
        Bitmap bitmap = Bitmap.createBitmap(row.getMeasuredWidth(), row.getMeasuredHeight(), Bitmap.Config.ARGB_8888);
        bitmap.eraseColor(Color.TRANSPARENT);
        row.draw(new Canvas(bitmap));
        File directory = new File(target.getExternalFilesDir(null), "source-marks");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new AssertionError("截图目录创建失败");
        try (FileOutputStream output = new FileOutputStream(new File(directory, "strip.png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new AssertionError("截图编码失败");
        } finally { bitmap.recycle(); }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }

    private static void check(boolean condition, String description) {
        if (!condition) throw new AssertionError(description);
    }
}
