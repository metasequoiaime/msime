package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;

/**
 * 按键区按下即反馈（#10757）的视图接线：在真实的 `KeyboardKeyArea` 上分发合成触摸，核对 `PressListener` 的回调。直接命中、落在空隙里归给拥有空隙的键、回车把左侧一段让给中/英、落在指定了所属键的占位格（`KeyboardSpacer`，#11051）里归给那个键而没有指定的占位格不接收、多指各自按下抬起、不可用的键不回调、`ACTION_CANCEL` 和滑行接管之后不再回调抬起；每个令牌交还一次，抬起那一次排在框架投递的点击后面。
 *
 * <p>规则本身（令牌的武装、消耗、撤回）由 JVM 上的 `KeyPressFeedbackPolicySmoke` 覆盖；这里只验证视图层把哪个键、在什么时候交给它。经 `OptionSheetDeviceSmoke -e key-feedback press-area` 运行，不增加 instrumentation 入口。键是普通 `Button`，不经输入法服务，不出声不振动。
 */
final class KeyPressAreaDeviceChecks {
    /** 键 100 px 见方，四周各留 10 px 外边距：A 占 10–110，B 占 130–230，C 占 250–350，键与键之间是 20 px 的空隙。C 的外边距到 360 为止，之后是两个 20 px 宽的占位格：360–380 归 C，380–400 不归任何键。 */
    private static final int KEY = 100;
    private static final int MARGIN = 10;
    private static final int SPACER = 20;
    private static final float MIDDLE_Y = MARGIN + KEY / 2f;

    private final DeviceSmoke test;
    private final List<String> log = new ArrayList<>();
    private Class<?> areaType;
    private Object area;
    private Button a;
    private Button b;
    private Button c;
    private long downTime;
    /** 滑行接管从这一刻起：为 true 时 `GlideTracker.track` 对之后的每个事件都返回 true。 */
    private boolean glideTakes;

    private KeyPressAreaDeviceChecks(DeviceSmoke test) { this.test = test; }

    static void run(DeviceSmoke test) throws Exception { new KeyPressAreaDeviceChecks(test).run(); }

    private void run() throws Exception {
        Activity activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        try {
            onMain(() -> activity.setContentView(build(activity)));
            test.waitForIdleSync();
            onMain(() -> {
                if (a.getWidth() != KEY || b.getLeft() != KEY + 3 * MARGIN)
                    throw new AssertionError("Synthetic key area was not laid out as specified");
            });

            test.stage = "key press area: a direct tap reports the key on press and on release";
            onMain(() -> {
                tap(MARGIN + KEY / 2f);
            });
            expectSettled(List.of("pressed:A", "releasing:A", "settled:pressed:A", "click:A", "settled:releasing:A"));

            test.stage = "key press area: a press in the gap reports the key that owns the gap";
            onMain(() -> {
                // 114 落在 A 右侧的外边距里（A 的右沿是 110），离 B 的外边距（从 120 起）还远。
                tap(MARGIN + KEY + 4f);
            });
            expectSettled(List.of("pressed:A", "releasing:A", "settled:pressed:A", "click:A", "settled:releasing:A"));

            test.stage = "key press area: the yielded strip of a key reports the key it yields to";
            onMain(() -> {
                setYield(b, a, 20f);
                // 135 在 B 键帽的左侧 5 px，落在让给 A 的那一段里，和回车把左沿让给中/英一样。
                tap(KEY + 3 * MARGIN + 5f);
                setYield(null, null, 0f);
            });
            expectSettled(List.of("pressed:A", "releasing:A", "settled:pressed:A", "click:A", "settled:releasing:A"));

            test.stage = "key press area: a press in a spacer that names a key reports that key";
            onMain(() -> {
                // 370 在归 C 的占位格中间，离 C 的外边距（到 360）还有 10 px：没有占位格的归属，这一下会丢。
                tap(3 * (KEY + 2 * MARGIN) + SPACER / 2f);
            });
            expectSettled(List.of("pressed:C", "releasing:C", "settled:pressed:C", "click:C", "settled:releasing:C"));

            test.stage = "key press area: a press in a spacer without a key is dropped";
            onMain(() -> {
                tap(3 * (KEY + 2 * MARGIN) + SPACER * 1.5f);
            });
            expectSettled(List.of());

            test.stage = "key press area: two fingers report their own keys";
            onMain(() -> {
                downTime = android.os.SystemClock.uptimeMillis();
                float ax = MARGIN + KEY / 2f;
                float bx = KEY + 3 * MARGIN + KEY / 2f;
                dispatch(single(MotionEvent.ACTION_DOWN, ax));
                dispatch(pair(MotionEvent.ACTION_POINTER_DOWN, ax, bx));
                dispatch(pair(MotionEvent.ACTION_POINTER_UP, ax, bx));
                dispatch(single(MotionEvent.ACTION_UP, ax));
            });
            test.waitForIdleSync();
            onMain(() -> {
                expectOrder("pressed:A", "pressed:B", "releasing:B", "releasing:A");
                expectOrder("click:B", "settled:releasing:B");
                expectOrder("click:A", "settled:releasing:A");
                expectEachTokenSettledOnce();
                log.clear();
            });

            test.stage = "key press area: a disabled key is not reported";
            onMain(() -> {
                c.setEnabled(false);
                tap(KEY * 2 + 5 * MARGIN + KEY / 2f);
                c.setEnabled(true);
            });
            expectSettled(List.of());

            test.stage = "key press area: a cancelled touch is not reported on release";
            onMain(() -> {
                downTime = android.os.SystemClock.uptimeMillis();
                dispatch(single(MotionEvent.ACTION_DOWN, MARGIN + KEY / 2f));
                dispatch(single(MotionEvent.ACTION_CANCEL, MARGIN + KEY / 2f));
            });
            expectSettled(List.of("pressed:A", "settled:pressed:A"));

            test.stage = "key press area: a glide takeover is not reported on release";
            onMain(() -> {
                downTime = android.os.SystemClock.uptimeMillis();
                dispatch(single(MotionEvent.ACTION_DOWN, MARGIN + KEY / 2f));
                glideTakes = true;
                dispatch(single(MotionEvent.ACTION_MOVE, MARGIN + KEY));
                dispatch(single(MotionEvent.ACTION_UP, KEY * 2f));
                glideTakes = false;
            });
            expectSettled(List.of("pressed:A", "settled:pressed:A"));
        } finally {
            onMain(activity::finish);
        }
    }

    private View build(Activity activity) throws Exception {
        Context context = test.getTargetContext();
        ClassLoader loader = context.getClassLoader();
        areaType = loader.loadClass("app.msime.android.KeyboardKeyArea");
        Constructor<?> constructor = areaType.getConstructor(Context.class, Predicate.class);
        Predicate<View> spaced = view -> view instanceof Button;
        area = constructor.newInstance(activity, spaced);
        LinearLayout row = new LinearLayout(activity);
        row.setOrientation(LinearLayout.HORIZONTAL);
        a = key(activity, "A");
        b = key(activity, "B");
        c = key(activity, "C");
        row.addView(a);
        row.addView(b);
        row.addView(c);
        Class<?> spacerType = loader.loadClass("app.msime.android.KeyboardSpacer");
        Constructor<?> spacer = spacerType.getConstructor(Context.class);
        View owned = (View) spacer.newInstance(activity);
        spacerType.getMethod("setOwner", View.class).invoke(owned, c);
        row.addView(owned, new LinearLayout.LayoutParams(SPACER, KEY + 2 * MARGIN));
        row.addView((View) spacer.newInstance(activity), new LinearLayout.LayoutParams(SPACER, KEY + 2 * MARGIN));
        ((LinearLayout) area).setOrientation(LinearLayout.VERTICAL);
        ((ViewGroup) area).addView(row, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        Class<?> listener = loader.loadClass("app.msime.android.KeyboardKeyArea$PressListener");
        Class<?> tracker = loader.loadClass("app.msime.android.KeyboardKeyArea$GlideTracker");
        areaType.getMethod("setPressListener", listener).invoke(area, proxy(loader, listener, this::onPress));
        areaType.getMethod("setGlideTracker", tracker).invoke(area, proxy(loader, tracker, (name, args) -> glideTakes));
        FrameLayout frame = new FrameLayout(activity);
        frame.addView((View) area, new FrameLayout.LayoutParams(3 * (KEY + 2 * MARGIN) + 2 * SPACER, KEY + 2 * MARGIN));
        return frame;
    }

    private Button key(Activity activity, String name) {
        Button key = new Button(activity);
        key.setText(name);
        key.setMinWidth(0);
        key.setMinimumWidth(0);
        key.setMinHeight(0);
        key.setMinimumHeight(0);
        key.setOnClickListener(ignored -> log.add("click:" + name));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(KEY, KEY);
        params.setMargins(MARGIN, MARGIN, MARGIN, MARGIN);
        key.setLayoutParams(params);
        return key;
    }

    /** `PressListener` 的三个回调：令牌就是「回调名:键名」，交还时原样记下。 */
    private Object onPress(String name, Object[] args) {
        if ("settled".equals(name)) {
            log.add("settled:" + args[0]);
            return null;
        }
        String token = name + ":" + label((View) args[0]);
        log.add(token);
        return token;
    }

    private String label(View view) {
        return view == a ? "A" : view == b ? "B" : view == c ? "C" : "?";
    }

    private void setYield(View yielding, View receiver, float width) throws Exception {
        areaType.getMethod("setYield", View.class, View.class, float.class).invoke(area, yielding, receiver, width);
    }

    private void tap(float x) {
        downTime = android.os.SystemClock.uptimeMillis();
        dispatch(single(MotionEvent.ACTION_DOWN, x));
        dispatch(single(MotionEvent.ACTION_UP, x));
    }

    private void dispatch(MotionEvent event) {
        ((View) area).dispatchTouchEvent(event);
        event.recycle();
    }

    private MotionEvent single(int action, float x) {
        MotionEvent event = MotionEvent.obtain(downTime, android.os.SystemClock.uptimeMillis(), action, x, MIDDLE_Y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        return event;
    }

    /** 两根手指（id 0 在 `first`，id 1 在 `second`），`action` 作用在第二根上。 */
    private MotionEvent pair(int action, float first, float second) {
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[2];
        MotionEvent.PointerCoords[] coordinates = new MotionEvent.PointerCoords[2];
        float[] xs = {first, second};
        for (int index = 0; index < 2; index++) {
            properties[index] = new MotionEvent.PointerProperties();
            properties[index].id = index;
            properties[index].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coordinates[index] = new MotionEvent.PointerCoords();
            coordinates[index].x = xs[index];
            coordinates[index].y = MIDDLE_Y;
            coordinates[index].pressure = 1f;
            coordinates[index].size = 1f;
        }
        return MotionEvent.obtain(downTime, android.os.SystemClock.uptimeMillis(),
            action | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT), 2, properties, coordinates,
            0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
    }

    /** 等主线程把这次分发里投递的点击和交还令牌都执行完，再核对整段记录。 */
    private void expectSettled(List<String> expected) throws Exception {
        test.waitForIdleSync();
        onMain(() -> {
            if (!log.equals(expected)) throw new AssertionError(test.stage + ": expected " + expected + " but got " + log);
            log.clear();
        });
    }

    private void expectOrder(String... entries) {
        int previous = -1;
        for (String entry : entries) {
            int index = log.indexOf(entry);
            if (index < 0 || index < previous) throw new AssertionError(test.stage + ": " + entry + " out of order in " + log);
            previous = index;
        }
    }

    private void expectEachTokenSettledOnce() {
        for (String entry : log) {
            if (!entry.startsWith("pressed:") && !entry.startsWith("releasing:")) continue;
            long settled = log.stream().filter(("settled:" + entry)::equals).count();
            if (settled != 1) throw new AssertionError(test.stage + ": " + entry + " settled " + settled + " times in " + log);
        }
    }

    private static Object proxy(ClassLoader loader, Class<?> type, Callback callback) {
        InvocationHandler handler = (Object self, Method method, Object[] args) -> {
            if (method.getDeclaringClass() == Object.class) {
                return switch (method.getName()) {
                    case "equals" -> self == args[0];
                    case "hashCode" -> System.identityHashCode(self);
                    default -> type.getSimpleName();
                };
            }
            return callback.call(method.getName(), args == null ? new Object[0] : args);
        };
        return Proxy.newProxyInstance(loader, new Class<?>[] {type}, handler);
    }

    @FunctionalInterface private interface Callback { Object call(String name, Object[] args); }

    private void onMain(Checked action) throws Exception {
        Throwable[] failure = {null};
        test.runOnMainSync(() -> { try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; } });
        if (failure[0] instanceof AssertionError error) throw error;
        if (failure[0] instanceof Exception error) throw error;
    }

    @FunctionalInterface private interface Checked { void run() throws Exception; }
}
