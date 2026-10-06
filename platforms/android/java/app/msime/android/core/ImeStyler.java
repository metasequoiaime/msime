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
import java.util.HashMap;
import java.util.Map;
import java.time.LocalDate;
import java.time.ZoneId;
import org.json.JSONException;
import org.json.JSONObject;

/** 键盘的着色与几何：按键样式、皮肤套用、键距行距与键盘高度；从 MSIMEInputService 原样搬出。 */
final class ImeStyler {
    private final MSIMEInputService s;
    private final Map<String, Integer> colorCache = new HashMap<>();

    ImeStyler(MSIMEInputService s) {
        this.s = s;
    }

    /** Skin colours repeat across every key in a render; keep the bounded palette parsed once. */
    private int color(String value) {
        Integer cached = colorCache.get(value);
        if (cached != null) return cached;
        if (colorCache.size() >= 64) colorCache.clear();
        int parsed = Color.parseColor(value);
        colorCache.put(value, parsed);
        return parsed;
    }

    // 偏好里没有 `app_theme` 时的默认值，与 Rust 的 `AppTheme::default()` 一致。
    /** 换季检查的最短间隔：每次渲染都会问一次，但月份一分钟内不会变。 */
    private static final long SEASON_CHECK_INTERVAL_MS = 60_000;
    private String seedTheme;
    private int seedMonth;
    private long seedCheckedAt = Long.MIN_VALUE;
    private AppThemePalette.Seed seed;
    private KeyboardSkin seededLight;
    private KeyboardSkin seededDark;

    /**
     * 跟随系统皮肤按应用主题取色：`system` 皮肤换成 {@link KeyboardSkin#system(boolean, AppThemePalette.Seed)}，其他皮肤原样返回。SVC 每次按偏好重建 `skin` 时拿到的是 classic 基础色，这里在着色时统一换掉，所以不必改 SVC 的每个赋值点。
     */
    KeyboardSkin themed(KeyboardSkin target) {
        if (target == null || target.designed() || !"system".equals(target.id())) return target;
        if (target == seededLight || target == seededDark) return target;
        if (seed == null) refreshAppTheme(true);
        if (target.dark()) {
            if (seededDark == null) seededDark = KeyboardSkin.system(true, seed);
            return seededDark;
        }
        if (seededLight == null) seededLight = KeyboardSkin.system(false, seed);
        return seededLight;
    }

    /** 当前应用主题的种子；还没解析过时先解析一次。 */
    AppThemePalette.Seed appThemeSeed() {
        if (seed == null) refreshAppTheme(true);
        return seed;
    }

    /**
     * 重新解析应用主题（`app_theme` + 本地月份），种子变了时丢掉已推导的键盘色。
     *
     * @param force 为假时一分钟内只检查一次
     * @return 种子是否因此变了
     */
    boolean refreshAppTheme(boolean force) {
        long now = android.os.SystemClock.uptimeMillis();
        if (!force && seedCheckedAt != Long.MIN_VALUE && now - seedCheckedAt < SEASON_CHECK_INTERVAL_MS)
            return false;
        seedCheckedAt = now;
        String theme = s.localSettings.choice(AndroidLocalSettings.APP_THEME);
        int month = LocalDate.now(ZoneId.systemDefault()).getMonthValue();
        if (seed != null && theme.equals(seedTheme) && month == seedMonth) return false;
        AppThemePalette.Seed next = AppThemePalette.Seed.fromResolved(
            resolveAppTheme(theme, month, false), resolveAppTheme(theme, month, true));
        if (next == null) next = AppThemePalette.Seed.AUTUMN;
        boolean changed = seed == null || !seed.id.equals(next.id) || !seed.season.equals(next.season);
        seedTheme = theme;
        seedMonth = month;
        seed = next;
        if (changed) {
            seededLight = null;
            seededDark = null;
        }
        return changed;
    }

    /** 渲染时调用：换季或换了应用主题就重新套一遍皮肤；同时让单手模式与调试开关跟上偏好。 */
    void refreshSeasonIfNeeded() {
        if (refreshAppTheme(false)) applySkin();
        s.imeFrame.applyOneHanded();
        s.imeDebugOverlay.refreshPreferences();
    }

    private static JSONObject resolveAppTheme(String theme, int month, boolean dark) {
        try {
            JSONObject root = new JSONObject(NativeClient.resolveAppTheme(theme, month, dark));
            return root.optBoolean("ok", false) ? root.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    /** The key spacing the displayed layout draws with: the user's setting, capped on the eleven-column Dachen rows (KeyboardGeometry.layoutKeySpacing). */
    private int layoutKeySpacingTenths() {
        return KeyboardGeometry.layoutKeySpacing(s.touchKeySpacingTenths, s.displayedTouchLayout(s.view));
    }

    void applyKeyboardGeometry(View node) {
        if (s.followsKeySpacing(node)
                && node.getLayoutParams() instanceof android.view.ViewGroup.MarginLayoutParams) {
            android.view.ViewGroup.MarginLayoutParams params =
                (android.view.ViewGroup.MarginLayoutParams) node.getLayoutParams();
            int horizontal = s.halfSpacingPixels(layoutKeySpacingTenths());
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
        // 设计的键区左右外边距 6 dp 量到键的边缘；键自己带半个键距的外边距，所以容器只补差值。
        int edge = BoundsPolicy.nonNegative(
            s.pixels(KeyboardGeometry.DESIGN_PADDING_HORIZONTAL_DP)
                - s.halfSpacingPixels(layoutKeySpacingTenths()));
        s.keyRows.setPadding(edge, s.keyRows.getPaddingTop(), edge, s.keyRows.getPaddingBottom());
        if (s.actionRow != null)
            s.actionRow.setPadding(edge, s.actionRow.getPaddingTop(), edge,
                s.actionRow.getPaddingBottom());
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
            height += s.halfSpacingPixels(s.touchRowSpacingTenths) * 2 * role.rowSpacings;
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

    /**
     * 九键、笔画、手写、大千四行的三行键块：与 26 键的三行字母键同高（3 × {@link KeyboardGeometry#KEY_ROW_HEIGHT_DP} + 整份键高调整 + 三份行距）。
     */
    void adjustThreeRowBlockHeight(View view) {
        adjustRowBlockHeight(view, 3);
    }

    /** 占 {@code rows} 行键高的整块（九键网格连同侧栏），按键盘高度偏好缩放，并按行数加上行距。 */
    void adjustRowBlockHeight(View view, int rows) {
        view.setTag(new MSIMEInputService.KeyboardHeightRole(
            KeyboardGeometry.KEY_ROW_HEIGHT_DP * rows, 1, 0, rows));
    }

    /** 连同底栏位置一起占用的整块（日语九键、注音九键没有底栏，四行都在块里）：三行键高加一条底栏高，底栏不带行距，与其他布局总高相同。 */
    void adjustBottomRowBlockHeight(View view) {
        view.setTag(new MSIMEInputService.KeyboardHeightRole(
            KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3 + KeyboardGeometry.STANDARD_ROW_HEIGHT_DP, 1, 0, 3));
    }

    void styleButton(Button button, boolean action) { styleButton(button, action, s.skin); }

    void styleButton(Button button, boolean action, KeyboardSkin target) {
        styleButton(button, action ? KeyboardKeyRole.ACCENT : KeyboardKeyRole.KEY, target);
    }

    /** `target` is the surface's own skin: the emoji and handwriting panels carry their own theme. */
    void styleButton(Button button, KeyboardKeyRole role, KeyboardSkin target) {
        target = themed(target);
        if (button instanceof KeyboardIconKey icon && button == s.shiftButton) {
            styleShiftKey(icon, target);
            return;
        }
        boolean selected = button.isSelected();
        // 选中的控件一律换成实心强调色，大小写键和简繁开关就是这样表示「开着」的。确认键和功能面板磁贴自己画开启状态，保留原角色；工具栏图标按钮（如打开回复面板时的「回复」）也不铺实心块，而是在图标后垫一块柔和的强调色底，和磁贴的开启状态是同一种表达。
        boolean toolbarGlyph = button instanceof KeyboardShortcutButton;
        KeyboardKeyRole face = selected && !toolbarGlyph && role != KeyboardKeyRole.RETURN
            && role != KeyboardKeyRole.TILE ? KeyboardKeyRole.ACCENT : role;
        if (face == KeyboardKeyRole.PILL) {
            // The pill is a label on the strip rather than a key, so it keeps a plain rounded face even over a designed skin, inset so the 44dp target stays.
            GradientDrawable pill = DrawablePolicy.rounded(color(target.keyBackground()),
                s.pixels(14));
            button.setBackground(new InsetDrawable(pill,
                s.pixels(2), s.pixels(8), s.pixels(2), s.pixels(8)));
            button.setTextColor(color(target.keyForeground()));
            button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
            button.setElevation(0);
            return;
        }
        if (!face.drawsCap()) {
            button.setBackground(null);
            String label = face.usesAccentLabel() ? target.accent() : target.keyForeground();
            if (button instanceof KeyboardShortcutButton shortcut) {
                shortcut.setActiveFill(color(target.accentSoft()));
                if (selected) label = target.accentText();
            }
            button.setTextColor(color(label));
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
            : action ? target.functionForeground() : target.keyForeground();
        float density = KeyboardGeometry.density(s);
        KeyboardPressButton press = button instanceof KeyboardPressButton key ? key : null;
        // 键帽完全由皮肤、角色、选中状态和密度决定；这几项都没变就留着现在这块，不再每次 render 换一个一样的新 Drawable 让整块键盘重画。
        if (press == null || !press.keepsFace(target, role, selected, density)) {
            if (target.designed()) {
                button.setBackground(new KeyboardSkinKeyDrawable(target,
                    color(background), selected || action || confirm, density));
            } else {
                GradientDrawable drawable = DrawablePolicy.rounded(color(background),
                    s.pixels(tile ? MoreToolsLayout.TILE_RADIUS_DP : target.cornerRadius()));
                int borderWidth = s.pixels(target.borderWidth());
                if (borderWidth > 0)
                    drawable.setStroke(borderWidth, color(target.borderColor()));
                button.setBackground(drawable);
            }
            if (press != null) press.rememberFace(target, role, selected, density);
        }
        button.setTextColor(color(foreground));
        if (button instanceof KeyHintButton hintButton) {
            hintButton.setHintColor(color(target.accent()));
            hintButton.setCornerHintColor(color(target.hint()));
        }
        if (button instanceof SpaceKeyFace space)
            space.setFaceColor(color(target.toolbarIcon()));
        if (button instanceof NineKeyDigitButton digitButton)
            digitButton.setDigitColor(color(target.accent()));
        button.setTypeface(target.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        applyShadow(button, target);
    }

    /**
     * 新设计的 Shift：键帽始终是功能键底色；开启（一次性或锁定）时换成字母键底色、图标 accent，不再铺实心 accent。锁定时图标换成带下划线的大写锁定形。
     */
    private void styleShiftKey(KeyboardIconKey key, KeyboardSkin target) {
        boolean on = key.isSelected();
        key.setKind(s.letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK
            ? KeyboardIconKey.Kind.CAPS_LOCK : KeyboardIconKey.Kind.SHIFT);
        String background = on ? target.keyBackground() : target.functionBackground();
        String foreground = on ? target.accent() : target.functionForeground();
        float density = KeyboardGeometry.density(s);
        // 记忆键帽时按「开着」当作 KEY 角色，免得开关切换后沿用旧的那块底图。
        KeyboardKeyRole remembered = on ? KeyboardKeyRole.KEY : KeyboardKeyRole.ACCENT;
        if (!key.keepsFace(target, remembered, false, density)) {
            if (target.designed()) {
                key.setBackground(new KeyboardSkinKeyDrawable(target,
                    color(background), !on, density));
            } else {
                GradientDrawable drawable = DrawablePolicy.rounded(color(background),
                    s.pixels(target.cornerRadius()));
                int borderWidth = s.pixels(target.borderWidth());
                if (borderWidth > 0)
                    drawable.setStroke(borderWidth, color(target.borderColor()));
                key.setBackground(drawable);
            }
            key.rememberFace(target, remembered, false, density);
        }
        key.setTextColor(color(foreground));
        applyShadow(key, target);
    }

    private void applyShadow(View view, KeyboardSkin target) {
        int shadowAlpha = (int) Math.round(255 * target.shadowOpacity());
        int shadowColor = Color.argb(shadowAlpha, 0, 0, 0);
        view.setOutlineAmbientShadowColor(shadowColor);
        view.setOutlineSpotShadowColor(shadowColor);
        view.setElevation(target.shadowOpacity() > 0
            ? s.pixels(Math.max(1, target.shadowRadius() + target.shadowOffset())) : 0);
    }

    /** One of the skin's colours at a fraction of its opacity. */
    static int fade(String color, double opacity) {
        int value = Color.parseColor(color);
        return ColorPolicy.withAlpha(value,
            (float) KeyboardGeometry.bounded(opacity, 0, 1));
    }

    /** The outlined badge the keyboard wears while nothing is being composed. */
    GradientDrawable brandPillDrawable() {
        return DrawablePolicy.outlined(s.pixels(14), KeyboardGeometry.atLeastOnePixel(s, 1),
            fade(s.skin.accent(), .45));
    }

    GradientDrawable candidateDrawable(int color) {
        int border = s.candidateAppearance.border();
        return roundedFace(color, s.pixels(6),
            Color.alpha(border) > 0 ? KeyboardGeometry.atLeastOnePixel(s, 1) : 0, border);
    }

    private static GradientDrawable roundedFace(int color, float radius, int borderWidth,
                                                int borderColor) {
        return borderWidth > 0
            ? DrawablePolicy.outlined(color, radius, borderWidth, borderColor)
            : DrawablePolicy.rounded(color, radius);
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
        target = themed(target);
        CharSequence description = node.getContentDescription();
        boolean candidate = candidateContext || node == s.candidateViewport || node == s.expandedCandidates
            || (description != null && description.toString().startsWith("候选 "));
        if (node instanceof KeyboardSkinCard) {
            // 皮肤面板的瓷砖自己画缩略图、描边和名字，配色由 ImePanels.styleSkinPicker 经 setTileColors 传入。当普通按钮上色会给每格铺一层卡片底，选中格更被整块填成强调色，盖住名字和缩略图。
        } else if (node instanceof Button) {
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
            text.setTextColor(candidate ? s.candidateAppearance.text() : color(target.keyForeground()));
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
        refreshAppTheme(false);
        s.skin = themed(s.skin);
        s.emojiSkin = themed(s.emojiSkin);
        s.handwritingSkin = themed(s.handwritingSkin);
        s.keyboardRoot.setBackgroundColor(color(s.skin.background()));
        s.imeFrame.applyNavigationBar(color(s.skin.background()), s.skin.dark());
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
                ? color(s.skin.accent()) : s.candidateAppearance.number());
            s.preedit.setTypeface(candidateTypeface());
            KeyboardGeometry.setKeyTextSize(s.preedit, s.brandPillVisible ? 12 : s.candidatePreeditFontSize);
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
        s.imeFrame.applyOneHanded();
    }

    void applySkinBackground(View node) { applySkinBackground(node, s.skin); }

    void applySkinBackground(View node, KeyboardSkin target) {
        target = themed(target);
        float density = KeyboardGeometry.density(s);
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
