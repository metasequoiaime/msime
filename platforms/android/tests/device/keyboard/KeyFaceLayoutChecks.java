package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/**
 * 真实键盘上的键组：离屏建一个输入法服务，按引擎视图依次画各种键盘（26 键、14/18 键、拼音九键、笔画、大千注音、注音九键、日语九键、韩文），每种在 411/360/320/240 dp 宽下各画字母层空闲、字母层组字和 123 层三种状态，每次都走一遍渲染。检查两件事：
 *
 * <ul>
 * <li>键画上的字都在它的组列出来的内容域里（{@code KeyFaceGroup.strayCount} 不增加）：列表漏了的字要等画上时才补进组，键会在打字中途变小。</li>
 * <li>作用相同的键一样大：角色表里没写明有意宽窄不一（{@code KeyFaceRole.mixedWidths}）的组，最宽和最窄、最高和最矮的键都差不过一成五。全组按最小的键格定字号，差得多时大键上的字显得小。</li>
 * </ul>
 *
 * <p>只看产品自己建的组；{@link KeyFaceGroupChecks} 的合成键会有意换上列表外的字，所以这里先跑、按前后的计数差判断。
 */
final class KeyFaceLayoutChecks {
    private static final int[] WIDTHS_DP = {411, 360, 320, 240};
    private static final float MAX_SPREAD = 1.15f;
    /** 名字、引擎视图（与按键放得下的检查同一组）。scheme：0 全拼，3 日文，4 韩文，6 注音，9 笔画。 */
    private static final String[][] LAYOUTS = {
        {"26 键", "{\"scheme\":0}"},
        {"14 键", "{\"scheme\":0,\"key_grid\":\"fourteen_key\"}"},
        {"18 键", "{\"scheme\":0,\"key_grid\":\"eighteen_key\"}"},
        {"拼音九键", "{\"scheme\":0,\"nine_key\":true}"},
        {"笔画", "{\"scheme\":9}"},
        {"大千注音", "{\"scheme\":6}"},
        {"注音九键", "{\"scheme\":6,\"touch_keyboard_layout\":\"nine_key\"}"},
        {"日语九键", "{\"scheme\":3,\"touch_keyboard_layout\":\"nine_key\"}"},
        {"韩文", "{\"scheme\":4}"},
    };

    private final Instrumentation test;
    private final List<String> problems = new ArrayList<>();
    /** 每个角色见过的最大键格差（宽、高），打印出来供核对角色表。 */
    private final Map<String, float[]> spreads = new TreeMap<>();
    private Class<?> type;
    private Object service;

    private KeyFaceLayoutChecks(Instrumentation test) { this.test = test; }

    static void verify(Instrumentation test) throws Exception { new KeyFaceLayoutChecks(test).run(); }

    private void run() throws Exception {
        Activity activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        AtomicReference<Throwable> failure = new AtomicReference<>();
        // 每种状态单独在主线程上跑一次，中间把主线程让出来：一次跑完一百多种状态会挡住输入事件，被系统判成无响应。
        FrameLayout[] holder = new FrameLayout[1];
        int[] strays = new int[1];
        try {
            onMain(failure, () -> {
                holder[0] = create(activity);
                strays[0] = strayCount();
            });
            for (String[] layout : LAYOUTS) {
                for (int widthDp : WIDTHS_DP) {
                    for (String state : new String[] {"空闲", "组字", "123 层"}) {
                        if (failure.get() != null) break;
                        onMain(failure, () -> {
                            String where = layout[0] + " " + widthDp + "dp " + state;
                            JSONObject view = new JSONObject(layout[1]);
                            if (state.equals("组字")) view.put("editing_text", "a");
                            show(holder[0], view, widthDp, state.equals("123 层"));
                            int now = strayCount();
                            if (now != strays[0])
                                problems.add(where + "：" + (now - strays[0]) + " 段字不在键组的内容域里（见 logcat 的 key face list misses）");
                            strays[0] = now;
                            inspect(where);
                        });
                    }
                }
            }
        } finally {
            test.runOnMainSync(() -> { if (service != null) destroy(); });
            test.runOnMainSync(activity::finish);
            // 等这个界面真的关掉、失焦事件派完再返回：随后的检查会长时间占着主线程，失焦事件排在后面超过 5 秒就被判成无响应。
            for (int waited = 0; waited < 50 && !activity.isDestroyed(); waited++) Thread.sleep(100);
            test.waitForIdleSync();
        }
        if (failure.get() != null) throw new AssertionError("键组布局检查没跑完：" + failure.get(), failure.get());
        for (Map.Entry<String, float[]> entry : spreads.entrySet())
            System.out.println("key face spread " + entry.getKey() + " width ×" + entry.getValue()[0] + " height ×"
                + entry.getValue()[1]);
        if (!problems.isEmpty()) {
            List<String> shown = problems.subList(0, Math.min(problems.size(), 25));
            throw new AssertionError(problems.size() + " 处键组问题：" + String.join("；", shown));
        }
    }

    private interface Step { void run() throws Exception; }

    /** 在主线程上跑一步，出错时记下第一个错误。 */
    private void onMain(AtomicReference<Throwable> failure, Step step) {
        test.runOnMainSync(() -> {
            try { step.run(); }
            catch (Exception | AssertionError error) { if (failure.get() == null) failure.set(error); }
        });
    }

    /** 新建一个离屏输入法服务，键盘放进一个可改宽度的容器。 */
    private FrameLayout create(Activity activity) throws Exception {
        Context target = test.getTargetContext();
        type = target.getClassLoader().loadClass("app.msime.android.MSIMEInputService");
        service = type.getConstructor().newInstance();
        method(ContextWrapper.class, "attachBaseContext", Context.class).invoke(service, target);
        type.getMethod("onCreate").invoke(service);
        View root = (View) type.getMethod("onCreateInputView").invoke(service);
        if (root.getParent() instanceof ViewGroup parent) parent.removeView(root);
        FrameLayout holder = new FrameLayout(activity);
        holder.addView(root, new FrameLayout.LayoutParams(1, ViewGroup.LayoutParams.WRAP_CONTENT));
        activity.setContentView(holder);
        return holder;
    }

    private void destroy() {
        try { type.getMethod("onDestroy").invoke(service); }
        catch (ReflectiveOperationException ignored) { /* 清理离屏服务。 */ }
        for (Field member : type.getDeclaredFields()) {
            if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
            try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
            catch (ReflectiveOperationException ignored) { /* 清理工作线程。 */ }
        }
        service = null;
    }

    /** 按引擎视图和键层重建键行和底行、走一遍渲染，量到 {@code widthDp} 宽。 */
    private void show(FrameLayout holder, JSONObject view, int widthDp, boolean symbols) throws Exception {
        field(type, "view").set(service, view);
        Class<?> layer = test.getTargetContext().getClassLoader().loadClass("app.msime.android.KeyboardLayout$Layer");
        field(type, "keyboardLayer").set(service, layer.getField(symbols ? "SYMBOLS" : "LETTERS").get(null));
        Object letters = field(type, "imeLetterRows").get(service);
        method(letters.getClass(), "rebuildKeyRows").invoke(letters);
        method(type, "render").invoke(service);
        View root = holder.getChildAt(0);
        int width = Math.round(widthDp * test.getTargetContext().getResources().getDisplayMetrics().density);
        root.getLayoutParams().width = width;
        int widthSpec = View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY);
        int heightSpec = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        root.measure(widthSpec, heightSpec);
        root.layout(0, 0, width, root.getMeasuredHeight());
        // 画一遍：键在绘制前核对自己的字在不在内容域里（KeyFaceRenderer）。
        android.graphics.Bitmap bitmap = android.graphics.Bitmap.createBitmap(Math.max(1, width),
            Math.max(1, root.getMeasuredHeight()), android.graphics.Bitmap.Config.ARGB_8888);
        root.draw(new android.graphics.Canvas(bitmap));
        bitmap.recycle();
    }

    /** 角色表里应当一样大的组：最宽和最窄、最高和最矮的键差不过一成五。 */
    private void inspect(String where) throws Exception {
        Object faces = field(type, "imeKeyFaces").get(service);
        Map<?, ?> groups = (Map<?, ?>) method(faces.getClass(), "groups").invoke(faces);
        for (Map.Entry<?, ?> entry : groups.entrySet()) {
            Object role = entry.getKey();
            float[] spread = (float[]) entry.getValue().getClass().getMethod("spread").invoke(entry.getValue());
            if (spread[0] == 0) continue;
            float[] seen = spreads.computeIfAbsent(String.valueOf(role), name -> new float[2]);
            seen[0] = Math.max(seen[0], spread[0]);
            seen[1] = Math.max(seen[1], spread[1]);
            Object mixed = role.getClass().getMethod("mixedWidths").invoke(role);
            if (mixed == null && (spread[0] > MAX_SPREAD || spread[1] > MAX_SPREAD)) {
                int[] range = (int[]) entry.getValue().getClass().getMethod("cellRange").invoke(entry.getValue());
                problems.add(where + "：" + role + " 的键宽窄不一（宽 " + range[0] + "–" + range[1] + " px ×" + spread[0]
                    + "，高 " + range[2] + "–" + range[3] + " px ×" + spread[1] + "）");
            }
        }
    }

    private int strayCount() throws Exception {
        return (int) test.getTargetContext().getClassLoader().loadClass("app.msime.android.KeyFaceGroup")
            .getMethod("strayCount").invoke(null);
    }

    private static Field field(Class<?> type, String name) throws Exception {
        for (Class<?> at = type; at != null; at = at.getSuperclass()) {
            try {
                Field field = at.getDeclaredField(name);
                field.setAccessible(true);
                return field;
            } catch (NoSuchFieldException ignored) { /* 往父类找。 */ }
        }
        throw new NoSuchFieldException(name);
    }

    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        for (Class<?> at = type; at != null; at = at.getSuperclass()) {
            try {
                Method method = at.getDeclaredMethod(name, parameters);
                method.setAccessible(true);
                return method;
            } catch (NoSuchMethodException ignored) { /* 往父类找。 */ }
        }
        throw new NoSuchMethodException(name);
    }
}
