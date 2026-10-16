package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.ContextThemeWrapper;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.ScrollView;
import java.lang.reflect.Proxy;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

/** 检查安装产品的全宽网格、底栏及首次测量，并用触摸分发验证切类不会误上屏。 */
public final class SymbolPanelLayoutDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        runOnMainSync(() -> {
            try { checkLayout(); }
            catch (ReflectiveOperationException | AssertionError | RuntimeException error) { failure.set(error); }
        });
        if (failure.get() != null) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: first symbol layout ("
                + failure.get().getClass().getSimpleName() + ")"
                + (failure.get() instanceof AssertionError ? " " + failure.get().getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        } else {
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: full-width symbol grid, bottom categories, continuous insertion and directional gestures\n");
            finish(Activity.RESULT_OK, result);
        }
    }

    private void checkLayout() throws ReflectiveOperationException {
        Context context = new ContextThemeWrapper(getTargetContext(), android.R.style.Theme_Material_Light);
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.SymbolPanelView");
        Class<?> factoryType = loader.loadClass("app.msime.android.SymbolPanelView$ButtonFactory");
        Class<?> listenerType = loader.loadClass("app.msime.android.SymbolPanelView$Listener");
        int[] insertions = {0};
        int[] closes = {0};
        Object factory = Proxy.newProxyInstance(loader, new Class<?>[] {factoryType}, (proxy, method, args) -> {
            if (!method.getName().equals("create")) throw new AssertionError("Unexpected button factory operation");
            Button button = new Button(context);
            button.setText((String) args[0]);
            button.setOnClickListener(ignored -> ((Runnable) args[2]).run());
            return button;
        });
        Object listener = Proxy.newProxyInstance(loader, new Class<?>[] {listenerType}, (proxy, method, args) -> {
            if (method.getName().equals("insert")) insertions[0]++;
            if (method.getName().equals("close")) closes[0]++;
            return null;
        });
        View panel = (View) type.getConstructor(Context.class, factoryType, listenerType)
            .newInstance(context, factory, listener);
        type.getMethod("resetForPresentation", List.class).invoke(panel, List.of("，", "@protonmail.com"));
        for (int width : new int[] {1000, 720, 1000}) {
            panel.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(700, View.MeasureSpec.EXACTLY));
            panel.layout(0, 0, width, 700);
            assertGeometry((ViewGroup) panel);
            for (String symbol : List.of("，", "@protonmail.com")) {
                Button button = findButton(panel, symbol);
                if (button == null || button.getWidth() <= 0 || button.getHeight() <= 0)
                    throw new AssertionError("Recent symbol has no bounds after layout at width " + width);
                if (!button.performClick()) throw new AssertionError("Recent symbol cannot be clicked");
            }
        }
        if (insertions[0] != 6) throw new AssertionError("Recent symbol callbacks were lost");
        if (closes[0] != 0) throw new AssertionError("Symbol insertion still closes the panel");
        findButton(panel, "中文").performClick();
        layout(panel, 1000);
        ScrollView scroll = (ScrollView) findDescription(panel, "符号网格；长条目加宽");
        swipe(scroll, 0.8f, 0.3f, 0.2f, 0.3f, MotionEvent.ACTION_UP);
        if (!findButton(panel, "英文").isSelected()) throw new AssertionError("Left swipe did not change category: "
            + scroll.getWidth() + "x" + scroll.getHeight() + ", child " + scroll.getChildAt(0).getWidth()
            + "x" + scroll.getChildAt(0).getHeight());
        layout(panel, 1000);
        swipe(scroll, 0.2f, 0.3f, 0.8f, 0.3f, MotionEvent.ACTION_CANCEL);
        if (!findButton(panel, "英文").isSelected()) throw new AssertionError("Cancelled swipe changed category");
        swipe(scroll, 0.2f, 0.3f, 0.8f, 0.3f, MotionEvent.ACTION_UP);
        if (!findButton(panel, "中文").isSelected()) throw new AssertionError("Right swipe did not change category");
        layout(panel, 1000);
        swipe(scroll, 0.5f, 0.8f, 0.5f, 0.2f, MotionEvent.ACTION_UP);
        if (!findButton(panel, "中文").isSelected() || scroll.getScrollY() <= 0)
            throw new AssertionError("Vertical swipe changed category or did not scroll");
        if (insertions[0] != 6) throw new AssertionError("Swiping inserted a symbol");
    }

    private static void layout(View panel, int width) {
        panel.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(700, View.MeasureSpec.EXACTLY));
        panel.layout(0, 0, width, 700);
    }

    private static void assertGeometry(ViewGroup panel) {
        View grid = findDescription(panel, "符号网格；长条目加宽");
        Rect gridBounds = bounds(panel, grid);
        Rect category = bounds(panel, findButton(panel, "常用"));
        Rect delete = bounds(panel, findButton(panel, "⌫"));
        Rect back = bounds(panel, findButton(panel, "‹"));
        if (gridBounds.width() < panel.getWidth() - panel.getPaddingLeft() - panel.getPaddingRight() - 1)
            throw new AssertionError("Symbol grid is not full width");
        if (category.top < gridBounds.bottom || delete.top < gridBounds.bottom
                || category.centerY() != delete.centerY() || back.bottom > gridBounds.top)
            throw new AssertionError("Categories and delete are not below the grid");
        if (findButton(panel, "锁定") != null || findButton(panel, "返回") != null)
            throw new AssertionError("Removed footer controls are still present");
    }

    private static Rect bounds(ViewGroup panel, View child) {
        if (child == null) throw new AssertionError("Expected symbol panel control is missing");
        Rect bounds = new Rect(0, 0, child.getWidth(), child.getHeight());
        panel.offsetDescendantRectToMyCoords(child, bounds);
        return bounds;
    }

    private static View findDescription(View view, String description) {
        if (view.getContentDescription() != null && description.contentEquals(view.getContentDescription())) return view;
        if (view instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++) {
                View result = findDescription(group.getChildAt(index), description);
                if (result != null) return result;
            }
        }
        return null;
    }

    private static void swipe(View view, float x0, float y0, float x1, float y1, int ending) {
        long start = SystemClock.uptimeMillis();
        for (int index = 0; index <= 12; index++) {
            int action = index == 0 ? MotionEvent.ACTION_DOWN : index == 12 ? ending : MotionEvent.ACTION_MOVE;
            float fraction = index / 12f;
            MotionEvent event = MotionEvent.obtain(start, start + index * 20, action,
                (x0 + (x1 - x0) * fraction) * view.getWidth(),
                (y0 + (y1 - y0) * fraction) * view.getHeight(), 0);
            event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            try { view.dispatchTouchEvent(event); }
            finally { event.recycle(); }
        }
    }

    private static Button findButton(View view, String text) {
        if (view instanceof Button button && text.contentEquals(button.getText())) return button;
        if (view instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++) {
                Button result = findButton(group.getChildAt(index), text);
                if (result != null) return result;
            }
        }
        return null;
    }
}
