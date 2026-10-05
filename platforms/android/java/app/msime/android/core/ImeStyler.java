package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.InsetDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.TextView;

/** 键盘的着色与几何：按键样式、皮肤套用、键距行距与键盘高度；从 MSIMEInputService 原样搬出。 */
final class ImeStyler {
    private final MSIMEInputService s;

    ImeStyler(MSIMEInputService s) {
        this.s = s;
    }

    void applyKeyboardGeometry(View node) {
        if (s.followsKeySpacing(node)
                && node.getLayoutParams() instanceof android.view.ViewGroup.MarginLayoutParams) {
            android.view.ViewGroup.MarginLayoutParams params =
                (android.view.ViewGroup.MarginLayoutParams) node.getLayoutParams();
            int horizontal = s.halfSpacingPixels(s.touchKeySpacingTenths);
            int vertical = s.halfSpacingPixels(s.touchRowSpacingTenths);
            params.setMargins(horizontal, vertical, horizontal, vertical);
            node.setLayoutParams(params);
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applyKeyboardGeometry(group.getChildAt(index));
        }
    }

    void applyKeyboardGeometry() {
        s.imePanels.applyReplyGeometry();
        if (s.keyRows == null) return;
        applyKeyboardGeometry(s.keyRows);
        applyKeyboardHeight(s.keyRows);
        // The action row is a sibling of the key rows rather than one of them -- its height is fixed
        // so the height setting cannot squeeze 换行 -- but its caps take the same spacing.
        if (s.actionRow != null) {
            applyKeyboardGeometry(s.actionRow);
            s.actionRow.requestLayout();
        }
        s.keyRows.requestLayout();
        if (s.keyboardRoot != null) {
            s.keyboardRoot.requestLayout();
            s.keyboardRoot.getRootView().requestLayout();
        }
    }

    void applyKeyboardHeight(View node) {
        Object tag = node.getTag();
        if (tag instanceof MSIMEInputService.KeyboardHeightRole) {
            MSIMEInputService.KeyboardHeightRole role = (MSIMEInputService.KeyboardHeightRole) tag;
            int height = s.pixels(KeyboardGeometry.adjustedRowHeight(role.baseHeight,
                s.touchKeyboardHeightAdjustment, role.rowCount, role.rowIndex));
            if (role.includesRowSpacing)
                height += s.halfSpacingPixels(s.touchRowSpacingTenths) * 2;
            if (node.getLayoutParams() != null) {
                android.view.ViewGroup.LayoutParams params = node.getLayoutParams();
                params.height = height;
                node.setLayoutParams(params);
            }
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applyKeyboardHeight(group.getChildAt(index));
        }
    }

    void adjustFixedHeight(View view, int baseHeight) {
        view.setTag(new MSIMEInputService.KeyboardHeightRole(baseHeight, 1, 0, false));
    }

    void styleButton(Button button, boolean action) { styleButton(button, action, s.skin); }

    void styleButton(Button button, boolean action, KeyboardSkin target) {
        styleButton(button, action ? KeyboardKeyRole.ACCENT : KeyboardKeyRole.KEY, target);
    }

    /** `target` is the surface's own skin: the emoji and handwriting panels carry their own theme. */
    void styleButton(Button button, KeyboardKeyRole role, KeyboardSkin target) {
        boolean selected = button.isSelected();
        // 选中的控件一律换成实心强调色，大小写键和简繁开关就是这样表示「开着」的。确认键和功能面板磁贴自己画开启状态，保留原角色；工具栏图标按钮（如打开回复面板时的「回复」）也不铺实心块，而是在图标后垫一块柔和的强调色底，和磁贴的开启状态是同一种表达。
        boolean toolbarGlyph = button instanceof KeyboardShortcutButton;
        KeyboardKeyRole face = selected && !toolbarGlyph && role != KeyboardKeyRole.RETURN
            && role != KeyboardKeyRole.TILE ? KeyboardKeyRole.ACCENT : role;
        if (face == KeyboardKeyRole.PILL) {
            // The pill is a label on the strip rather than a key, so it keeps a plain rounded face even over a designed skin, inset so the 44dp target stays.
            GradientDrawable pill = new GradientDrawable();
            pill.setColor(Color.parseColor(target.keyBackground()));
            pill.setCornerRadius(s.pixels(14));
            button.setBackground(new InsetDrawable(pill,
                s.pixels(2), s.pixels(8), s.pixels(2), s.pixels(8)));
            button.setTextColor(Color.parseColor(target.keyForeground()));
            button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            button.setElevation(0);
            return;
        }
        if (!face.drawsCap()) {
            button.setBackground(null);
            String label = face.usesAccentLabel() ? target.accent() : target.keyForeground();
            if (button instanceof KeyboardShortcutButton shortcut) {
                shortcut.setActiveFill(Color.parseColor(target.accentSoft()));
                if (selected) label = target.accentText();
            }
            button.setTextColor(Color.parseColor(label));
            button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            button.setElevation(0);
            return;
        }
        boolean action = face == KeyboardKeyRole.ACCENT;
        boolean confirm = face == KeyboardKeyRole.RETURN;
        boolean tile = face == KeyboardKeyRole.TILE;
        String background = confirm ? target.returnBackground()
            : tile ? (selected ? target.accentSoft() : target.keyBackground())
            : selected ? target.accent()
            : action ? target.functionBackground() : target.keyBackground();
        String foreground = confirm ? target.returnForeground()
            : tile ? (selected ? target.accentText() : target.keyForeground())
            : selected ? target.onAccent()
            : action ? target.actionForeground() : target.keyForeground();
        float density = s.getResources().getDisplayMetrics().density;
        KeyboardPressButton press = button instanceof KeyboardPressButton key ? key : null;
        // 键帽完全由皮肤、角色、选中状态和密度决定；这几项都没变就留着现在这块，不再每次 render 换一个一样的新 Drawable 让整块键盘重画。
        if (press == null || !press.keepsFace(target, role, selected, density)) {
            if (target.designed()) {
                button.setBackground(new KeyboardSkinKeyDrawable(target,
                    Color.parseColor(background), selected || action || confirm, density));
            } else {
                GradientDrawable drawable = new GradientDrawable();
                drawable.setColor(Color.parseColor(background));
                drawable.setCornerRadius(s.pixels(tile ? MoreToolsLayout.TILE_RADIUS_DP : target.cornerRadius()));
                int borderWidth = s.pixels(target.borderWidth());
                if (borderWidth > 0)
                    drawable.setStroke(borderWidth, Color.parseColor(target.borderColor()));
                button.setBackground(drawable);
            }
            if (press != null) press.rememberFace(target, role, selected, density);
        }
        button.setTextColor(Color.parseColor(foreground));
        if (button instanceof ShuangpinHintButton hintButton)
            hintButton.setHintColor(Color.parseColor(target.accent()));
        if (button instanceof NineKeyDigitButton digitButton)
            digitButton.setDigitColor(Color.parseColor(target.accent()));
        button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        int shadowAlpha = (int) Math.round(255 * target.shadowOpacity());
        int shadowColor = Color.argb(shadowAlpha, 0, 0, 0);
        button.setOutlineAmbientShadowColor(shadowColor);
        button.setOutlineSpotShadowColor(shadowColor);
        button.setElevation(target.shadowOpacity() > 0
            ? s.pixels(Math.max(1, target.shadowRadius() + target.shadowOffset())) : 0);
    }

    /** One of the skin's colours at a fraction of its opacity. */
    static int fade(String color, double opacity) {
        int value = Color.parseColor(color);
        return Color.argb((int) Math.round(255 * KeyboardGeometry.bounded(opacity, 0, 1)),
            Color.red(value), Color.green(value), Color.blue(value));
    }

    /** The outlined badge the keyboard wears while nothing is being composed. */
    GradientDrawable brandPillDrawable() {
        GradientDrawable pill = new GradientDrawable();
        pill.setColor(Color.TRANSPARENT);
        pill.setCornerRadius(s.pixels(14));
        pill.setStroke(Math.max(1, s.pixels(1)), fade(s.skin.accent(), .45));
        return pill;
    }

    GradientDrawable candidateDrawable(int color) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(color);
        drawable.setCornerRadius(s.pixels(6));
        if (Color.alpha(s.candidateAppearance.border()) > 0)
            drawable.setStroke(Math.max(1, s.pixels(1)), s.candidateAppearance.border());
        return drawable;
    }

    Typeface candidateTypeface() {
        try {
            return Typeface.create(s.candidateAppearance.preferredFont(), Typeface.NORMAL);
        } catch (RuntimeException ignored) {
            return Typeface.DEFAULT;
        }
    }

    void applySkinToView(View node) {
        applySkinToView(node, node == s.candidateViewport || node == s.expandedCandidates, s.skin);
    }

    /** Re-walk one subtree with its own surface skin after the keyboard-wide pass. */
    void applySkinToView(View node, KeyboardSkin target) {
        applySkinToView(node, false, target);
    }

    void applySkinToView(View node, boolean candidateContext, KeyboardSkin target) {
        CharSequence description = node.getContentDescription();
        boolean candidate = candidateContext || node == s.candidateViewport || node == s.expandedCandidates
            || (description != null && description.toString().startsWith("候选 "));
        if (node instanceof Button) {
            boolean key = description != null && (description.toString().startsWith("按键 ")
                || description.toString().startsWith("候选 ")
                || description.toString().startsWith("输入方案卡片 "));
            KeyboardKeyRole role = node instanceof KeyboardPressButton press
                ? press.keyboardRole() : null;
            if (candidate) s.imeCandidates.styleCandidateButton((Button) node);
            else if (role != null) styleButton((Button) node, role, target);
            else styleButton((Button) node, !key, target);
            if (description != null && "恢复默认".contentEquals(description))
                ((Button) node).setTextColor(Color.RED);
        } else if (node instanceof TextView) {
            TextView text = (TextView) node;
            text.setTextColor(candidate ? s.candidateAppearance.text() : Color.parseColor(target.keyForeground()));
            text.setTypeface(candidate ? candidateTypeface()
                : target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        }
        if (node instanceof android.view.ViewGroup) {
            android.view.ViewGroup group = (android.view.ViewGroup) node;
            for (int index = 0; index < group.getChildCount(); index++)
                applySkinToView(group.getChildAt(index), candidate, target);
        }
    }

    void applySkin() {
        if (s.keyboardRoot == null) return;
        s.keyboardRoot.setBackgroundColor(Color.parseColor(s.skin.background()));
        if (s.keyboardSurface != null) applySkinBackground(s.keyboardSurface);
        if (s.candidateViewport != null)
            s.candidateViewport.setBackgroundColor(s.candidateAppearance.surface());
        if (s.expandedCandidates != null)
            s.expandedCandidates.setBackgroundColor(s.candidateAppearance.surface());
        if (s.clipboardPanel != null)
            applySkinBackground(s.clipboardPanel);
        if (s.schemePanel != null)
            applySkinBackground(s.schemePanel);
        if (s.skinPanel != null)
            applySkinBackground(s.skinPanel);
        if (s.layoutSettingsPanel != null)
            applySkinBackground(s.layoutSettingsPanel);
        if (s.moreToolsPanel != null)
            applySkinBackground(s.moreToolsPanel);
        if (s.emojiPanel != null)
            applySkinBackground(s.emojiPanel, s.emojiSkin);
        if (s.voiceResultPanel != null)
            applySkinBackground(s.voiceResultPanel);
        if (s.aiPolishPanel != null)
            applySkinBackground(s.aiPolishPanel);
        if (s.aiPolishContainer != null)
            applySkinBackground(s.aiPolishContainer);
        if (s.replyKeyboard != null)
            applySkinBackground(s.replyKeyboard);
        if (s.handwritingCanvas != null) s.handwritingCanvas.applySkin(s.handwritingSkin);
        applySkinToView(s.keyboardRoot);
        // The keyboard-wide pass already styled these subtrees; re-walk the two that carry their
        // own light/dark setting so only their faces change.
        if (s.emojiPanel != null) {
            applySkinToView(s.emojiPanel, s.emojiSkin);
            s.imePanels.styleEmojiChrome();
        }
        if (s.handwritingActive() && s.keyRows != null) applySkinToView(s.keyRows, s.handwritingSkin);
        // 回复面板的分段控件、源文字卡片和操作列不按角色上色，上面那一遍把它们清成了无底色，这里补回来。
        s.imePanels.styleReplyKeyboard();
        s.imeLayoutRows.applySidebarRail();
        if (s.preedit != null) {
            // Idle, this is the brand badge the shared design draws as an outlined pill; composing, it is the reading itself, set in the strip's typeface and its secondary colour above the candidates.
            s.preedit.setTextColor(s.brandPillVisible
                ? Color.parseColor(s.skin.accent()) : s.candidateAppearance.number());
            s.preedit.setTypeface(candidateTypeface());
            s.preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP,
                s.brandPillVisible ? 12 : s.candidatePreeditFontSize);
            s.preedit.setBackground(s.brandPillVisible ? brandPillDrawable() : null);
            s.preedit.setPadding(s.pixels(s.brandPillVisible ? 12 : 2), s.pixels(s.brandPillVisible ? 4 : 0),
                s.pixels(s.brandPillVisible ? 12 : 2), s.pixels(s.brandPillVisible ? 4 : 0));
        }
        if (s.candidateBrandMark != null) s.candidateBrandMark.invalidate();
        if (s.status != null) s.status.setTextColor(fade(s.skin.accent(), .55));
        if (s.candidatePage != null) {
            s.candidatePage.setTextColor(s.candidateAppearance.accent());
            s.candidatePage.setTypeface(candidateTypeface());
        }
        if (s.layoutAdjustView != null) s.layoutAdjustView.updateSkin(s.skin);
    }

    void applySkinBackground(View node) { applySkinBackground(node, s.skin); }

    void applySkinBackground(View node, KeyboardSkin target) {
        float density = s.getResources().getDisplayMetrics().density;
        // 同一个皮肤对象画出的底图完全一样；已经是它就不再换新的，免得每按一个键都让整块键盘底图重画（照片皮肤还要重新上传位图）。
        if (node.getBackground() instanceof KeyboardSkinBackgroundDrawable current
                && current.draws(target, density)) return;
        node.setBackground(new KeyboardSkinBackgroundDrawable(target, density));
    }

    /** Keep phone keys edge-to-edge while a tablet or two-in-one gets a bounded, centred surface. */
    void applyKeyboardSurfaceGeometry() {
        if (s.keyboardSurface == null) return;
        Configuration configuration = s.getResources().getConfiguration();
        int widthDp = KeyboardFormFactorPolicy.surfaceWidthDp(
            configuration.smallestScreenWidthDp, configuration.screenWidthDp);
        int width = widthDp == 0 ? FrameLayout.LayoutParams.MATCH_PARENT : s.pixels(widthDp);
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
            width, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        s.keyboardSurface.setLayoutParams(params);
        s.keyboardSurface.setElevation(widthDp == 0 ? 0 : s.pixels(10));
    }
}
