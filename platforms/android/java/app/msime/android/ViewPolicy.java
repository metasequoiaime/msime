package app.msime.android;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.util.TypedValue;
import android.text.TextUtils;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

/** Shared view configuration for host controls whose widget defaults need resetting. */
public final class ViewPolicy {
    private ViewPolicy() {}

    /** Remove both legacy and platform minimum-height constraints from a view. */
    public static void clearMinimumHeight(View view) {
        view.setMinimumHeight(0);
    }

    /** Remove both minimum-height constraints from a text widget. */
    public static void clearMinimumHeight(TextView view) {
        view.setMinHeight(0);
        view.setMinimumHeight(0);
    }

    /** Apply a minimum height to a generic view. */
    public static void setMinimumHeight(View view, int height) {
        view.setMinimumHeight(height);
    }

    /** Remove both minimum width and height constraints from a generic view. */
    public static void clearMinimumSize(View view) {
        clearMinimumWidth(view);
        clearMinimumHeight(view);
    }

    /** Remove both minimum width and height constraints from a text widget. */
    public static void clearMinimumSize(TextView view) {
        clearMinimumWidth(view);
        clearMinimumHeight(view);
    }

    /** Apply a minimum height to both text-widget constraints. */
    public static void setMinimumHeight(TextView view, int height) {
        view.setMinHeight(height);
        view.setMinimumHeight(height);
    }

    /** Remove both legacy and platform minimum-width constraints from a view. */
    public static void clearMinimumWidth(View view) {
        view.setMinimumWidth(0);
    }

    /** Remove both minimum-width constraints from a text widget. */
    public static void clearMinimumWidth(TextView view) {
        view.setMinWidth(0);
        view.setMinimumWidth(0);
    }

    /** Apply a minimum width to a generic view. */
    public static void setMinimumWidth(View view, int width) {
        view.setMinimumWidth(width);
    }

    /** Apply a minimum width to both text-widget constraints. */
    public static void setMinimumWidth(TextView view, int width) {
        view.setMinWidth(width);
        view.setMinimumWidth(width);
    }

    /** Apply equal horizontal and vertical padding to a view. */
    public static void setSymmetricPadding(View view, int horizontal, int vertical) {
        view.setPadding(horizontal, vertical, horizontal, vertical);
    }

    /** Apply equal horizontal pixel padding while leaving vertical padding unset. */
    public static void setHorizontalPadding(View view, int horizontal) {
        view.setPadding(horizontal, 0, horizontal, 0);
    }

    /** Clear all view padding. */
    public static void clearPadding(View view) {
        view.setPadding(0, 0, 0, 0);
    }

    /** Clear vertical padding while preserving horizontal padding. */
    public static void clearVerticalPadding(View view) {
        view.setPadding(view.getPaddingLeft(), 0, view.getPaddingRight(), 0);
    }

    /** Set equal horizontal padding while preserving the current vertical padding. */
    public static void setHorizontalPaddingPreservingVertical(View view, int horizontal) {
        view.setPadding(horizontal, view.getPaddingTop(), horizontal, view.getPaddingBottom());
    }

    /** Apply explicit pixel padding on all four sides. */
    public static void setPadding(View view, int left, int top, int right, int bottom) {
        view.setPadding(left, top, right, bottom);
    }

    /** Remove the default background, padding, and minimum size from a view. */
    public static void clearChrome(View view) {
        clearBackground(view);
        clearPadding(view);
        clearMinimumSize(view);
    }

    /** Keep a button label in its authored casing instead of applying the platform default. */
    public static void setAllCapsFalse(Button button) {
        button.setAllCaps(false);
    }

    /** Create a text view with its initial content assigned. */
    public static TextView newTextView(Context context, CharSequence text) {
        TextView view = new TextView(context);
        view.setText(text);
        return view;
    }

    /** Create a keyboard press button with authored casing preserved. */
    public static KeyboardPressButton newPressButton(android.content.Context context) {
        KeyboardPressButton button = new KeyboardPressButton(context);
        setAllCapsFalse(button);
        return button;
    }

    /** Bind a caller-supplied action to a view without changing any other interaction policy. */
    public static void bindClick(View view, Runnable action) {
        view.setOnClickListener(ignored -> action.run());
    }

    /** Set whether a view accepts input without changing its visibility or focus policy. */
    public static void setEnabled(View view, boolean enabled) {
        view.setEnabled(enabled);
    }

    /** Announce changing view content to accessibility services without interrupting the user. */
    public static void setPoliteLiveRegion(View view) {
        view.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
    }

    /** Center a view's content on both axes. */
    public static void setCentered(View view) {
        if (view instanceof TextView text) {
            text.setGravity(Gravity.CENTER);
        } else if (view instanceof LinearLayout layout) {
            layout.setGravity(Gravity.CENTER);
        } else {
            throw new IllegalArgumentException("Centered policy requires a text or linear-layout view");
        }
    }

    /** Center a view's content along the vertical axis. */
    public static void setCenteredVertically(View view) {
        if (view instanceof TextView text) {
            text.setGravity(Gravity.CENTER_VERTICAL);
        } else if (view instanceof LinearLayout layout) {
            layout.setGravity(Gravity.CENTER_VERTICAL);
        } else {
            throw new IllegalArgumentException("Vertical centering requires a text or linear-layout view");
        }
    }

    /** Center a view's content along the horizontal axis. */
    public static void setCenteredHorizontally(LinearLayout view) {
        view.setGravity(Gravity.CENTER_HORIZONTAL);
    }

    /** Center a text view's content along the horizontal axis. */
    public static void setCenteredHorizontally(TextView view) {
        view.setGravity(Gravity.CENTER_HORIZONTAL);
    }

    /** Align a view's content to the start edge and center it vertically. */
    public static void setStartCenteredVertically(LinearLayout view) {
        view.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
    }

    /** Align a text view's content to the start edge and center it vertically. */
    public static void setStartCenteredVertically(TextView view) {
        view.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
    }

    /** Align a view's content to the end edge and center it vertically. */
    public static void setEndCenteredVertically(TextView view) {
        view.setGravity(Gravity.END | Gravity.CENTER_VERTICAL);
    }

    /** Align a multi-line text field to the start edge and top. */
    public static void setTopStart(TextView view) {
        view.setGravity(Gravity.TOP | Gravity.START);
    }

    /** Apply a typeface style while preserving the text view's current family. */
    public static void setTypefaceStyle(TextView view, int style) {
        view.setTypeface(view.getTypeface(), style);
    }

    /** Set a text view's size in scalable pixels. */
    public static void setTextSizeSp(TextView view, float sizeSp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp);
    }

    /** Set a text view's solid foreground color. */
    public static void setTextColor(TextView view, int color) {
        view.setTextColor(color);
    }

    /** Set a text label and its scalable size. */
    public static void setTextSizeLabel(TextView view, CharSequence text, float sizeSp) {
        view.setText(text);
        setTextSizeSp(view, sizeSp);
    }

    /** Create a text label with its scalable size assigned. */
    public static TextView textLabel(Context context, CharSequence text, float sizeSp) {
        TextView view = newTextView(context, text);
        setTextSizeSp(view, sizeSp);
        return view;
    }

    /** Set a text view's scalable size and center its content. */
    public static void setCenteredTextSizeSp(TextView view, float sizeSp) {
        setTextSizeSp(view, sizeSp);
        setCentered(view);
    }

    /** Set a keyboard-scaled size and center the text view's content. */
    public static void setCenteredKeyTextSizeSp(TextView view, float sizeSp) {
        KeyboardGeometry.setKeyTextSize(view, sizeSp);
        setCentered(view);
    }

    /** Set a centered text label and its scalable size. */
    public static void setCenteredText(TextView view, CharSequence text, float sizeSp) {
        view.setText(text);
        setCenteredTextSizeSp(view, sizeSp);
    }

    /** Create a centered text label with its scalable size assigned. */
    public static TextView centeredText(Context context, CharSequence text, float sizeSp) {
        TextView view = newTextView(context, text);
        setCenteredTextSizeSp(view, sizeSp);
        return view;
    }

    /** Set a text view's scalable size and align its content to the start edge vertically centered. */
    public static void setStartCenteredTextSizeSp(TextView view, float sizeSp) {
        setTextSizeSp(view, sizeSp);
        setStartCenteredVertically(view);
    }

    /** Set a keyboard-scaled size and align its content to the start edge vertically centered. */
    public static void setStartCenteredKeyTextSizeSp(TextView view, float sizeSp) {
        KeyboardGeometry.setKeyTextSize(view, sizeSp);
        setStartCenteredVertically(view);
    }

    /** Remove a view's default background drawable. */
    public static void clearBackground(View view) {
        view.setBackground(null);
    }

    /** Limit a text view to a fixed number of lines and truncate at the end. */
    public static void setMaxLinesEllipsized(TextView view, int maxLines) {
        view.setMaxLines(maxLines);
        view.setEllipsize(TextUtils.TruncateAt.END);
    }

    /** Keep a text view on one line and truncate overflowing text at the end. */
    public static void setSingleLineEllipsized(TextView view) {
        view.setSingleLine(true);
        view.setEllipsize(TextUtils.TruncateAt.END);
    }

    /** Keep a text view on one line without changing its truncation policy. */
    public static void setSingleLine(TextView view) {
        view.setSingleLine(true);
    }

    /** Limit a text view to a maximum number of lines without changing truncation policy. */
    public static void setMaxLines(TextView view, int maxLines) {
        view.setMaxLines(maxLines);
    }

    /** Require a text view to occupy at least the requested number of lines. */
    public static void setMinLines(TextView view, int minLines) {
        view.setMinLines(minLines);
    }

    /** Force a text view to occupy exactly the requested number of lines. */
    public static void setFixedLines(TextView view, int lines) {
        view.setMinLines(lines);
        view.setMaxLines(lines);
    }

    /** Configure uniform automatic text sizing with scalable-pixel bounds. */
    public static void setAutoSizeSp(TextView view, int minSp, int maxSp, int stepSp) {
        view.setAutoSizeTextTypeUniformWithConfiguration(minSp, maxSp, stepSp,
            TypedValue.COMPLEX_UNIT_SP);
    }

    /** Remove Android's extra font top and bottom padding from a text view. */
    public static void clearFontPadding(TextView view) {
        view.setIncludeFontPadding(false);
    }

    /** Remove font padding and vertical view padding while preserving horizontal padding. */
    public static void clearFontAndVerticalPadding(TextView view) {
        clearFontPadding(view);
        clearVerticalPadding(view);
    }

    /** Make a view passive for touch and focus navigation. */
    public static void setNonInteractive(View view) {
        view.setClickable(false);
        view.setFocusable(false);
    }

    /** Expose a view's selected state to drawable and accessibility state lists. */
    public static void setSelected(View view, boolean selected) {
        view.setSelected(selected);
    }

    /** Set whether a view participates in touch and focus navigation. */
    public static void setInteractive(View view, boolean interactive) {
        view.setClickable(interactive);
        view.setFocusable(interactive);
    }

    /** Exclude a decorative view from the accessibility tree. */
    public static void hideFromAccessibility(View view) {
        view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    /** Remove the platform state-list animator from a view. */
    public static void clearStateListAnimator(View view) {
        view.setStateListAnimator(null);
    }

    /** Remove any platform elevation from a view. */
    public static void clearElevation(View view) {
        view.setElevation(0);
    }

    /** Apply full opacity to an active view and a caller-selected opacity otherwise. */
    public static void setActiveAlpha(View view, boolean active, float inactiveAlpha) {
        view.setAlpha(active ? 1f : inactiveAlpha);
    }

    /** Apply minimum width and height constraints to a view. */
    public static void setMinimumSize(View view, int width, int height) {
        view.setMinimumWidth(width);
        view.setMinimumHeight(height);
    }

    /** Show a view in layout and rendering. */
    public static void show(View view) {
        view.setVisibility(View.VISIBLE);
    }

    /** Hide a view from layout and rendering. */
    public static void hide(View view) {
        view.setVisibility(View.GONE);
    }

    /** Toggle between visible and gone layout participation. */
    public static void setVisible(View view, boolean visible) {
        view.setVisibility(visible ? View.VISIBLE : View.GONE);
    }

    /** Hide a view while preserving its layout space. */
    public static void setInvisible(View view) {
        view.setVisibility(View.INVISIBLE);
    }
}
