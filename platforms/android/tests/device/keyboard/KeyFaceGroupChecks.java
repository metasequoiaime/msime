package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.view.ContextThemeWrapper;
import android.view.View;
import android.widget.Button;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/**
 * 用产品的键组画字母键和键区功能键：同一组的键用同一份参数（字号、基线、退让方式），主标签、角标和底部提示的墨迹不相碰、不出键格，换提示只重画不重新布局；功能键的字不大过字母、和字母落在同一条中线上，图标画在键格中间。只比墨迹框和算出的参数，不比像素颜色以外的细节。
 *
 * <p>键帽轮廓：用产品的设计皮肤（胶囊、卵石、票根、圆角，平面和凸起）给字母键、九键和回车配上产品自己的键帽，键面的墨迹一点都不落在键帽面之外（凸起的厚度和边框也算在外面）。
 */
final class KeyFaceGroupChecks {
    private KeyFaceGroupChecks() { }

    static void verify(Instrumentation test) throws Exception {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        test.runOnMainSync(() -> {
            try {
                for (float fontScale : new float[] {1f, 2f}) {
                    Configuration config = new Configuration(test.getTargetContext().getResources().getConfiguration());
                    config.fontScale = fontScale;
                    Context context = new ContextThemeWrapper(test.getTargetContext().createConfigurationContext(config),
                        android.R.style.Theme_Material_Light_NoActionBar);
                    for (int height : new int[] {29, 40, 46, 52, 84}) {
                        verifyRow(context, height);
                        verifyFunctions(context, height);
                        verifyNineKeys(context, height);
                        verifyNumberRow(context, height);
                        verifySideSymbols(context, height);
                        verifyLayer(context, height);
                        verifyScripts(context, height);
                        verifyStroke(context, height);
                        verifyZhuyinNineKey(context, height);
                        verifyJapaneseNineKey(context, height);
                        verifyOutlines(context, height);
                    }
                }
            } catch (Exception | AssertionError error) { failure.set(error); }
        });
        if (failure.get() != null) throw new AssertionError("键组字母键面：" + failure.get());
    }

    /** 一排宽窄不同的字母键放进同一个组。 */
    private static void verifyRow(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        Object group = product.letterGroup(density, scale);
        String[][] faces = {{"W", "ei", "2"}, {"Q", "iu", "1"}, {"M", "ian", "?"}, {"I", "", ""}, {"L", "uang", "（"}};
        int[] widthsDp = {28, 34, 40, 62, 72};
        int height = Math.round(heightDp * density);
        List<Button> keys = new ArrayList<>();
        for (int index = 0; index < faces.length; index++) {
            Button key = product.letterKey(faces[index][0], faces[index][1], faces[index][2]);
            product.join(key, group);
            int width = Math.round(widthsDp[index] * density);
            key.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
            key.layout(0, 0, width, height);
            keys.add(key);
        }
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Object params = product.params(keys.get(0));
        check(params != null, "键组没有算出参数：" + where);
        for (Button key : keys) check(product.params(key) == params, "同一组的键参数不同：" + where);
        float size = product.mainSize(params);
        if (heightDp >= 40) check(size >= 12 * density * scale - 1, "主字母小于最小字号：" + size + " " + where);
        for (int index = 0; index < keys.size(); index++) {
            Button key = keys.get(index);
            String at = faces[index][0] + " " + widthsDp[index] + "×" + where;
            Bitmap bitmap = Bitmap.createBitmap(key.getWidth(), key.getHeight(), Bitmap.Config.ARGB_8888);
            try {
                Canvas canvas = new Canvas(bitmap);
                canvas.drawColor(Color.WHITE);
                key.draw(canvas);
                Rect main = ink(bitmap, 1), corner = ink(bitmap, 0), bottom = ink(bitmap, 2);
                check(!main.isEmpty(), "主字母没有画出来：" + at);
                check(main.left >= 0 && main.right <= key.getWidth() && main.top >= 0 && main.bottom <= key.getHeight(),
                    "主字母出了键格：" + at);
                if (!corner.isEmpty()) check(!Rect.intersects(main, corner), "主字母压住角标：" + at);
                if (!bottom.isEmpty()) check(main.bottom <= bottom.top, "主字母压住底部提示：" + at);
                check(faces[index][1].isEmpty() || product.hidesBottom(params) || !bottom.isEmpty(), "底部提示没画：" + at);
            } finally { bitmap.recycle(); }
        }
        // 换提示：只重画，参数和布局都不变。
        Button first = keys.get(0);
        product.setCorner(first, "");
        product.setBottom(first, "");
        check(!first.isLayoutRequested(), "去掉提示后请求了重新布局：" + where);
        check(product.params(first) == params, "去掉提示后参数变了：" + where);
        product.setCorner(first, "2");
        check(!first.isLayoutRequested() && product.params(first) == params, "恢复提示后请求了重新布局或参数变了：" + where);
    }

    /**
     * 键区功能键，按真实键盘里的搭配各放一组：26 键的 ⇧（组字时画「分词」）和 ⌫、14/18 键带「辅」的分词键和 ⌫（这两组以一个窄字母键所在的字母组为上限），九键右列的 ⌫ 和重输（还没有字母组，不设上限）。
     */
    private static void verifyFunctions(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Button letter = product.letterKey("W", "", "");
        product.join(letter, product.letterGroup(density, scale));
        layout(letter, Math.round(30 * density), height);

        Button shift = product.iconKey("SHIFT", "⇧");
        product.setTextFace(shift, "分词");
        verifyFunctionRow(product, density, scale, height, "26 键 " + where, 0, letter, null,
            new Button[] {shift, product.iconKey("BACKSPACE", "⌫")}, new int[] {48, 48});
        // 组字结束：⇧ 换回图标，只重画。
        product.setIconFace(shift, "SHIFT");
        shift.setText("⇧");
        check(!shift.isLayoutRequested(), "⇧ 换回图标后请求了重新布局：" + where);

        Button separator = product.letterKey("分词", "", "辅");
        verifyFunctionRow(product, density, scale, height, "14/18 键 " + where, 8, letter, separator,
            new Button[] {separator, product.iconKey("BACKSPACE", "⌫")}, new int[] {52, 52});
        product.setCorner(separator, "");
        product.setCorner(separator, "辅");
        check(!separator.isLayoutRequested(), "分词键换角标后请求了重新布局：" + where);

        Button rewrite = product.plainKey("重输");
        // TextView 可能按过期的排版把内容滚开（键组里的键不走它的测量）；键面要照样画在键格中间。
        verifyFunctionRow(product, density, scale, height, "九键 " + where, 1, null, null,
            new Button[] {product.iconKey("BACKSPACE", "⌫"), rewrite}, new int[] {56, 56});
    }

    /**
     * 一组功能键：参数相同、字不大过字母组、图标和文字竖直居中、文字按全组的水平偏移居中、不压住角标；TextView 补建了自己的排版之后换文字（连行数都变）也不请求重新布局。
     *
     * @param touchLayout 这副键盘的触屏布局（{@code KeyboardLayout} 的常量），功能键组按它列内容域
     * @param letter 同一副键盘的一个字母键（功能键的字以它的组为上限），九键为 {@code null}
     * @param cornerKey 带右上角提示的那个键，没有时为 {@code null}
     */
    private static void verifyFunctionRow(Product product, float density, float scale, int height, String where,
            int touchLayout, Button letter, Button cornerKey, Button[] keys, int[] widthsDp) throws Exception {
        Object functions = product.functionGroup(density, scale, touchLayout);
        if (letter != null) product.capMainAt(functions, product.faceGroup(letter));
        for (int index = 0; index < keys.length; index++) {
            keys[index].setLayoutParams(new android.view.ViewGroup.LayoutParams(
                android.view.ViewGroup.LayoutParams.MATCH_PARENT, android.view.ViewGroup.LayoutParams.MATCH_PARENT));
            product.joinFunction(keys[index], functions, keys[index] == cornerKey);
            layout(keys[index], Math.round(widthsDp[index] * density), height);
        }
        Object params = product.params(keys[0]);
        check(params != null, "功能键组没有算出参数：" + where);
        for (Button key : keys) check(product.params(key) == params, "同一组的功能键参数不同：" + where);
        if (letter != null)
            check(product.mainSize(params) <= product.mainSize(product.params(letter)) + 0.01f,
                "功能键的字大过了字母：" + product.mainSize(params) + " " + where);
        for (Button key : keys) {
            boolean icon = product.isIcon(key);
            String at = key.getText() + " " + key.getWidth() + "px " + where;
            if (!icon) key.scrollTo(-Math.round(26 * density), 0);
            Bitmap bitmap = Bitmap.createBitmap(key.getWidth(), key.getHeight(), Bitmap.Config.ARGB_8888);
            try {
                Canvas canvas = new Canvas(bitmap);
                canvas.drawColor(Color.WHITE);
                // 父视图画子视图时按它的滚动平移画布，这里照做。
                canvas.translate(-key.getScrollX(), -key.getScrollY());
                key.draw(canvas);
                Rect main = ink(bitmap, 1), corner = ink(bitmap, 0);
                check(!main.isEmpty(), "功能键没有画出来：" + at);
                check(main.left >= 0 && main.right <= key.getWidth() && main.top >= 0 && main.bottom <= key.getHeight(),
                    "功能键面出了键格：" + at);
                check(Math.abs(main.exactCenterY() - key.getHeight() / 2f) <= 2 * density,
                    "功能键面没有竖直居中：" + main + " " + at);
                float offset = icon ? 0 : product.mainShift(params);
                check(Math.abs(main.exactCenterX() - (key.getWidth() / 2f + offset)) <= 2 * density,
                    "功能键面没有水平居中：" + main + " 偏移 " + offset + " " + at);
                if (!corner.isEmpty()) check(!Rect.intersects(main, corner), "主标签压住角标：" + at);
            } finally { bitmap.recycle(); }
            key.scrollTo(0, 0);
            if (icon) continue;
            // 绘制前 TextView 会补建自己的排版；之后换成两行的文字（TextView 的排版高度跟着变）再换回来，都只重画。
            // 两行的文字是人为的，它补进内容域后参数可能变，这里只看布局。
            key.onPreDraw();
            CharSequence text = key.getText();
            key.setText(text.subSequence(0, 1) + "\n" + text.subSequence(1, text.length()));
            check(!key.isLayoutRequested(), "换成两行文字后请求了重新布局：" + at);
            key.setText(text);
            check(!key.isLayoutRequested(), "换回文字后请求了重新布局：" + at);
        }
    }

    /**
     * 九键：字母层的键（「ABC」「WXYZ」「分词」，右上角数字）和九键双拼的数字键（底部声韵提示）各一组。右上角数字按模板离上边和右边（12 dp 的设计值随字号缩放），主标签水平居中，墨迹不相碰不出键格，换数字只重画。
     */
    private static void verifyNineKeys(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Object letters = product.nineGroup("NINE_LETTERS", "letters", density, scale);
        Button[] keys = {product.nineKey("ABC", "2", ""), product.nineKey("WXYZ", "9", ""), product.nineKey("分词", "1", "")};
        for (Button key : keys) {
            product.joinNine(key, letters, "TOP_RIGHT");
            layout(key, Math.round(78 * density), height);
        }
        Object params = product.params(keys[0]);
        check(params != null, "九键字母组没有算出参数：" + where);
        for (Button key : keys) check(product.params(key) == params, "同一组的九键参数不同：" + where);
        for (Button key : keys) {
            String at = key.getText() + " " + where;
            Rect[] ink = draw(key);
            Rect main = ink[1], corner = ink[0];
            check(!main.isEmpty() && !corner.isEmpty(), "九键主标签或数字没画出来：" + at);
            check(Math.abs(main.exactCenterX() - (key.getWidth() / 2f + product.mainShift(params))) <= 2 * density,
                "九键主标签没有水平居中：" + main + " " + at);
            check(!Rect.intersects(main, corner), "九键主标签压住数字：" + at);
            if (product.hintSize(params, "topRight") > 0) {
                // 数字按文字框定位：排版宽度的右端离右边 insetX，所以墨迹右沿再往里一个字形的右侧留白（「1」的留白比「8」大得多）；数字的墨迹下沿就是基线。
                float right = key.getWidth() - corner.right, baseline = corner.bottom;
                float bearing = product.rightBearing(product.digitText(key), product.hintSize(params, "topRight"));
                check(Math.abs(right - product.hintInset(params, "insetXPx") - bearing) <= 2 * density,
                    "九键数字离右边不对：" + right + "（右侧留白 " + bearing + "）" + at);
                check(Math.abs(baseline - product.hintInset(params, "topBaseline")) <= 2 * density,
                    "九键数字的基线不对：" + baseline + " " + at);
            }
        }
        product.setDigit(keys[0], "");
        product.setDigit(keys[0], "2");
        check(!keys[0].isLayoutRequested() && product.params(keys[0]) == params, "换数字后请求了重新布局或参数变了：" + where);

        Object shuangpin = product.nineGroup("NINE_DIGITS", "digits", density, scale);
        Button[] digits = {product.nineKey("2", "", "bpa"), product.nineKey("8", "", "zhuang"), product.nineKey("5", "", "")};
        for (Button key : digits) {
            product.joinNine(key, shuangpin, "BOTTOM");
            layout(key, Math.round(78 * density), height);
        }
        Object digitParams = product.params(digits[0]);
        for (Button key : digits) {
            String at = key.getText() + " " + where;
            Rect[] ink = draw(key);
            check(!ink[1].isEmpty(), "九键双拼数字没画出来：" + at);
            if (!ink[2].isEmpty()) check(ink[1].bottom <= ink[2].top, "九键双拼数字压住提示：" + at);
            check(product.params(key) == digitParams, "同一组的九键双拼参数不同：" + where);
        }
    }

    /** 数字行：宽窄不同的数字键用同一份参数，数字水平居中、共用一条基线（数字一样高，墨迹的上下沿对齐），不出键格。 */
    private static void verifyNumberRow(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Object group = product.group("NUMBER_ROW", "app.msime.android.LetterKeyFaceDomains", "numberRow", Typeface.DEFAULT,
            density, scale);
        String[] faces = {"1", "7", "0"};
        int[] widthsDp = {30, 36, 42};
        Rect first = null;
        Object params = null;
        for (int index = 0; index < faces.length; index++) {
            Button key = product.plainKey(faces[index]);
            product.joinFunction(key, group, false);
            layout(key, Math.round(widthsDp[index] * density), height);
            if (params == null) params = product.params(key);
            check(params != null && product.params(key) == params, "数字行参数不同或没算出来：" + where);
            Rect main = draw(key)[1];
            String at = faces[index] + " " + where;
            check(!main.isEmpty() && main.left >= 0 && main.right <= key.getWidth() && main.top >= 0
                && main.bottom <= key.getHeight(), "数字没画出来或出了键格：" + at);
            check(Math.abs(main.exactCenterX() - (key.getWidth() / 2f + product.mainShift(params))) <= 2 * density,
                "数字没有水平居中：" + main + " " + at);
            if (first == null) first = main;
            else check(Math.abs(main.top - first.top) <= 2 && Math.abs(main.bottom - first.bottom) <= 2,
                "数字行的数字不在一条基线上：" + main + " 对 " + first + " " + at);
        }
    }

    /**
     * 符号栏：一格是三行键高的五分之一（{@code heightDp} 是一行键高），宽 55 dp。整栏一个组、一个字号（带 0 的 ！ 自己一组，不比整栏大），整栏不左移，每个符号按自己的墨迹居中（「，」「。」「、」不贴底，比中线低 0.1 个字号），不出格子；带 0 的 ！ 的 0 不压住 ！；换符号只重画。
     */
    private static void verifySideSymbols(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int width = Math.round(55 * density), height = Math.round(heightDp * 3 / 5f * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        List<String> symbols = List.of("，", "。", "、", "？", "……", "@", "！");
        Class<?> domains = product.loader.loadClass("app.msime.android.SideColumnKeyFaceDomains");
        Method domain = domains.getMethod("symbols", List.class, boolean.class);
        // 与 ImeKeyFaces.beginSideSymbols 相同：整栏一组，带 0 的 ！ 自己一组、以整栏为上限。
        Object group = product.group("SIDE_SYMBOLS", domain.invoke(null, symbols, false), Typeface.DEFAULT, density, scale);
        Object cornerGroup = product.group("SIDE_SYMBOLS", domain.invoke(null, List.of("！"), true), Typeface.DEFAULT, density,
            scale);
        cornerGroup.getClass().getMethod("capMainAt", cornerGroup.getClass()).invoke(cornerGroup, group);
        List<Button> keys = new ArrayList<>();
        for (String symbol : symbols.subList(0, symbols.size() - 1)) {
            Button key = product.plainKey(symbol);
            product.joinFunction(key, group, false);
            keys.add(key);
        }
        Button exclamation = product.nineKey("！", "0", "");
        product.joinNine(exclamation, cornerGroup, "TOP_RIGHT");
        keys.add(exclamation);
        for (Button key : keys) layout(key, width, height);
        Object params = product.params(keys.get(0));
        check(params != null && product.mainShift(params) == 0, "符号栏没有算出参数或整栏左移了：" + where);
        Object cornerParams = product.params(exclamation);
        check(cornerParams != null && product.mainSize(cornerParams) <= product.mainSize(params) + 0.01f,
            "带 0 的 ！ 不应比整栏的符号大：" + where);
        float tolerance = density + 1;
        for (Button key : keys) {
            String at = key.getText() + " " + where;
            check(key == exclamation || product.params(key) == params, "同一栏的符号参数不同：" + at);
            Rect[] ink = draw(key);
            Rect main = ink[1];
            check(!main.isEmpty() && main.left >= 0 && main.right <= width && main.top >= 0 && main.bottom <= height,
                "符号没画出来或出了格子：" + main + " " + at);
            float drop = symbols.indexOf(String.valueOf(key.getText())) < 3 ? 0.1f * product.mainSize(params) : 0;
            check(Math.abs(main.exactCenterX() - width / 2f - product.mainShift(product.params(key))) <= tolerance
                && Math.abs(main.exactCenterY() - height / 2f - drop) <= tolerance,
                "符号没有按自己的墨迹居中：" + main + " 格子 " + width + "x" + height + " " + at);
            if (key == exclamation && !ink[0].isEmpty()) check(!Rect.intersects(main, ink[0]), "！ 右上角的 0 压住了 ！：" + at);
        }
        // 「，」放大 1.3 倍画：墨迹比同字号的「，」高。
        Rect comma = draw(keys.get(0))[1];
        check(comma.height() >= 1.15f * product.inkHeight("，", product.mainSize(params)), "「，」没有放大画：" + comma + " " + where);
        keys.get(3).setText("；");
        check(!keys.get(3).isLayoutRequested() && product.params(keys.get(3)) == params, "换符号后请求了重新布局或参数变了：" + where);
    }

    /**
     * #+= 层：第一排和其余几排各一组，内容域由放进来的键自己的文字组成。同组一份参数，字符不出键格，成对的括号上下沿对齐（共用一条基线），第一排比其余几排大，换字符只重画。
     */
    private static void verifyLayer(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Object empty = product.loader.loadClass("app.msime.android.KeyFaceDomain").getConstructor(Map.class)
            .newInstance(new HashMap<>());
        Object top = product.group("LAYER_TOP_ROW", empty, Typeface.DEFAULT, density, scale);
        Object rest = product.group("LAYER_CHARACTERS", empty, Typeface.DEFAULT, density, scale);
        String[] topFaces = {"[", "]", "{", "}", "#", "%", "^", "*", "+", "="};
        String[] restFaces = {"_", "\\", "|", "~", "《", "》", "€", "&", "·", "…", "，", "？"};
        List<Button> topKeys = new ArrayList<>(), restKeys = new ArrayList<>();
        for (String face : topFaces) {
            Button key = product.plainKey(face);
            product.joinFunction(key, top, false);
            topKeys.add(key);
        }
        for (String face : restFaces) {
            Button key = product.plainKey(face);
            product.joinFunction(key, rest, false);
            restKeys.add(key);
        }
        int width = Math.round(34 * density);
        for (Button key : topKeys) layout(key, width, height);
        for (Button key : restKeys) layout(key, width, height);
        Object topParams = product.params(topKeys.get(0)), restParams = product.params(restKeys.get(0));
        check(topParams != null && restParams != null, "#+= 层没有算出参数：" + where);
        check(product.mainSize(topParams) >= product.mainSize(restParams), "#+= 层第一排应不小于其余几排：" + where);
        Rect open = null;
        for (List<Button> keys : List.of(topKeys, restKeys)) {
            Object params = product.params(keys.get(0));
            for (Button key : keys) {
                String at = key.getText() + " " + where;
                check(product.params(key) == params, "#+= 层同一组参数不同：" + at);
                Rect main = draw(key)[1];
                check(!main.isEmpty() && main.left >= 0 && main.right <= width && main.top >= 0 && main.bottom <= height,
                    "#+= 层字符没画出来或出了键格：" + main + " " + at);
                if ("[".contentEquals(key.getText())) open = main;
                if ("]".contentEquals(key.getText()))
                    check(open != null && Math.abs(main.top - open.top) <= 2 && Math.abs(main.bottom - open.bottom) <= 2,
                        "成对的括号不在一条基线上：" + main + " 对 " + open + " " + at);
            }
        }
        restKeys.get(0).setText("-");
        check(!restKeys.get(0).isLayoutRequested() && product.params(restKeys.get(0)) == restParams,
            "#+= 层换字符后请求了重新布局或参数变了：" + where);
    }

    /**
     * 26 键一族的其他文字：韩文字母（换双辅音只重画）、大千（声调按自己的墨迹居中、放大画，注音符号共用基线）、日文罗马字的 ー（自己一组，不比字母大）。同组一份参数，墨迹不出键格。
     */
    private static void verifyScripts(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density), width = Math.round(34 * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        String domains = "app.msime.android.LetterKeyFaceDomains";
        Object korean = product.group("KOREAN", domains, "korean", Typeface.DEFAULT, density, scale);
        Object dachen = product.group("DACHEN", domains, "dachen", Typeface.DEFAULT, density, scale);
        List<Button> keys = new ArrayList<>();
        for (String jamo : new String[] {"ㅂ", "ㅈ", "ㅏ", "ㅡ"}) {
            Button key = product.plainKey(jamo);
            product.joinFunction(key, korean, false);
            keys.add(key);
        }
        Button tone = null;
        for (String face : new String[] {"ㄅ", "ㄤ", "ˇ", "ㄦ"}) {
            Button key = product.plainKey(face);
            product.joinFunction(key, dachen, false);
            keys.add(key);
            if ("ˇ".equals(face)) tone = key;
        }
        for (Button key : keys) layout(key, width, height);
        float tolerance = density + 1;
        for (Button key : keys) {
            String at = key.getText() + " " + where;
            Object params = product.params(key);
            check(params != null && params == product.params(keys.get(key == tone || keys.indexOf(key) >= 4 ? 4 : 0)),
                "同组参数不同或没算出来：" + at);
            Rect main = draw(key)[1];
            check(!main.isEmpty() && main.left >= 0 && main.right <= width && main.top >= 0 && main.bottom <= height,
                "字没画出来或出了键格：" + main + " " + at);
        }
        Object toneParams = product.params(tone);
        Rect toneInk = draw(tone)[1];
        check(Math.abs(toneInk.exactCenterX() - width / 2f) <= tolerance
            && Math.abs(toneInk.exactCenterY() - height / 2f - 0.1f * product.mainSize(toneParams)) <= tolerance,
            "大千声调没有按自己的墨迹居中：" + toneInk + " " + where);
        check(toneInk.height() >= 1.5f * product.inkHeight("ˇ", product.mainSize(toneParams)), "大千声调没有放大画：" + where);
        Object koreanParams = product.params(keys.get(0));
        keys.get(0).setText("ㅃ");
        check(!keys.get(0).isLayoutRequested() && product.params(keys.get(0)) == koreanParams,
            "韩文换双辅音后请求了重新布局或参数变了：" + where);

        Object letters = product.letterGroup(density, scale);
        Button letter = product.plainKey("W");
        product.joinFunction(letter, letters, false);
        Object empty = product.loader.loadClass("app.msime.android.KeyFaceDomain").getConstructor(Map.class)
            .newInstance(new HashMap<>());
        Object vowel = product.group("LETTERS", empty, Typeface.DEFAULT, density, scale);
        vowel.getClass().getMethod("capMainAt", vowel.getClass()).invoke(vowel, letters);
        Button longVowel = product.plainKey("ー");
        product.joinFunction(longVowel, vowel, false);
        layout(letter, Math.round(30 * density), height);
        layout(longVowel, Math.round(30 * density), height);
        check(product.mainSize(product.params(longVowel)) <= product.mainSize(product.params(letter)) + 0.01f,
            "ー 不应比字母大：" + where);
        Rect bar = draw(longVowel)[1];
        check(!bar.isEmpty() && bar.left >= 0 && bar.right <= longVowel.getWidth(), "ー 没画出来或出了键格：" + bar + " " + where);
    }

    /**
     * 笔画键盘：笔画一组，@#、0、句点和 ！ 一组、以笔画为上限。同组一份参数，墨迹不出键格；「。」按自己的墨迹居中、放大画；中英文标点换句点只重画。
     */
    private static void verifyStroke(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density), width = Math.round(64 * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        String domains = "app.msime.android.NineKeyFaceDomains";
        Object strokes = product.group("STROKES", domains, "strokes", Typeface.DEFAULT, density, scale);
        Object others = product.group("STROKE_KEYS", domains, "strokeKeys", Typeface.DEFAULT, density, scale);
        others.getClass().getMethod("capMainAt", others.getClass()).invoke(others, strokes);
        List<Button> strokeKeys = new ArrayList<>(), otherKeys = new ArrayList<>();
        for (String face : new String[] {"一", "丨", "丿", "丶", "乛", "＊"}) {
            Button key = product.plainKey(face);
            product.joinFunction(key, strokes, false);
            strokeKeys.add(key);
        }
        for (String face : new String[] {"@#", "0", "。", "！"}) {
            Button key = product.plainKey(face);
            product.joinFunction(key, others, false);
            otherKeys.add(key);
        }
        for (Button key : strokeKeys) layout(key, width, height);
        for (Button key : otherKeys) layout(key, width, height);
        Object strokeParams = product.params(strokeKeys.get(0)), otherParams = product.params(otherKeys.get(0));
        check(strokeParams != null && otherParams != null, "笔画键盘没有算出参数：" + where);
        check(product.mainSize(otherParams) <= product.mainSize(strokeParams) + 0.01f, "@#、0、句点、！ 不应比笔画大：" + where);
        for (List<Button> keys : List.of(strokeKeys, otherKeys)) {
            Object params = product.params(keys.get(0));
            for (Button key : keys) {
                String at = key.getText() + " " + where;
                check(product.params(key) == params, "笔画键盘同一组参数不同：" + at);
                Rect main = draw(key)[1];
                check(!main.isEmpty() && main.left >= 0 && main.right <= width && main.top >= 0 && main.bottom <= height,
                    "笔画键盘的字没画出来或出了键格：" + main + " " + at);
            }
        }
        Button period = otherKeys.get(2);
        Rect periodInk = draw(period)[1];
        float tolerance = density + 1;
        check(Math.abs(periodInk.exactCenterX() - width / 2f) <= tolerance
            && Math.abs(periodInk.exactCenterY() - height / 2f - 0.1f * product.mainSize(otherParams)) <= tolerance,
            "「。」没有按自己的墨迹居中：" + periodInk + " " + where);
        check(periodInk.height() >= 1.15f * product.inkHeight("。", product.mainSize(otherParams)), "「。」没有放大画：" + where);
        period.setText(".");
        check(!period.isLayoutRequested() && product.params(period) == otherParams, "换句点后请求了重新布局或参数变了：" + where);
    }

    /**
     * 注音九键：声调一组（每个按自己的墨迹居中，比中线低 0.1 个字号），音键一组（注音在上、发送的数字在底部，两者不相碰、不出键格，换数字只重画），逗号句号一组（不比音键大；「。」按自己的墨迹居中、放大画，半角的 . 落在文字的基线上，不画成键面正中的点）。
     */
    private static void verifyZhuyinNineKey(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        String domains = "app.msime.android.NineKeyFaceDomains";
        float tolerance = density + 1;

        Object tones = product.group("ZHUYIN_TONES", domains, "zhuyinTones", Typeface.DEFAULT, density, scale);
        int toneWidth = Math.round(60 * density), toneHeight = Math.round(heightDp * 4 / 5f * density);
        List<Button> toneKeys = new ArrayList<>();
        for (String face : new String[] {"ˉ", "ˊ", "ˇ", "ˋ", "˙"}) {
            Button key = product.plainKey(face);
            product.joinFunction(key, tones, false);
            layout(key, toneWidth, toneHeight);
            toneKeys.add(key);
        }
        Object toneParams = product.params(toneKeys.get(0));
        check(toneParams != null, "注音九键声调没有算出参数：" + where);
        for (Button key : toneKeys) {
            String at = key.getText() + " " + where;
            check(product.params(key) == toneParams, "注音九键声调参数不同：" + at);
            Rect ink = draw(key)[1];
            check(!ink.isEmpty() && ink.left >= 0 && ink.right <= toneWidth && ink.top >= 0 && ink.bottom <= toneHeight,
                "声调没画出来或出了键格：" + ink + " " + at);
            check(Math.abs(ink.exactCenterX() - toneWidth / 2f) <= tolerance
                && Math.abs(ink.exactCenterY() - toneHeight / 2f - 0.1f * product.mainSize(toneParams)) <= tolerance,
                "声调没有按自己的墨迹居中：" + ink + " " + at);
        }

        Object sounds = product.group("ZHUYIN_SOUNDS", domains, "zhuyinSounds", Typeface.DEFAULT, density, scale);
        Class<?> bottomKey = product.loader.loadClass("app.msime.android.BottomLabelKey");
        Method setBottom = bottomKey.getMethod("setBottomText", String.class);
        int width = Math.round(80 * density);
        List<Button> soundKeys = new ArrayList<>();
        for (String[] face : new String[][] {{"ㄅㄆㄇㄈ", "1"}, {"ㄓㄔㄕㄖ", "5"}, {"ㄍㄎㄏ", "3"}, {"ㄢㄣㄤㄥ", "0"}}) {
            Button key = (Button) bottomKey.getConstructor(Context.class).newInstance(context);
            key.setBackground(null);
            key.setText(face[0]);
            key.setTextColor(Color.rgb(0, 200, 0));
            setBottom.invoke(key, face[1]);
            product.joinNine(key, sounds, "BOTTOM");
            layout(key, width, height);
            soundKeys.add(key);
        }
        Object soundParams = product.params(soundKeys.get(0));
        check(soundParams != null && product.hintSize(soundParams, "bottom") > 0, "注音九键音键没有算出参数或底部数字：" + where);
        for (Button key : soundKeys) {
            String at = key.getText() + " " + where;
            check(product.params(key) == soundParams, "注音九键音键参数不同：" + at);
            Rect whole = draw(key)[1];
            check(!whole.isEmpty() && whole.left >= 0 && whole.right <= width && whole.top >= 0 && whole.bottom <= height,
                "音键没画出来或出了键格：" + whole + " " + at);
        }
        // 数字与注音同色：分别只留一样来量墨迹。
        Button one = soundKeys.get(0);
        setBottom.invoke(one, "");
        Rect symbols = draw(one)[1];
        CharSequence label = one.getText();
        one.setText("");
        setBottom.invoke(one, "1");
        Rect digit = draw(one)[1];
        one.setText(label);
        check(!symbols.isEmpty() && !digit.isEmpty() && digit.top > symbols.bottom,
            "音键的数字没画在注音下面或压住了注音：" + digit + " 对 " + symbols + " " + where);
        check(Math.abs(digit.exactCenterX() - width / 2f) <= tolerance, "音键的数字没有水平居中：" + digit + " " + where);
        setBottom.invoke(one, "9");
        check(!one.isLayoutRequested() && product.params(one) == soundParams, "音键换数字后请求了重新布局或参数变了：" + where);

        Object marks = product.group("ZHUYIN_MARKS", domains, "zhuyinMarks", Typeface.DEFAULT, density, scale);
        marks.getClass().getMethod("capMainAt", marks.getClass()).invoke(marks, sounds);
        int half = Math.round(40 * density);
        Button period = product.plainKey("。");
        product.joinFunction(period, marks, false);
        layout(period, half, height);
        Object markParams = product.params(period);
        check(markParams != null && product.mainSize(markParams) <= product.mainSize(soundParams) + 0.01f,
            "注音九键逗号句号没有算出参数或比音键大：" + where);
        Rect full = draw(period)[1];
        check(!full.isEmpty() && full.left >= 0 && full.right <= half && full.top >= 0 && full.bottom <= height,
            "「。」没画出来或出了键格：" + full + " " + where);
        check(Math.abs(full.exactCenterX() - half / 2f) <= tolerance
            && Math.abs(full.exactCenterY() - height / 2f - 0.1f * product.mainSize(markParams)) <= tolerance,
            "注音九键的「。」没有按自己的墨迹居中：" + full + " " + where);
        // 最小字号上「。」的墨迹只有六七个像素，抗锯齿的边缘浅于取色阈值，量出来少一个像素。
        check(full.height() + 1 >= 1.15f * product.inkHeight("。", product.mainSize(markParams)),
            "注音九键的「。」没有放大画：" + full + " 字号 " + product.mainSize(markParams) + " " + where);
        period.setText(".");
        check(!period.isLayoutRequested() && product.params(period) == markParams, "换半角句点后请求了重新布局或参数变了：" + where);
        Rect dot = draw(period)[1];
        check(!dot.isEmpty() && dot.exactCenterY() >= height / 2f + 0.15f * product.mainSize(markParams),
            "半角的 . 应落在文字的基线上，不在键面正中：" + dot + " " + where);
    }

    /**
     * 日语九键，按 240 dp 宽的窄键盘（网格一格 56 dp、侧列 36 dp）：假名一组、数字一组，同组一份参数、不出键格；「小゛゜」和侧列的文字键放进功能键组（日语九键的内容域），一行画下、不出键格（原来固定 14 sp 的「小゛゜」在窄键盘加大字体时折成两行），侧列的字和「小゛゜」同一个字号。
     */
    private static void verifyJapaneseNineKey(Context context, int heightDp) throws Exception {
        Product product = new Product(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density), cell = Math.round(56 * density), side = Math.round(36 * density);
        int sideHeight = Math.round(heightDp * 4 / 5f * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        Class<?> nineDomains = product.loader.loadClass("app.msime.android.NineKeyFaceDomains");
        Method kanaDomain = nineDomains.getMethod("japaneseKana", boolean.class);
        for (boolean symbols : new boolean[] {false, true}) {
            Object group = product.group("JAPANESE_KANA", kanaDomain.invoke(null, symbols), Typeface.DEFAULT, density, scale);
            List<Button> keys = new ArrayList<>();
            for (String face : symbols ? new String[] {"1", "5", "0", "、"} : new String[] {"あ", "さ", "や", "わ", "、"}) {
                Button key = product.plainKey(face);
                product.joinFunction(key, group, false);
                layout(key, cell, height);
                keys.add(key);
            }
            Object params = product.params(keys.get(0));
            check(params != null, "日语九键" + (symbols ? "数字" : "假名") + "没有算出参数：" + where);
            for (Button key : keys) {
                String at = key.getText() + " " + where;
                check(product.params(key) == params, "日语九键同一组参数不同：" + at);
                Rect ink = draw(key)[1];
                check(!ink.isEmpty() && ink.left >= 0 && ink.right <= cell && ink.top >= 0 && ink.bottom <= height,
                    "日语九键的字没画出来或出了键格：" + ink + " " + at);
            }
        }

        Object functions = product.group("JAPANESE_FUNCTIONS", product.functionDomain(2),
            Typeface.DEFAULT, density, scale);
        Button variants = product.plainKey("小゛゜");
        product.joinFunction(variants, functions, false);
        layout(variants, cell, height);
        List<Button> sideKeys = new ArrayList<>();
        for (String face : new String[] {"123", "符", "切换", "空白", "英"}) {
            Button key = product.plainKey(face);
            product.joinFunction(key, functions, false);
            layout(key, side, sideHeight);
            sideKeys.add(key);
        }
        Object params = product.params(variants);
        check(params != null, "日语九键功能键没有算出参数：" + where);
        // 默认的 411 dp 宽、默认键高上侧列的字不比原来（按键宽缩到 14 sp 封顶）小：设计字号落在侧列一格的高度上。240 dp 宽时侧列只有 36 dp，要放下符号层的「あいう」，全组跟着小，那是一组一个字号的代价。
        if (heightDp == 52 && context.getResources().getConfiguration().fontScale == 1f) {
            Object wide = product.group("JAPANESE_FUNCTIONS", product.functionDomain(2),
                Typeface.DEFAULT, density, scale);
            Button space = product.plainKey("空白");
            product.joinFunction(space, wide, false);
            layout(space, Math.round(62 * density), sideHeight);
            check(product.mainSize(product.params(space)) >= 14 * density,
                "日语九键侧列的字比原来小：" + product.mainSize(product.params(space)) + " " + where);
        }
        Rect ink = draw(variants)[1];
        check(!ink.isEmpty() && ink.left >= 0 && ink.right <= cell && ink.top >= 0 && ink.bottom <= height
            && ink.height() < 1.6f * product.inkHeight("小", product.mainSize(params)),
            "「小゛゜」没有一行画在键格里：" + ink + " " + where);
        for (Button key : sideKeys) {
            String at = key.getText() + " " + where;
            check(product.params(key) == params, "日语九键侧列和「小゛゜」字号不同：" + at);
            Rect text = draw(key)[1];
            check(!text.isEmpty() && text.left >= 0 && text.right <= side && text.top >= 0 && text.bottom <= sideHeight,
                "侧列的字没画出来或出了键格：" + text + " " + at);
        }
        sideKeys.get(3).setText("変換");
        check(!sideKeys.get(3).isLayoutRequested() && product.params(sideKeys.get(3)) == params,
            "空白换成変換后请求了重新布局或参数变了：" + where);
    }

    /**
     * 键帽轮廓：每种键形和材质配上产品自己的键帽，字母键（窄到宽，带角标和底部提示）、九键字母和数字（双拼底部提示）、回车（「换行」角标）的墨迹都落在键帽面里。键帽面取键帽画出来的白色部分：凸起材质下方的厚度画得更暗，边框是灰色，都不算。
     */
    private static void verifyOutlines(Context context, int heightDp) throws Exception {
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        int height = Math.round(heightDp * density);
        String where = heightDp + "dp fontScale=" + context.getResources().getConfiguration().fontScale;
        for (String shape : new String[] {"capsule", "pebble", "ticket", "rounded"}) {
            for (String material : new String[] {"flat", "raised"}) {
                Product product = new Product(context);
                Object skin = product.designedSkin(shape, material);
                product.outline = product.outlineFor(skin, density);
                String at = shape + "/" + material + " " + where;
                Object letters = product.letterGroup(density, scale);
                String[][] faces = {{"W", "uang", "2"}, {"Q", "iu", "（"}, {"M", "ian", "@"}};
                int[] widthsDp = {28, 34, 48};
                for (int index = 0; index < faces.length; index++) {
                    Button key = product.letterKey(faces[index][0], faces[index][1], faces[index][2]);
                    product.join(key, letters);
                    layout(key, Math.round(widthsDp[index] * density), height);
                    inside(key, product.keyCap(skin, density), faces[index][0] + " " + widthsDp[index] + "dp " + at);
                }
                Object nine = product.nineGroup("NINE_LETTERS", "letters", density, scale);
                for (String[] face : new String[][] {{"WXYZ", "9"}, {"@#", "1"}}) {
                    Button key = product.nineKey(face[0], face[1], "");
                    product.joinNine(key, nine, "TOP_RIGHT");
                    layout(key, Math.round(78 * density), height);
                    inside(key, product.keyCap(skin, density), face[0] + " " + at);
                }
                Object digits = product.nineGroup("NINE_DIGITS", "digits", density, scale);
                Button digit = product.nineKey("8", "", "zhuang");
                product.joinNine(digit, digits, "BOTTOM");
                layout(digit, Math.round(78 * density), height);
                inside(digit, product.keyCap(skin, density), "九键双拼 " + at);
                Button enter = product.iconKey("RETURN", "");
                product.setIconCorner(enter, "换行");
                product.setKeyOutline(enter, product.outline);
                layout(enter, Math.round(55 * density), height);
                inside(enter, product.keyCap(skin, density), "回车 " + at);
            }
        }
    }

    /** 键面的墨迹（不画底色时不透明的像素）都落在键帽面（键帽单独画出来的白色像素）里。 */
    private static void inside(Button key, Drawable cap, String at) {
        int width = key.getWidth(), height = key.getHeight();
        Bitmap shape = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        Bitmap face = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        try {
            cap.setBounds(0, 0, width, height);
            cap.draw(new Canvas(shape));
            key.setBackground(null);
            key.draw(new Canvas(face));
            int[] capPixels = new int[width * height], facePixels = new int[width * height];
            shape.getPixels(capPixels, 0, width, 0, 0, width, height);
            face.getPixels(facePixels, 0, width, 0, 0, width, height);
            int outside = 0, inked = 0, firstX = -1, firstY = -1;
            for (int index = 0; index < capPixels.length; index++) {
                if (Color.alpha(facePixels[index]) < 96) continue;
                inked++;
                int pixel = capPixels[index];
                int red = Color.red(pixel), green = Color.green(pixel), blue = Color.blue(pixel);
                boolean surface = Color.alpha(pixel) > 200 && red > 205 && Math.abs(red - green) < 12
                    && Math.abs(green - blue) < 12;
                if (!surface && outside++ == 0) {
                    firstX = index % width;
                    firstY = index / width;
                }
            }
            check(inked > 0, "键面没有画出来：" + at);
            check(outside == 0, "键面有 " + outside + " 个像素落在键帽外（第一个在 " + firstX + "," + firstY + "，键 " + width + "×"
                + height + "）：" + at);
        } finally {
            shape.recycle();
            face.recycle();
        }
    }

    /** 画一个键，返回红（右上角）、绿（主标签）、蓝（底部）三种墨迹框。 */
    private static Rect[] draw(Button key) {
        Bitmap bitmap = Bitmap.createBitmap(key.getWidth(), key.getHeight(), Bitmap.Config.ARGB_8888);
        try {
            Canvas canvas = new Canvas(bitmap);
            canvas.drawColor(Color.WHITE);
            key.draw(canvas);
            return new Rect[] {ink(bitmap, 0), ink(bitmap, 1), ink(bitmap, 2)};
        } finally { bitmap.recycle(); }
    }

    private static void layout(Button key, int width, int height) {
        key.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
        key.layout(0, 0, width, height);
    }

    /** 墨迹框：0 红（角标）、1 绿（主标签）、2 蓝（底部提示）。 */
    private static Rect ink(Bitmap bitmap, int channel) {
        Rect bounds = new Rect();
        for (int y = 0; y < bitmap.getHeight(); y++) for (int x = 0; x < bitmap.getWidth(); x++) {
            int pixel = bitmap.getPixel(x, y);
            int red = Color.red(pixel), green = Color.green(pixel), blue = Color.blue(pixel);
            boolean selected = switch (channel) {
                case 0 -> red > green + 60 && red > blue + 60;
                case 1 -> green > red + 60 && green > blue + 60;
                default -> blue > red + 60 && blue > green + 60;
            };
            if (selected) bounds.union(x, y, x + 1, y + 1);
        }
        return bounds;
    }

    /** 经产品的类加载器构造产品类，测的是装在设备上的那一份。 */
    private static final class Product {
        private final Context context;
        private final ClassLoader loader;
        private final Class<?> slot;
        private final Class<?> hintButton;
        private final Class<?> pressButton;

        /** 键组环境里的键帽轮廓；{@code null} 时没有轮廓（只受键格限制）。 */
        Object outline;

        Object environment(float density, float fontScale) throws Exception {
            Class<?> type = loader.loadClass("app.msime.android.KeyFaceEnvironment");
            if (outline == null) return type.getConstructor(float.class, float.class).newInstance(density, fontScale);
            return type.getConstructor(float.class, float.class, loader.loadClass("app.msime.android.KeyOutline"))
                .newInstance(density, fontScale, outline);
        }

        /** 产品的设计皮肤：中性配色，键形 {@code shape}、材质 {@code material}。 */
        Object designedSkin(String shape, String material) throws Exception {
            JSONObject design = new JSONObject().put("background", 0xE6EBF0).put("keyBackground", 0xFFFFFF)
                .put("keyForeground", 0x1F2933).put("accent", 0x2F6FB0).put("actionBackground", 0x2F6FB0)
                .put("cornerRadius", 18).put("borderWidth", 0.5).put("customBorderColor", 0xB8C2CC).put("shadow", 0.1)
                .put("keyOpacity", 1).put("keyShape", shape).put("keyMaterial", material);
            return loader.loadClass("app.msime.android.KeyboardSkin").getMethod("custom", JSONObject.class, boolean.class)
                .invoke(null, design, false);
        }

        Object outlineFor(Object skin, float density) throws Exception {
            Class<?> skinType = loader.loadClass("app.msime.android.KeyboardSkin");
            return loader.loadClass("app.msime.android.KeyOutline").getMethod("forSkin", skinType, float.class)
                .invoke(null, skin, density);
        }

        /** 产品给这个皮肤画的白色键帽。 */
        Drawable keyCap(Object skin, float density) throws Exception {
            return (Drawable) loader.loadClass("app.msime.android.KeyboardSkinKeyDrawable")
                .getConstructor(loader.loadClass("app.msime.android.KeyboardSkin"), int.class, boolean.class, float.class)
                .newInstance(skin, Color.WHITE, false, density);
        }

        void setIconCorner(Button key, String text) throws Exception {
            key.getClass().getMethod("setCornerHint", String.class).invoke(key, text);
        }

        void setKeyOutline(Button key, Object value) throws Exception {
            key.getClass().getMethod("setKeyOutline", loader.loadClass("app.msime.android.KeyOutline")).invoke(key, value);
        }

        Product(Context context) throws Exception {
            this.context = context;
            loader = context.getClassLoader();
            slot = loader.loadClass("app.msime.android.KeyFaceSlot");
            hintButton = loader.loadClass("app.msime.android.ShuangpinHintButton");
            pressButton = loader.loadClass("app.msime.android.KeyboardPressButton");
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        Object slot(String name) { return Enum.valueOf((Class) slot, name); }

        Object letterGroup(float density, float fontScale) throws Exception {
            Object template = loader.loadClass("app.msime.android.KeyFaceTemplates").getField("LETTERS").get(null);
            Object environment = environment(density, fontScale);
            Map<Object, List<String>> texts = new HashMap<>();
            List<String> faces = new ArrayList<>();
            for (char letter = 'A'; letter <= 'Z'; letter++) faces.add(String.valueOf(letter));
            texts.put(slot("MAIN"), faces);
            texts.put(slot("TOP_RIGHT"), List.of("1", "2", "?", "（", "@"));
            texts.put(slot("BOTTOM"), List.of("ei", "iu", "ian", "uang"));
            Object domain = loader.loadClass("app.msime.android.KeyFaceDomain").getConstructor(Map.class).newInstance(texts);
            Object measurer = loader.loadClass("app.msime.android.PaintKeyFaceMeasurer")
                .getConstructor(Typeface.class).newInstance(Typeface.DEFAULT);
            Class<?> templateType = loader.loadClass("app.msime.android.KeyFaceTemplate");
            Class<?> environmentType = loader.loadClass("app.msime.android.KeyFaceEnvironment");
            Class<?> domainType = loader.loadClass("app.msime.android.KeyFaceDomain");
            Class<?> measurerType = loader.loadClass("app.msime.android.KeyFaceMeasurer");
            return loader.loadClass("app.msime.android.KeyFaceGroup")
                .getConstructor(templateType, environmentType, domainType, measurerType)
                .newInstance(template, environment, domain, measurer);
        }

        /** 触屏布局 {@code touchLayout} 上键区功能键组的内容域（{@code KeyFaceSpecs.functionDomain}），按全拼可分词、默认设置。 */
        Object functionDomain(int touchLayout) throws Exception {
            return loader.loadClass("app.msime.android.KeyFaceSpecs")
                .getMethod("functionDomain", loader.loadClass("app.msime.android.KeyFaceLayout")).invoke(null, layout(touchLayout));
        }

        /** 触屏布局 {@code touchLayout} 的 {@code KeyFaceLayout}：全拼可分词、九键 1 键组字时分词，其余按默认设置。 */
        Object layout(int touchLayout) throws Exception {
            Class<?> layoutType = loader.loadClass("app.msime.android.KeyFaceLayout");
            Object marks = loader.loadClass("app.msime.android.ChineseSymbolFaces$Marks").getField("DEFAULT").get(null);
            return layoutType.getConstructors()[0].newInstance(touchLayout, false, "separate", false, marks, false,
                List.of(), List.of());
        }

        Object functionGroup(float density, float fontScale, int touchLayout) throws Exception {
            Object template = loader.loadClass("app.msime.android.KeyFaceTemplates").getField("FUNCTIONS").get(null);
            Object environment = environment(density, fontScale);
            Object domain = functionDomain(touchLayout);
            Object measurer = loader.loadClass("app.msime.android.PaintKeyFaceMeasurer")
                .getConstructor(Typeface.class).newInstance(Typeface.DEFAULT);
            return loader.loadClass("app.msime.android.KeyFaceGroup")
                .getConstructor(loader.loadClass("app.msime.android.KeyFaceTemplate"),
                    loader.loadClass("app.msime.android.KeyFaceEnvironment"),
                    loader.loadClass("app.msime.android.KeyFaceDomain"),
                    loader.loadClass("app.msime.android.KeyFaceMeasurer"))
                .newInstance(template, environment, domain, measurer);
        }

        void capMainAt(Object group, Object leader) throws Exception {
            Class<?> type = loader.loadClass("app.msime.android.KeyFaceGroup");
            type.getMethod("capMainAt", type).invoke(group, leader);
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        Button iconKey(String kind, String text) throws Exception {
            Class<?> iconKey = loader.loadClass("app.msime.android.KeyboardIconKey");
            Class kinds = loader.loadClass("app.msime.android.KeyboardIconKey$Kind");
            Button key = (Button) iconKey.getConstructor(Context.class, kinds).newInstance(context, Enum.valueOf(kinds, kind));
            key.setBackground(null);
            key.setText(text);
            key.setTextColor(Color.rgb(0, 200, 0));
            return key;
        }

        Button plainKey(String text) throws Exception {
            Button key = (Button) pressButton.getConstructor(Context.class).newInstance(context);
            key.setBackground(null);
            key.setText(text);
            key.setTextColor(Color.rgb(0, 200, 0));
            return key;
        }

        boolean isIcon(Button key) throws Exception {
            Class<?> iconKey = loader.loadClass("app.msime.android.KeyboardIconKey");
            return iconKey.isInstance(key) && !(boolean) iconKey.getMethod("showsText").invoke(key);
        }

        /** 一个组：模板 {@code template}，内容域取 {@code domainClass} 的无参方法 {@code domainMethod}，主标签用 {@code typeface}。 */
        Object group(String template, String domainClass, String domainMethod, Typeface typeface, float density,
                float fontScale) throws Exception {
            Object spec = loader.loadClass("app.msime.android.KeyFaceTemplates").getField(template).get(null);
            Object environment = environment(density, fontScale);
            Object texts = loader.loadClass(domainClass).getMethod(domainMethod).invoke(null);
            Object measurer = loader.loadClass("app.msime.android.PaintKeyFaceMeasurer").getConstructor(Typeface.class)
                .newInstance(typeface);
            return loader.loadClass("app.msime.android.KeyFaceGroup")
                .getConstructor(loader.loadClass("app.msime.android.KeyFaceTemplate"),
                    loader.loadClass("app.msime.android.KeyFaceEnvironment"),
                    loader.loadClass("app.msime.android.KeyFaceDomain"),
                    loader.loadClass("app.msime.android.KeyFaceMeasurer"))
                .newInstance(spec, environment, texts, measurer);
        }

        /** 一个组：模板 {@code template}，内容域 {@code domain} 已经建好。 */
        Object group(String template, Object domain, Typeface typeface, float density, float fontScale) throws Exception {
            Object spec = loader.loadClass("app.msime.android.KeyFaceTemplates").getField(template).get(null);
            Object measurer = loader.loadClass("app.msime.android.PaintKeyFaceMeasurer").getConstructor(Typeface.class)
                .newInstance(typeface);
            return loader.loadClass("app.msime.android.KeyFaceGroup")
                .getConstructor(loader.loadClass("app.msime.android.KeyFaceTemplate"),
                    loader.loadClass("app.msime.android.KeyFaceEnvironment"),
                    loader.loadClass("app.msime.android.KeyFaceDomain"),
                    loader.loadClass("app.msime.android.KeyFaceMeasurer"))
                .newInstance(spec, environment(density, fontScale), domain, measurer);
        }

        /** 默认字体里 {@code text} 在 {@code sizePx} 字号下的墨迹高。 */
        float inkHeight(String text, float sizePx) {
            android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
            paint.setTextSize(sizePx);
            Rect bounds = new Rect();
            paint.getTextBounds(text, 0, text.length(), bounds);
            return bounds.height();
        }

        /** 九键的组：模板 {@code template}，内容域取 {@code NineKeyFaceDomains} 的 {@code domain}（双拼用一张小表），字体同产品的九键。 */
        Object nineGroup(String template, String domain, float density, float fontScale) throws Exception {
            Object spec = loader.loadClass("app.msime.android.KeyFaceTemplates").getField(template).get(null);
            Object environment = environment(density, fontScale);
            Class<?> domains = loader.loadClass("app.msime.android.NineKeyFaceDomains");
            Object texts = "letters".equals(domain) ? domains.getMethod("letters", loader.loadClass(
                "app.msime.android.KeyFaceLayout")).invoke(null, layout(1)) : domains.getMethod("digits").invoke(null);
            Object measurer = loader.loadClass("app.msime.android.PaintKeyFaceMeasurer").getConstructor(Typeface.class)
                .newInstance(Typeface.create("sans-serif-medium", Typeface.NORMAL));
            return loader.loadClass("app.msime.android.KeyFaceGroup")
                .getConstructor(loader.loadClass("app.msime.android.KeyFaceTemplate"),
                    loader.loadClass("app.msime.android.KeyFaceEnvironment"),
                    loader.loadClass("app.msime.android.KeyFaceDomain"),
                    loader.loadClass("app.msime.android.KeyFaceMeasurer"))
                .newInstance(spec, environment, texts, measurer);
        }

        Button nineKey(String label, String digit, String bottom) throws Exception {
            Class<?> nine = loader.loadClass("app.msime.android.NineKeyDigitButton");
            Button key = (Button) nine.getConstructor(Context.class).newInstance(context);
            key.setBackground(null);
            key.setText(label);
            key.setTextColor(Color.rgb(0, 200, 0));
            nine.getMethod("setDigitColor", int.class).invoke(key, Color.rgb(220, 0, 0));
            hintButton.getMethod("setHintColor", int.class).invoke(key, Color.rgb(0, 0, 220));
            setDigit(key, digit);
            setBottom(key, bottom);
            return key;
        }

        /** 九键右上角此刻画的数字（键面的 {@code TOP_RIGHT} 文字）。 */
        String digitText(Button key) throws Exception {
            java.lang.reflect.Method method = key.getClass().getMethod("faceText", slot);
            return String.valueOf(method.invoke(key, slot("TOP_RIGHT")));
        }

        /** 提示用系统默认字体：{@code text} 在 {@code sizePx} 字号下排版宽度的右端比墨迹右沿多出多少。 */
        float rightBearing(String text, float sizePx) {
            android.graphics.Paint paint = new android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG);
            paint.setTypeface(Typeface.DEFAULT);
            paint.setTextSize(sizePx);
            Rect bounds = new Rect();
            paint.getTextBounds(text, 0, text.length(), bounds);
            return paint.measureText(text) - bounds.right;
        }

        void setDigit(Button key, String digit) throws Exception {
            key.getClass().getMethod("setDigitText", String.class).invoke(key, digit);
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        void joinNine(Button key, Object group, String hintSlot) throws Exception {
            Set slots = EnumSet.of((Enum) slot(hintSlot));
            pressButton.getMethod("setFaceGroup", loader.loadClass("app.msime.android.KeyFaceGroup"), Set.class)
                .invoke(key, group, slots);
        }

        float hintSize(Object params, String which) throws Exception {
            Object hint = params.getClass().getMethod(which).invoke(params);
            return hint == null ? 0 : (float) hint.getClass().getMethod("sizePx").invoke(hint);
        }

        float hintInset(Object params, String inset) throws Exception {
            Object hint = params.getClass().getMethod("topRight").invoke(params);
            return (float) hint.getClass().getMethod(inset).invoke(hint);
        }

        Object faceGroup(Button key) throws Exception { return pressButton.getMethod("faceGroup").invoke(key); }

        /** 按键面规格画文字（{@code KeyboardIconKey.setFace}）。 */
        void setTextFace(Button key, String text) throws Exception {
            Class<?> faceType = loader.loadClass("app.msime.android.KeyFace");
            key.getClass().getMethod("setFace", faceType).invoke(key, faceType.getMethod("text", String.class).invoke(null, text));
        }

        /** 按键面规格画图标 {@code kind}（{@code KeyboardIconKey.Kind} 的名字）。 */
        @SuppressWarnings({"unchecked", "rawtypes"})
        void setIconFace(Button key, String kind) throws Exception {
            Class<?> faceType = loader.loadClass("app.msime.android.KeyFace");
            Class kindType = loader.loadClass("app.msime.android.KeyboardIconKey$Kind");
            key.getClass().getMethod("setFace", faceType).invoke(key,
                faceType.getMethod("icon", kindType).invoke(null, Enum.valueOf(kindType, kind)));
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        void joinFunction(Button key, Object group, boolean corner) throws Exception {
            Set slots = corner ? EnumSet.of((Enum) slot("TOP_RIGHT")) : EnumSet.noneOf((Class) slot);
            pressButton.getMethod("setFaceGroup", loader.loadClass("app.msime.android.KeyFaceGroup"), Set.class)
                .invoke(key, group, slots);
        }

        Button letterKey(String label, String bottom, String corner) throws Exception {
            Button key = (Button) hintButton.getConstructor(Context.class).newInstance(context);
            key.setBackground(null);
            key.setText(label);
            key.setTextColor(Color.rgb(0, 200, 0));
            hintButton.getMethod("setHintColor", int.class).invoke(key, Color.rgb(0, 0, 220));
            hintButton.getMethod("setCornerHintColor", int.class).invoke(key, Color.rgb(220, 0, 0));
            setBottom(key, bottom);
            setCorner(key, corner);
            return key;
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        void join(Button key, Object group) throws Exception {
            Set slots = EnumSet.of((Enum) slot("TOP_RIGHT"), (Enum) slot("BOTTOM"));
            pressButton.getMethod("setFaceGroup", loader.loadClass("app.msime.android.KeyFaceGroup"), Set.class)
                .invoke(key, group, slots);
        }

        void setCorner(Button key, String text) throws Exception {
            hintButton.getMethod("setCornerHint", String.class).invoke(key, text);
        }

        void setBottom(Button key, String text) throws Exception {
            hintButton.getMethod("setHintText", String.class).invoke(key, text);
        }

        Object params(Button key) throws Exception { return pressButton.getMethod("faceParams").invoke(key); }

        float mainSize(Object params) throws Exception {
            Object main = params.getClass().getMethod("main").invoke(params);
            return (float) main.getClass().getMethod("sizePx").invoke(main);
        }

        float mainShift(Object params) throws Exception {
            Object main = params.getClass().getMethod("main").invoke(params);
            return (float) main.getClass().getMethod("shiftXPx").invoke(main);
        }

        boolean hidesBottom(Object params) throws Exception {
            return params.getClass().getMethod("bottom").invoke(params) == null;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
