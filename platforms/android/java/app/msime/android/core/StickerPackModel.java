package app.msime.android;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 表情面板第一个分类「水杉娘」表情包：随包清单的校验、分类签的排序和默认分类、网格列数、文案和「能不能直接发图」的判断。
 *
 * <p>只放不依赖 Android 运行时的部分，JVM 冒烟测得到。清单（`assets/stickers/shuishan/manifest.json`，来自 `resources/stickers/shuishan`）的顺序就是显示顺序，`name` 是含义，读屏和提示都用它。解码、发送和剪贴板在 {@link ImeStickers}。
 */
public final class StickerPackModel {
    /** APK 里表情包所在的 assets 目录，`build-apk.sh` 把 `resources/stickers/shuishan` 原样放到这里。 */
    public static final String ASSET_DIRECTORY = "stickers/shuishan";
    public static final String MANIFEST = "manifest.json";
    /** 清单最多读这么多字节：66 条的清单约 2 KB，留足余量但不让一份坏清单把整个文件读进内存。 */
    public static final int MAX_MANIFEST_BYTES = 64 * 1024;
    public static final int MAX_STICKERS = 128;
    public static final int MAX_NAME_CODE_POINTS = 16;
    /** 源图边长的上限：素材是 320×320，超出这个数说明 assets 被换成了不该在键盘进程里整张解码的东西。 */
    public static final int MAX_SOURCE_EDGE = 1024;
    /** 竖屏手机上一行五格；更宽时按每格约 72 dp 加列。 */
    public static final int MIN_COLUMNS = 5;
    public static final int CELL_DP = 72;
    /** 贴纸四周的留白。 */
    public static final int CELL_PADDING_DP = 4;
    /** 分类签上那张缩略图的边长，和其他分类签 17 sp 的 Emoji 图标看起来一样大。 */
    public static final int TAB_ICON_DP = 24;
    /** 发出去的 PNG 放在应用缓存下的这个目录里，`res/xml/msime_file_paths.xml` 按同名把它暴露给 FileProvider。目录里始终至多一张，只在写下一张时替换，不定时删：复制到剪贴板的那张要在键盘进程被回收后仍能粘贴。 */
    public static final String DIRECTORY = "sticker-images";
    /** 发出去的类型：WebP 解码后转成保留透明的 PNG，收图的应用普遍认 PNG，不一定认 WebP。 */
    public static final String MIME_TYPE = "image/png";
    public static final String EXTENSION = "png";
    /** 分类签的无障碍描述。 */
    public static final String TAB_DESCRIPTION = "水杉娘表情包";
    /** 「最近」的分类序号；各 Emoji 分类从 0 起编号。 */
    public static final int RECENTS_CATEGORY = -1;
    /** 没有可切换的分类（横划已在两端，或当前分类不在底栏里）。 */
    public static final int NO_CATEGORY = Integer.MIN_VALUE;
    private static final Pattern FILE = Pattern.compile("[0-9]{2}\\.webp");

    private StickerPackModel() {}

    /** 一张贴纸：assets 里的文件名和它的含义。 */
    public record Sticker(String file, String name) {
        public Sticker {
            if (file == null || !FILE.matcher(file).matches())
                throw new IllegalArgumentException("Invalid sticker file");
            if (!validName(name)) throw new IllegalArgumentException("Invalid sticker name");
        }
    }

    /** 整个表情包：名字（「水杉娘」）和按显示顺序排好的贴纸。 */
    public record Pack(String name, List<Sticker> stickers) {
        public Pack {
            if (!validName(name)) throw new IllegalArgumentException("Invalid sticker pack name");
            if (stickers == null || stickers.isEmpty() || stickers.size() > MAX_STICKERS)
                throw new IllegalArgumentException("Invalid sticker count");
            HashSet<String> files = new HashSet<>();
            for (Sticker sticker : stickers) {
                if (sticker == null || !files.add(sticker.file()))
                    throw new IllegalArgumentException("Duplicate sticker file");
            }
            stickers = List.copyOf(stickers);
        }
    }

    /** 由清单里取出的原始值构造一张贴纸；值不是字符串、文件名不安全或名字越界时抛 IllegalArgumentException。 */
    public static Sticker sticker(Object file, Object name) {
        if (!(file instanceof String) || !(name instanceof String))
            throw new IllegalArgumentException("Invalid sticker entry");
        return new Sticker((String) file, (String) name);
    }

    /** 由清单里取出的原始值构造整个表情包。 */
    public static Pack pack(Object name, List<Sticker> stickers) {
        if (!(name instanceof String)) throw new IllegalArgumentException("Invalid sticker pack name");
        return new Pack((String) name, stickers);
    }

    /** 解析随包清单（`{"name": …, "stickers": [{"file": …, "name": …}, …]}`）；任何一处不合规就整份拒绝。 */
    public static Pack decode(String document) throws JSONException {
        if (document == null || document.length() > MAX_MANIFEST_BYTES)
            throw new JSONException("Invalid sticker manifest");
        JSONObject root = new JSONObject(document);
        JSONArray entries = root.getJSONArray("stickers");
        if (entries.length() > MAX_STICKERS) throw new JSONException("Too many stickers");
        ArrayList<Sticker> stickers = new ArrayList<>(entries.length());
        try {
            for (int index = 0; index < entries.length(); index++) {
                JSONObject entry = entries.getJSONObject(index);
                stickers.add(sticker(entry.opt("file"), entry.opt("name")));
            }
            return pack(root.opt("name"), stickers);
        } catch (IllegalArgumentException error) {
            throw new JSONException("Invalid sticker manifest");
        }
    }

    /** assets 里这张贴纸的路径。 */
    public static String assetPath(Sticker sticker) {
        return ASSET_DIRECTORY + "/" + sticker.file();
    }

    /** 表情包的分类序号：取在所有 Emoji 分类之后的那个号。它只是标识，显示时排在第一个，见 {@link #categoryOrder}。 */
    public static int category(int emojiCategories) {
        return emojiCategories;
    }

    /**
     * 底栏分类签从左到右的分类序号，横划也按这个顺序切换：表情包在时排第一，之后是「最近」（有记录时才有）和各 Emoji 分类。读不到清单（`stickers` 为 false）时就是原来的「最近、各 Emoji 分类」。
     */
    public static List<Integer> categoryOrder(boolean stickers, boolean recents, int emojiCategories) {
        ArrayList<Integer> order = new ArrayList<>(emojiCategories + 2);
        if (stickers) order.add(category(emojiCategories));
        if (recents) order.add(RECENTS_CATEGORY);
        for (int index = 0; index < emojiCategories; index++) order.add(index);
        return List.copyOf(order);
    }

    /** 打开表情面板时选中的分类：底栏第一个，即表情包；没有表情包时有最近记录就是「最近」，否则是第一个 Emoji 分类。 */
    public static int defaultCategory(boolean stickers, boolean recents, int emojiCategories) {
        List<Integer> order = categoryOrder(stickers, recents, emojiCategories);
        return order.isEmpty() ? NO_CATEGORY : order.get(0);
    }

    /** 从 `current` 横划一步（`step` 为 1 往右、-1 往左）到的分类；在两端不回绕，当前分类不在底栏里时也不切换，都返回 {@link #NO_CATEGORY}。 */
    public static int swipeCategory(int current, int step, boolean stickers, boolean recents, int emojiCategories) {
        List<Integer> order = categoryOrder(stickers, recents, emojiCategories);
        int position = order.indexOf(current);
        if (position < 0) return NO_CATEGORY;
        int next = position + Integer.signum(step);
        return next >= 0 && next < order.size() ? order.get(next) : NO_CATEGORY;
    }

    /** 网格一行几格：至少五格，宽度每多约 72 dp 多一格。 */
    public static int columns(int widthDp) {
        return Math.max(MIN_COLUMNS, widthDp / CELL_DP);
    }

    /** 一格里贴纸能画的边长（格子边长减去四周留白），缩略图按它解码；至少 1。 */
    public static int thumbnailEdge(int cellPx, int paddingPx) {
        return Math.max(1, cellPx - 2 * Math.max(0, paddingPx));
    }

    /**
     * 把 `width`×`height` 的源图等比缩到长边不超过 `edge`（不放大），返回 `{宽, 高}`；源图尺寸无效或超过 {@link #MAX_SOURCE_EDGE} 时抛 IllegalArgumentException。
     */
    public static int[] fit(int width, int height, int edge) {
        if (width <= 0 || height <= 0 || width > MAX_SOURCE_EDGE || height > MAX_SOURCE_EDGE || edge <= 0)
            throw new IllegalArgumentException("Invalid sticker size");
        int longest = Math.max(width, height);
        if (longest <= edge) return new int[] {width, height};
        double scale = edge / (double) longest;
        return new int[] {
            Math.max(1, (int) Math.round(width * scale)), Math.max(1, (int) Math.round(height * scale))};
    }

    /** 每格的无障碍描述，例如「水杉娘 开心」。 */
    public static String description(Pack pack, Sticker sticker) {
        return pack.name() + " " + sticker.name();
    }

    /** 网格的无障碍描述；以「表情网格；」开头，和表情分类的网格是同一个控件。 */
    public static String gridDescription(int columns) {
        return "表情网格；" + TAB_DESCRIPTION + "，每行 " + columns + " 个";
    }

    /** 网格的状态描述（读屏念出、设备测试据此数数量）。 */
    public static String status(Pack pack) {
        return pack.name() + " · " + pack.stickers().size() + " 张表情";
    }

    /** 输入框不收图片（或没收下）时，贴纸放进了系统剪贴板，告诉用户怎么发出去。 */
    public static String copiedNotice(Pack pack, Sticker sticker) {
        return "已复制" + pack.name() + "「" + sticker.name() + "」，长按输入框粘贴发送";
    }

    /** 当前输入框经 `EditorInfo.contentMimeTypes` 声明收得下 PNG 时直接经 `commitContent` 发图，否则退回剪贴板。 */
    public static boolean commitsContent(String[] editorMimeTypes) {
        return ClipboardImagePolicy.accepts(editorMimeTypes, MIME_TYPE);
    }

    /**
     * 系统剪贴板里的这个 URI 是不是键盘自己放进去的贴纸：本应用 FileProvider（`<包名>.files`）下 {@link #DIRECTORY} 里的文件。
     *
     * <p>键盘写剪贴板也会触发自己的剪贴板监听；不排除的话，刚复制的贴纸会被当成用户复制的图片，在工具栏上变成「发送复制的图片」。
     */
    public static boolean ownClipUri(String scheme, String authority, String path, String packageName) {
        return "content".equals(scheme) && packageName != null && !packageName.isEmpty()
            && (packageName + ".files").equals(authority)
            && path != null && path.startsWith("/" + DIRECTORY + "/");
    }

    private static boolean validName(String name) {
        return TextPolicy.validBoundedText(name, MAX_NAME_CODE_POINTS)
            && name.strip().equals(name) && name.indexOf('\n') < 0 && name.indexOf('\t') < 0;
    }
}
