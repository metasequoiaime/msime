package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;

/** Shared Android containers and layout parameter factories. */
public final class LayoutPolicy {
    private LayoutPolicy() {}

    /** Create a vertical container for stacked keyboard content. */
    public static LinearLayout column(Context context) {
        return ViewPolicy.newColumn(context);
    }

    /** Create a horizontal container for inline keyboard content. */
    public static LinearLayout row(Context context) {
        return ViewPolicy.newRow(context);
    }

    /** Create linear layout parameters from density-independent dimensions. */
    public static LinearLayout.LayoutParams linearParams(Context context, float widthDp, float heightDp) {
        return new LinearLayout.LayoutParams(DimensionPolicy.pixels(context, widthDp),
            DimensionPolicy.pixels(context, heightDp));
    }

    /** Create linear layout parameters from already pixel-sized dimensions. */
    public static LinearLayout.LayoutParams linearParamsPx(int widthPixels, int heightPixels) {
        return new LinearLayout.LayoutParams(widthPixels, heightPixels);
    }

    /** Create full-width linear layout parameters with content-sized height. */
    public static LinearLayout.LayoutParams matchWidthWrapParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create linear layout parameters that fill both dimensions. */
    public static LinearLayout.LayoutParams matchParentParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create linear layout parameters that wrap both dimensions. */
    public static LinearLayout.LayoutParams wrapParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create content-height parameters whose width either wraps or fills the parent. */
    public static LinearLayout.LayoutParams wrapOrMatchWidthParams(boolean wrapWidth) {
        return new LinearLayout.LayoutParams(wrapWidth
            ? LinearLayout.LayoutParams.WRAP_CONTENT : LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create weighted linear layout parameters that fill the parent's height. */
    public static LinearLayout.LayoutParams weightedMatchParentParams(float weight) {
        return new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, weight);
    }

    /** Create weighted linear layout parameters with content-sized height. */
    public static LinearLayout.LayoutParams weightedWrapParams(float weight) {
        return new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, weight);
    }

    /** Create weighted linear layout parameters with an already pixel-sized height. */
    public static LinearLayout.LayoutParams weightedHeightPxParams(int heightPixels, float weight) {
        return new LinearLayout.LayoutParams(0, heightPixels, weight);
    }

    /** Create weighted linear layout parameters that fill width with a zero-height basis. */
    public static LinearLayout.LayoutParams weightedWidthParams(float weight) {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, weight);
    }

    /** Create weighted linear layout parameters with zero width and height bases. */
    public static LinearLayout.LayoutParams weightedZeroParams(float weight) {
        return new LinearLayout.LayoutParams(0, 0, weight);
    }

    /** Fill the cross axis while distributing the main axis by weight. */
    public static LinearLayout.LayoutParams weightedCrossAxisFillParams(boolean horizontal,
            float weight) {
        return horizontal ? weightedMatchParentParams(weight) : weightedWidthParams(weight);
    }

    /** Create full-width linear layout parameters with an already pixel-sized height. */
    public static LinearLayout.LayoutParams matchWidthHeightPx(int heightPixels) {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, heightPixels);
    }

    /** Create linear layout parameters with content-sized width and parent-sized height. */
    public static LinearLayout.LayoutParams wrapMatchParentParams() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create frame layout parameters that fill both parent dimensions. */
    public static FrameLayout.LayoutParams frameMatchParentParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT);
    }

    /** Create frame layout parameters that wrap both dimensions. */
    public static FrameLayout.LayoutParams frameWrapParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create frame layout parameters that fill width with an already pixel-sized height. */
    public static FrameLayout.LayoutParams frameMatchWidthHeightPx(int heightPixels) {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, heightPixels);
    }

    /** Create frame layout parameters for a pixel-sized square. */
    public static FrameLayout.LayoutParams squareFrameParamsPx(int size) {
        return new FrameLayout.LayoutParams(size, size);
    }

    /** Create frame layout parameters from already pixel-sized dimensions. */
    public static FrameLayout.LayoutParams frameParamsPx(int widthPixels, int heightPixels) {
        return new FrameLayout.LayoutParams(widthPixels, heightPixels);
    }

    /** Create frame layout parameters from pixel dimensions and explicit gravity. */
    public static FrameLayout.LayoutParams frameParamsPx(int widthPixels, int heightPixels,
            int gravity) {
        return new FrameLayout.LayoutParams(widthPixels, heightPixels, gravity);
    }

    /** Create frame layout parameters that fill width with content-sized height. */
    public static FrameLayout.LayoutParams frameMatchWidthWrapParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.WRAP_CONTENT);
    }

    /** Create frame layout parameters that fill width with content-sized height and gravity. */
    public static FrameLayout.LayoutParams frameMatchWidthWrapParams(int gravity) {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.WRAP_CONTENT, gravity);
    }

    /** Create scroll-view child parameters that fill both dimensions. */
    public static ScrollView.LayoutParams scrollMatchParentParams() {
        return new ScrollView.LayoutParams(ScrollView.LayoutParams.MATCH_PARENT,
            ScrollView.LayoutParams.MATCH_PARENT);
    }

    /** Create scroll-view child parameters that fill width and wrap content height. */
    public static ScrollView.LayoutParams scrollMatchWidthWrapParams() {
        return new ScrollView.LayoutParams(ScrollView.LayoutParams.MATCH_PARENT,
            ScrollView.LayoutParams.WRAP_CONTENT);
    }

    /** Apply the same start and end margin to a layout parameter. */
    public static void setHorizontalMargins(ViewGroup.MarginLayoutParams params, int margin) {
        params.setMarginStart(margin);
        params.setMarginEnd(margin);
    }

    /** Apply symmetric horizontal and vertical padding expressed in dp. */
    public static void setSymmetricPaddingDp(View view, Context context, float horizontalDp,
            float verticalDp) {
        int horizontal = DimensionPolicy.pixels(context, horizontalDp);
        int vertical = DimensionPolicy.pixels(context, verticalDp);
        ViewPolicy.setSymmetricPadding(view, horizontal, vertical);
    }

    /** Apply equal horizontal dp padding with no vertical padding. */
    public static void setHorizontalPaddingDp(View view, Context context, float horizontalDp) {
        ViewPolicy.setHorizontalPadding(view, DimensionPolicy.pixels(context, horizontalDp));
    }

    /** Apply four-sided padding expressed in density-independent pixels. */
    public static void setPaddingDp(View view, Context context, float leftDp, float topDp,
            float rightDp, float bottomDp) {
        ViewPolicy.setPadding(view, DimensionPolicy.pixels(context, leftDp),
            DimensionPolicy.pixels(context, topDp), DimensionPolicy.pixels(context, rightDp),
            DimensionPolicy.pixels(context, bottomDp));
    }

    /** Create weighted linear layout parameters with a fixed height in dp. */
    public static LinearLayout.LayoutParams weightedHeightParams(Context context, float heightDp,
            float weight) {
        return new LinearLayout.LayoutParams(0, DimensionPolicy.pixels(context, heightDp), weight);
    }

}
