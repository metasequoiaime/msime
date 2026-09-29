package app.msime.client;

/**
 * 一个键在皮肤里扮演的视觉角色。
 *
 * <p>The style pass used to carry a single boolean: a key wore the key face, and everything else
 * wore the filled accent face. That is why the shortcut strip came out as a row of solid green
 * blocks. A button now says which face it wants, so the capless treatments the shared design uses
 * for toolbars, sidebars and paging controls are expressible instead of being approximated by the
 * one filled face.
 */
public enum KeyboardKeyRole {
    /** A key cap: the skin's key background and label colour. */
    KEY,
    /** A function key such as ⇧, ⌫, 123, 中 or 换行: the theme's tinted function-key face. */
    ACCENT,
    /** 换行 while a composition is open (确认): the theme's accent fill. */
    RETURN,
    /** A function-panel tile: the key face while off, the soft accent surface with accent text while on. */
    TILE,
    /** The idle top row's scheme pill (全拼): a flat, fully rounded key-coloured face inset inside its touch target, without the cap's border or shadow. */
    PILL,
    /** A key without a cap, keeping the key label colour; the nine-key punctuation column. */
    PLAIN,
    /** A capless control drawn in the accent colour: the toolbar glyphs and the paging controls. */
    GLYPH;

    /** Whether the skin's cap background, border and shadow apply to this role. */
    public boolean drawsCap() { return this == KEY || this == ACCENT || this == RETURN || this == TILE; }

    /** Whether the label takes the accent colour rather than the skin's key label colour. */
    public boolean usesAccentLabel() { return this == GLYPH; }

    /** Whether the key spacing setting insets this role inside its row. */
    public boolean followsKeySpacing() { return this != GLYPH && this != PILL; }
}
