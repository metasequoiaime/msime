package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import android.view.ContextThemeWrapper;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import java.lang.reflect.Proxy;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

/** 对安装产品做首次测量，保证重建后的符号键在当前布局就有可见边界。 */
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
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: first symbol layout and width changes keep recent cells measured and clickable\n");
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
        Object factory = Proxy.newProxyInstance(loader, new Class<?>[] {factoryType}, (proxy, method, args) -> {
            if (!method.getName().equals("create")) throw new AssertionError("Unexpected button factory operation");
            Button button = new Button(context);
            button.setText((String) args[0]);
            button.setOnClickListener(ignored -> ((Runnable) args[2]).run());
            return button;
        });
        Object listener = Proxy.newProxyInstance(loader, new Class<?>[] {listenerType}, (proxy, method, args) -> {
            if (method.getName().equals("insert")) insertions[0]++;
            return null;
        });
        View panel = (View) type.getConstructor(Context.class, factoryType, listenerType)
            .newInstance(context, factory, listener);
        type.getMethod("resetForPresentation", List.class).invoke(panel, List.of("，", "@protonmail.com"));
        for (int width : new int[] {1000, 720, 1000}) {
            panel.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                View.MeasureSpec.makeMeasureSpec(700, View.MeasureSpec.EXACTLY));
            panel.layout(0, 0, width, 700);
            for (String symbol : List.of("，", "@protonmail.com")) {
                Button button = findButton(panel, symbol);
                if (button == null || button.getWidth() <= 0 || button.getHeight() <= 0)
                    throw new AssertionError("Recent symbol has no bounds after layout at width " + width);
                if (!button.performClick()) throw new AssertionError("Recent symbol cannot be clicked");
            }
        }
        if (insertions[0] != 6) throw new AssertionError("Recent symbol callbacks were lost");
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
