package app.msime.android;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Paint;
import android.graphics.Typeface;
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

    /** Bind an optional action, clearing the listener when no action is available. */
    public static void bindOptionalClick(View view, Runnable action) {
        view.setOnClickListener(action == null ? null : ignored -> action.run());
    }

    /** Set whether a view accepts input without changing its visibility or focus policy. */
    public static void setEnabled(View view, boolean enabled) {
        view.setEnabled(enabled);
    }

    /** Apply enabled state and a caller-selected inactive opacity. */
    public static void setEnabledWithAlpha(View view, boolean enabled, float inactiveAlpha) {
        setEnabled(view, enabled);
        setActiveAlpha(view, enabled, inactiveAlpha);
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

    /** Set an explicit child gravity on a linear layout. */
    public static void setGravity(LinearLayout view, int gravity) {
        view.setGravity(gravity);
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

    /** Apply a default-family typeface with the supplied numeric weight. */
    public static void setTypefaceWeight(TextView view, int weight) {
        view.setTypeface(Typeface.create(Typeface.DEFAULT, weight, false));
    }

    /** Set a text view's size in scalable pixels. */
    public static void setTextSizeSp(TextView view, float sizeSp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp);
    }

    /** Set a paint's text size in scalable pixels using the supplied display context. */
    public static void setTextSizeSp(Paint paint, Context context, float sizeSp) {
        paint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sizeSp,
            context.getResources().getDisplayMetrics()));
    }

    /** Set a text view's size in density-independent pixels. */
    public static void setTextSizeDp(TextView view, float sizeDp) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_DIP, sizeDp);
    }

    /** Set a text view's solid foreground color. */
    public static void setTextColor(TextView view, int color) {
        view.setTextColor(color);
    }

    /** Set a text view's state-aware foreground colors. */
    public static void setTextColor(TextView view, ColorStateList colors) {
        view.setTextColor(colors);
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

    /** Set a view background while preserving its other visual state. */
    public static void setBackground(View view, android.graphics.drawable.Drawable background) {
        view.setBackground(background);
    }

    /** Apply a solid background color while preserving the view's other visual state. */
    public static void setBackgroundColor(View view, int color) {
        view.setBackgroundColor(color);
    }

    /** Remove a view's default background drawable. */
    public static void clearBackground(View view) {
        setBackground(view, null);
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

    /** Set whether a text view is constrained to one line without changing truncation policy. */
    public static void setSingleLine(TextView view, boolean singleLine) {
        view.setSingleLine(singleLine);
    }

    /** Set additional line spacing and multiplier on a text view. */
    public static void setLineSpacing(TextView view, float add, float multiplier) {
        view.setLineSpacing(add, multiplier);
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

    /** Set whether a view responds to taps without changing its focus policy. */
    public static void setClickable(View view, boolean clickable) {
        view.setClickable(clickable);
    }

    /** Set whether a view accepts keyboard focus without changing its click policy. */
    public static void setFocusable(View view, boolean focusable) {
        view.setFocusable(focusable);
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

    /** Show a view only when the supplied text is non-null and non-empty. */
    public static void setVisibilityForText(View view, CharSequence text) {
        setVisible(view, text != null && text.length() != 0);
    }

    /** Hide a view while preserving its layout space. */
    public static void setInvisible(View view) {
        view.setVisibility(View.INVISIBLE);
    }
}
