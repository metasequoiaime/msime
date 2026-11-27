package app.msime.android;

import android.view.ViewGroup;
import android.widget.FrameLayout;

import java.util.EnumMap;

/**
 * 输入法窗口里画在面板之上的层（{@link OverlayLayer} 里 {@link OverlayLayer.Host#IME_WINDOW} 的那些）。每层是 {@code keyboardRoot} 里排在面板（{@code keyboardSurface}）之后的一个铺满的 FrameLayout，按 {@link OverlayLayer} 的顺序插入，后面的层盖住前面的层；层和按键在同一个窗口里，同一帧画出。
 *
 * <p>根视图覆盖面板和它上方的透明高度（{@link PanelGeometry#HEADROOM_DP}），所以层里的东西可以伸出面板上沿而不被截掉。层本身是空的、不可点的 FrameLayout，不拦截触摸，也不进无障碍树。层在第一次用到时才加，换了新的输入视图后重新加。
 */
final class ImeOverlayHost {
    /** 浮动键盘和平板居中时面板（{@code keyboardSurface}）的投影高度（dp）。 */
    static final int PANEL_ELEVATION_DP = 10;
    /**
     * 层的高度（dp）：比面板的投影高。同一父视图里 Z 高的子视图后画，面板有投影时会盖住排在它后面、Z 为 0 的层；层都用同一个 Z，彼此之间仍按子视图顺序。层本身没有背景，不投影。
     */
    private static final int LAYER_ELEVATION_DP = PANEL_ELEVATION_DP + 1;
    private final MSIMEInputService s;
    private final EnumMap<OverlayLayer, FrameLayout> layers = new EnumMap<>(OverlayLayer.class);
    private ViewGroup root;

    ImeOverlayHost(MSIMEInputService s) {
        this.s = s;
    }

    /** {@code onCreateInputView} 建好新的根视图和面板后调用：旧根视图上的层作废。 */
    void attach(ViewGroup keyboardRoot) {
        layers.clear();
        root = keyboardRoot;
    }

    /**
     * 这一层的视图，没有就建好插到根视图里：面板之后，排在它前面且已经加上的层之后。
     *
     * @return 根视图或面板还没建时为 null
     * @throws IllegalArgumentException 这一层不在输入法窗口里，或者就是面板
     */
    FrameLayout layer(OverlayLayer layer) {
        if (layer.host != OverlayLayer.Host.IME_WINDOW || layer == OverlayLayer.PANEL)
            throw new IllegalArgumentException(layer + " is not a layer above the panel in the IME window");
        FrameLayout view = layers.get(layer);
        if (view != null && view.getParent() == root) return view;
        if (root == null || s.keyboardSurface == null || s.keyboardSurface.getParent() != root) return null;
        int index = root.indexOfChild(s.keyboardSurface) + 1;
        for (OverlayLayer lower : layers.keySet())
            if (lower.compareTo(layer) < 0 && layers.get(lower).getParent() == root) index++;
        view = new FrameLayout(s);
        ViewPolicy.setNonInteractive(view);
        ViewPolicy.hideFromAccessibility(view);
        ViewPolicy.setElevation(view, s.pixels(LAYER_ELEVATION_DP));
        root.addView(view, index, LayoutPolicy.frameMatchParentParams());
        layers.put(layer, view);
        return view;
    }
}
