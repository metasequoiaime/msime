import app.msime.android.ClipboardImageCache;
import app.msime.android.ClipboardImagePolicy;
import app.msime.android.StickerPackModel;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** 水杉娘表情包：清单条目的边界校验、底栏分类顺序与默认分类、网格列数、缩略图尺寸、文案、直接发图的判断、自己写进剪贴板的贴纸的识别，以及发送目录只在写下一张时替换。 */
public final class StickerPackModelSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void rejects(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError(message);
    }

    public static void main(String[] args) throws IOException {
        // 文件名只认两位数字加 .webp，路径穿越、别的扩展名和非字符串一律拒绝。
        StickerPackModel.Sticker happy = StickerPackModel.sticker("01.webp", "开心");
        check("01.webp".equals(happy.file()) && "开心".equals(happy.name()), "a plain entry is kept as is");
        check("stickers/shuishan/01.webp".equals(StickerPackModel.assetPath(happy)), "asset path under the packaged directory");
        for (String file : new String[] {"1.webp", "001.webp", "01.png", "01.WEBP", "../01.webp", "01.webp/",
                "a1.webp", "01.webp\n", "", "／01.webp"}) {
            rejects(() -> StickerPackModel.sticker(file, "开心"), "unsafe file name accepted: " + file);
        }
        rejects(() -> StickerPackModel.sticker(null, "开心"), "missing file accepted");
        rejects(() -> StickerPackModel.sticker(1, "开心"), "non-string file accepted");

        // 名字非空、不带首尾空白和换行、控制字符，长度按码点有界。
        check(StickerPackModel.sticker("02.webp", "打call").name().equals("打call"), "mixed-script name");
        check(StickerPackModel.sticker("03.webp", "一二三四五六七八九十一二三四五六").name().codePointCount(0, 16) == 16,
            "a sixteen code point name fits");
        for (String name : new String[] {"", "  ", " 开心", "开心 ", "开\n心", "开\t心", "开\u0007心",
                "一二三四五六七八九十一二三四五六七"}) {
            rejects(() -> StickerPackModel.sticker("04.webp", name), "invalid name accepted: " + name);
        }
        rejects(() -> StickerPackModel.sticker("04.webp", null), "missing name accepted");
        rejects(() -> StickerPackModel.sticker("04.webp", 4), "non-string name accepted");

        // 整个表情包：顺序保留、复制后不可改，空表、超量、重复文件和坏名字整份拒绝。
        List<StickerPackModel.Sticker> stickers = new ArrayList<>(List.of(happy, StickerPackModel.sticker("02.webp", "大笑")));
        StickerPackModel.Pack pack = StickerPackModel.pack("水杉娘", stickers);
        stickers.clear();
        check(pack.stickers().size() == 2 && "02.webp".equals(pack.stickers().get(1).file()),
            "manifest order is display order and the list is copied");
        boolean immutable = false;
        try {
            pack.stickers().add(happy);
        } catch (UnsupportedOperationException expected) {
            immutable = true;
        }
        check(immutable, "pack list is immutable");
        rejects(() -> StickerPackModel.pack("水杉娘", List.of()), "empty pack accepted");
        rejects(() -> StickerPackModel.pack("水杉娘", List.of(happy, StickerPackModel.sticker("01.webp", "又一张"))),
            "duplicate file accepted");
        rejects(() -> StickerPackModel.pack(null, List.of(happy)), "missing pack name accepted");
        rejects(() -> StickerPackModel.pack(" ", List.of(happy)), "blank pack name accepted");
        ArrayList<StickerPackModel.Sticker> many = new ArrayList<>();
        for (int index = 0; index <= StickerPackModel.MAX_STICKERS; index++)
            many.add(new StickerPackModel.Sticker(String.format("%02d.webp", index % 100), "第" + index));
        rejects(() -> StickerPackModel.pack("水杉娘", many), "oversized pack accepted");
        check(StickerPackModel.MAX_STICKERS >= 66, "the shipped 66 stickers fit the bound");

        // 竖屏手机五列，更宽时每约 72 dp 加一列，至少五列。
        check(StickerPackModel.columns(0) == 5, "no width still gives five columns");
        check(StickerPackModel.columns(320) == 5 && StickerPackModel.columns(360) == 5
            && StickerPackModel.columns(412) == 5, "portrait phones show five columns");
        check(StickerPackModel.columns(432) == 6, "a wider panel adds a column per 72 dp");
        check(StickerPackModel.columns(704) == 9 && StickerPackModel.columns(800) == 11, "landscape and tablets add columns");

        // 底栏分类顺序：表情包排第一（在「最近」之前），默认选中它；横划按同一顺序走，两端不回绕。
        int emojiCategories = 10;
        int stickerCategory = StickerPackModel.category(emojiCategories);
        int recents = StickerPackModel.RECENTS_CATEGORY;
        int none = StickerPackModel.NO_CATEGORY;
        check(stickerCategory == emojiCategories && stickerCategory >= 0,
            "the sticker category id follows the emoji categories");
        check(List.of(StickerPackModel.category(3), recents, 0, 1, 2).equals(StickerPackModel.categoryOrder(true, true, 3)),
            "stickers lead, then recents and the emoji categories");
        check(List.of(StickerPackModel.category(3), 0, 1, 2).equals(StickerPackModel.categoryOrder(true, false, 3)),
            "without recents the stickers are followed by the first emoji category");
        check(List.of(recents, 0, 1, 2).equals(StickerPackModel.categoryOrder(false, true, 3))
            && List.of(0, 1, 2).equals(StickerPackModel.categoryOrder(false, false, 3)),
            "without a sticker manifest the original order stays");
        check(StickerPackModel.defaultCategory(true, true, emojiCategories) == stickerCategory
            && StickerPackModel.defaultCategory(true, false, emojiCategories) == stickerCategory,
            "the sticker pack is selected when the panel opens");
        check(StickerPackModel.defaultCategory(false, true, emojiCategories) == recents
            && StickerPackModel.defaultCategory(false, false, emojiCategories) == 0,
            "without stickers the panel opens on recents, or the first emoji category");
        check(StickerPackModel.swipeCategory(stickerCategory, 1, true, true, emojiCategories) == recents
            && StickerPackModel.swipeCategory(recents, 1, true, true, emojiCategories) == 0
            && StickerPackModel.swipeCategory(0, -1, true, true, emojiCategories) == recents
            && StickerPackModel.swipeCategory(recents, -1, true, true, emojiCategories) == stickerCategory,
            "swiping walks stickers, recents and the emoji categories in tab order");
        check(StickerPackModel.swipeCategory(stickerCategory, 1, true, false, emojiCategories) == 0
            && StickerPackModel.swipeCategory(0, -1, true, false, emojiCategories) == stickerCategory,
            "without recents the stickers sit next to the first emoji category");
        check(StickerPackModel.swipeCategory(stickerCategory, -1, true, true, emojiCategories) == none
            && StickerPackModel.swipeCategory(emojiCategories - 1, 1, true, true, emojiCategories) == none,
            "swiping does not wrap at either end");
        check(StickerPackModel.swipeCategory(0, -1, false, false, emojiCategories) == none
            && StickerPackModel.swipeCategory(0, -1, false, true, emojiCategories) == recents
            && StickerPackModel.swipeCategory(emojiCategories - 1, 1, false, true, emojiCategories) == none,
            "without stickers the original swipe bounds stay");
        check(StickerPackModel.swipeCategory(recents, 1, true, false, emojiCategories) == none
            && StickerPackModel.swipeCategory(stickerCategory, 1, false, true, emojiCategories) == none
            && StickerPackModel.swipeCategory(none, 1, true, true, emojiCategories) == none,
            "a category missing from the tab bar does not swipe");
        check(StickerPackModel.thumbnailEdge(270, 12) == 246, "cell minus padding on both sides");
        check(StickerPackModel.thumbnailEdge(10, 12) == 1 && StickerPackModel.thumbnailEdge(10, -3) == 10,
            "thumbnail edge stays positive and ignores negative padding");

        // 缩略图等比缩到格子里，不放大，源图尺寸越界拒绝。
        int[] square = StickerPackModel.fit(320, 320, 240);
        check(square[0] == 240 && square[1] == 240, "a square source scales to the cell");
        int[] wide = StickerPackModel.fit(320, 160, 160);
        check(wide[0] == 160 && wide[1] == 80, "aspect ratio is kept");
        int[] small = StickerPackModel.fit(100, 100, 240);
        check(small[0] == 100 && small[1] == 100, "small sources are not upscaled");
        int[] full = StickerPackModel.fit(320, 320, StickerPackModel.MAX_SOURCE_EDGE);
        check(full[0] == 320 && full[1] == 320, "the send path decodes at source size");
        int[] tiny = StickerPackModel.fit(1000, 1, 10);
        check(tiny[0] == 10 && tiny[1] == 1, "extreme aspect ratios keep at least one pixel");
        rejects(() -> StickerPackModel.fit(0, 320, 100), "empty source accepted");
        rejects(() -> StickerPackModel.fit(320, 2048, 100), "oversized source accepted");
        rejects(() -> StickerPackModel.fit(320, 320, 0), "empty target accepted");

        // 无障碍和提示文案。
        check("水杉娘 开心".equals(StickerPackModel.description(pack, happy)), "cell description names pack and meaning");
        check("已复制水杉娘「开心」，长按输入框粘贴发送".equals(StickerPackModel.copiedNotice(pack, happy)),
            "clipboard fallback notice");
        check(StickerPackModel.gridDescription(5).startsWith("表情网格；")
            && StickerPackModel.gridDescription(5).contains("5"), "grid description shares the emoji grid prefix");
        check("水杉娘 · 2 张表情".equals(StickerPackModel.status(pack)), "status counts stickers");
        check("水杉娘表情包".equals(StickerPackModel.TAB_DESCRIPTION), "tab description");

        // 输入框声明收 PNG 才直接发图，否则走剪贴板。
        check(StickerPackModel.commitsContent(new String[] {"image/png"}), "PNG editors take stickers");
        check(StickerPackModel.commitsContent(new String[] {"image/*"}), "image/* editors take stickers");
        check(StickerPackModel.commitsContent(new String[] {"*/*"}), "*/* editors take stickers");
        check(!StickerPackModel.commitsContent(new String[] {"image/gif"}), "GIF-only editors fall back to the clipboard");
        check(!StickerPackModel.commitsContent(new String[0]) && !StickerPackModel.commitsContent(null),
            "editors without content types fall back to the clipboard");
        // 微信的聊天输入框不声明图片类型，只在 extras 里写 1，走它的私有表情命令。
        check(StickerPackModel.sendsExpression(null, 1), "WeChat-style editors take the private expression command");
        check(StickerPackModel.sendsExpression(new String[] {"image/gif"}, 1), "a GIF-only editor that also opts in takes the command");
        check(!StickerPackModel.sendsExpression(new String[] {"image/png"}, 1), "PNG editors keep commitContent");
        check(!StickerPackModel.sendsExpression(null, 0) && !StickerPackModel.sendsExpression(null, 2),
            "editors that do not write 1 fall back to the clipboard");
        check("SUPPORT_SOGOU_EXPRESSION".equals(StickerPackModel.EXPRESSION_EXTRA)
            && "com.sogou.inputmethod.exp.commit".equals(StickerPackModel.EXPRESSION_COMMAND)
            && "EXP_PATH_URI".equals(StickerPackModel.EXPRESSION_URI), "the names match what WeChat reads");
        check(StickerPackModel.EXTENSION.equals(ClipboardImagePolicy.extension(StickerPackModel.MIME_TYPE)),
            "the file extension maps back to the declared type");
        check("sticker-images".equals(StickerPackModel.DIRECTORY),
            "the directory name matches the FileProvider path in res/xml/msime_file_paths.xml");
        check(!StickerPackModel.DIRECTORY.equals(ClipboardImagePolicy.DIRECTORY),
            "stickers do not share the recent-clip copy directory");

        // 自己放进剪贴板的贴纸：本应用（含各版本后缀）FileProvider 下 sticker-images 里的文件。
        check(StickerPackModel.ownClipUri("content", "app.msime.android.files", "/sticker-images/clip-1.png",
            "app.msime.android"), "own sticker URI recognized");
        check(StickerPackModel.ownClipUri("content", "app.msime.android.pinyin.files", "/sticker-images/clip-1.png",
            "app.msime.android.pinyin"), "edition authorities are recognized");
        check(!StickerPackModel.ownClipUri("content", "app.msime.android.files", "/clipboard-images/clip-1.png",
            "app.msime.android"), "recent-clip copies are not stickers");
        check(!StickerPackModel.ownClipUri("content", "com.example.files", "/sticker-images/clip-1.png",
            "app.msime.android"), "another app's provider is the user's copy");
        check(!StickerPackModel.ownClipUri("content", "app.msime.android.files", "/sticker-images-other/x.png",
            "app.msime.android"), "prefix lookalikes are not stickers");
        check(!StickerPackModel.ownClipUri("file", "app.msime.android.files", "/sticker-images/clip-1.png",
            "app.msime.android"), "only content URIs");
        check(!StickerPackModel.ownClipUri("content", ".files", "/sticker-images/clip-1.png", "")
            && !StickerPackModel.ownClipUri("content", null, null, null), "missing parts never match");
        // 发送目录始终至多一张：写下一张时替换旧的，写完的那张一直留着（不定时删，复制到剪贴板的 URI 要在键盘进程被回收后仍能粘贴）。
        Path root = Files.createTempDirectory("msime-sticker-smoke");
        File directory = root.resolve(StickerPackModel.DIRECTORY).toFile();
        try {
            byte[] first = new byte[] {(byte) 0x89, 'P', 'N', 'G', 1};
            File sent = ClipboardImageCache.replace(new ByteArrayInputStream(first), directory,
                StickerPackModel.EXTENSION, ClipboardImagePolicy.MAX_BYTES);
            File[] kept = directory.listFiles();
            check(kept != null && kept.length == 1 && kept[0].equals(sent) && sent.getName().endsWith(".png")
                && Arrays.equals(first, Files.readAllBytes(sent.toPath())), "the sent sticker stays as the one PNG");
            byte[] second = new byte[] {(byte) 0x89, 'P', 'N', 'G', 2};
            File next = ClipboardImageCache.replace(new ByteArrayInputStream(second), directory,
                StickerPackModel.EXTENSION, ClipboardImagePolicy.MAX_BYTES);
            kept = directory.listFiles();
            check(kept != null && kept.length == 1 && kept[0].equals(next) && !sent.exists()
                && Arrays.equals(second, Files.readAllBytes(next.toPath())), "the next sticker replaces the previous one");
        } finally {
            ClipboardImageCache.clear(directory);
            Files.deleteIfExists(directory.toPath());
            Files.deleteIfExists(root);
        }
        System.out.println("StickerPackModelSmoke passed");
    }
}
