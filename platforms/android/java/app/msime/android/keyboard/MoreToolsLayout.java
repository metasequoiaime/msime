package app.msime.android;

/** Platform-neutral layout and accessibility contract for the Apple-style tools panel. */
public final class MoreToolsLayout {
    /** A list card: the local-input tools and their way back. */
    public static final int CARD_HEIGHT_DP = 48;
    /** The design's function-panel tile: four to a row, icon over title. */
    public static final int TILE_HEIGHT_DP = 52;
    public static final int TILE_RADIUS_DP = 16;
    public static final int HEADER_HEIGHT_DP = 44;
    public static final int ROW_SPACING_DP = 6;
    public static final int CARD_SPACING_DP = 8;
    /** 功能菜单每页四列三行，条目见 {@link FunctionPanelModel}。 */
    public static final int PAGE_COLUMNS = FunctionPanelModel.COLUMNS;
    public static final int PAGE_ROWS = FunctionPanelModel.ROWS;

    public enum Section {
        TOOLS("", 4),
        SETTINGS("设置", 4),
        LOCAL_INPUT("本地输入", 2),
        LOCAL_INPUT_BACK("", 1);

        private final String title;
        private final int columns;

        Section(String title, int columns) {
            this.title = title;
            this.columns = columns;
        }

        public String title() { return title; }
        public int columns() { return columns; }
        /** Whether this section is drawn as the design's icon-over-title tiles. */
        public boolean tiles() { return this == TOOLS || this == SETTINGS; }
        public int height() { return tiles() ? TILE_HEIGHT_DP : CARD_HEIGHT_DP; }
    }

    private MoreToolsLayout() { }

    public static int rowCount(int itemCount, Section section) {
        if (itemCount < 0) throw new IllegalArgumentException("Negative item count");
        return (itemCount + section.columns() - 1) / section.columns();
    }

    /** 新设计功能面板的页数。 */
    public static int pageCount() { return FunctionPanelModel.pageCount(); }

    /** 新设计功能面板条目的文字图标（{@link #icon(String)} 按标签取）。 */
    public static String icon(FunctionPanelModel.Item item) {
        return icon(item == null ? null : item.label());
    }

    public static String state(Section section, boolean active) {
        return switch (section) {
            case SETTINGS -> active ? "已开启" : "已关闭";
            case TOOLS, LOCAL_INPUT, LOCAL_INPUT_BACK -> "点击打开";
        };
    }

    /** 原生卡片使用的简短文字图标。 */
    public static String icon(String title) {
        if (title == null) return "⌘";
        return switch (title) {
            case "表情" -> "☺";
            case "剪贴板历史", "剪贴板" -> "▤";
            case "设备信息" -> "▣";
            case "AI 润色", "AI 回复与润色", "AI 辅助" -> "✦";
            case "本地输入", "返回工具" -> "⌘";
            case "应用设置", "设置", "编辑工具栏" -> "⚙";
            case "语音输入", "语音结果", "插入语音文字" -> "◉";
            case "繁体输出", "繁体" -> "繁";
            case "全角输入" -> "Ａ";
            case "全角" -> "全";
            case "中文标点" -> "，";
            case "模糊音" -> "≈";
            case "手写" -> "✎";
            case "拍照识字" -> "照";
            case "词库" -> "▦";
            case "键盘高度" -> "⇕";
            case "按键音" -> "♪";
            case "按键振动", "振动" -> "◌";
            case "振动强度" -> "↕";
            case "单手模式" -> "◧";
            case "浮动键盘" -> "⧉";
            case "隐私模式", "无痕输入" -> "⛉";
            case "反馈" -> "✉";
            case "关于" -> "ⓘ";
            case "打开官网" -> "↗";
            case "宣传" -> "吆";
            case "文本编辑" -> "⇄";
            case "世界时钟" -> "◷";
            default -> "⌨";
        };
    }
}
