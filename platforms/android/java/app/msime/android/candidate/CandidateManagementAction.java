package app.msime.android;

/** Stable candidate-management menu order and host operation metadata. */
public enum CandidateManagementAction {
    PROMOTE("优先显示", "已优先显示", false),
    FIX_FIRST("固定到首位", "已固定到首位", false),
    CLEAR_POSITION("取消固定", "已取消固定", false),
    REMOVE("删除词条…", "已删除词条", true);

    private static final int MENU_ITEM_BASE = 1000;
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

    public static int validatePosition(int position) {
        if (position < 1 || position > 5)
            throw new IllegalArgumentException("Candidate position must be between 1 and 5");
        return position;
    }
}
