package app.msime.android;

/** Stable candidate-management menu order and host operation metadata. */
public enum CandidateManagementAction {
    PROMOTE("优先显示", "已优先显示", false),
    FIX_FIRST("固定到首位", "已固定到首位", false),
    CLEAR_POSITION("取消固定", "已取消固定", false),
    REMOVE("删除词条…", "已删除词条", true);

    private static final int MENU_ITEM_BASE = 1000;
    /** 长按菜单「加入生词本」的条目 id：避开可上屏释义（2000 起，最多 {@link CandidateTranslationPolicy#MAX_INSERTION_SENSES} 项）和上面这些词库操作的 id。 */
    public static final int PERSONAL_WORDBOOK_MENU_ITEM = 3000;
    /** 「加入生词本」的排序：比所有词库操作都大，排在菜单最后，和 iOS、鸿蒙的长按菜单、桌面的候选右键菜单同一个位置。 */
    public static final int PERSONAL_WORDBOOK_ORDER = 1000;
    public static final String PERSONAL_WORDBOOK_TITLE = "加入生词本";
    private final String title;
    private final String announcement;
    private final boolean confirmationRequired;

    CandidateManagementAction(String title, String announcement, boolean confirmationRequired) {
        this.title = title;
        this.announcement = announcement;
        this.confirmationRequired = confirmationRequired;
    }

    public String title() { return title; }
    public String announcement() { return announcement; }
    public boolean confirmationRequired() { return confirmationRequired; }
    public int menuItemId() { return MENU_ITEM_BASE + ordinal(); }

    /**
     * 确认那一项的文字（收在同一个长按菜单的子菜单里，与 iOS 的嵌套 `UIMenu` 同款）。只有
     * {@link #REMOVE} 需要确认，别的操作调它是写错了。
     */
    public String confirmationTitle() {
        if (this != REMOVE) throw new IllegalStateException("Action does not require confirmation");
        return "确认删除此词条";
    }

    public int fixedPosition() {
        if (this != FIX_FIRST) throw new IllegalStateException("Action does not fix a position");
        return 1;
    }

    public static CandidateManagementAction fromMenuItemId(int itemId) {
        int index = itemId - MENU_ITEM_BASE;
        CandidateManagementAction[] actions = values();
        if (index < 0 || index >= actions.length)
            throw new IllegalArgumentException("Unknown candidate management action");
        return actions[index];
    }

    /** 只在这一行显示着可上屏的释义时才有「加入生词本」：Engine 的注释（例如五笔编码）不是释义，不会出现在 {@code glosses} 里。 */
    public static boolean offersPersonalWordbook(java.util.List<String> glosses) {
        return glosses != null && !glosses.isEmpty();
    }

    public static int validatePosition(int position) {
        if (position < 1 || position > 5)
            throw new IllegalArgumentException("Candidate position must be between 1 and 5");
        return position;
    }
}
