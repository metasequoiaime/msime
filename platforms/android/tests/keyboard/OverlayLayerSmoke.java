import app.msime.android.OverlayLayer;

public final class OverlayLayerSmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        OverlayLayer[] layers = OverlayLayer.values();
        check(layers[0] == OverlayLayer.PANEL, "the panel is the bottom layer");
        check(layers.length == 5, "five layers: panel, reading strip, key preview, long-press, magnifier");
        check(OverlayLayer.READING_STRIP.compareTo(OverlayLayer.KEY_PREVIEW) < 0,
            "the key preview draws over the reading strip");
        check(OverlayLayer.KEY_PREVIEW.compareTo(OverlayLayer.LONG_PRESS) < 0,
            "long-press options draw over the key preview");
        check(OverlayLayer.ordered(), "no layer is hosted in a lower window than a layer below it");
        check(OverlayLayer.KEY_PREVIEW.host == OverlayLayer.Host.IME_WINDOW,
            "the key preview shares the keys' window, so both draw in the same frame");
        check(OverlayLayer.LONG_PRESS.host == OverlayLayer.Host.POPUP_WINDOW,
            "long-press options keep their own popup window");
    }
}
