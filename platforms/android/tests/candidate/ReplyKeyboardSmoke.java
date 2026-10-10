import app.msime.android.CommunityReplyLibrary;
import app.msime.android.ReplyKeyboardModel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

public final class ReplyKeyboardSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] arguments) throws Exception {
        check(ReplyKeyboardModel.STYLES.stream().map(ReplyKeyboardModel.Style::label).toList().equals(List.of(
            "专属回复", "暖心关怀", "捧场王", "恋人", "幽默风趣", "成熟稳重", "土味情话", "高情商", "委婉拒绝")));
        ReplyKeyboardModel model = new ReplyKeyboardModel();
        model.setSource("synthetic source");
        ReplyKeyboardModel.Request first = model.begin("高情商", List.of());
        check(first.prompt().contains("尊重对方且有边界") && first.prompt().contains("不编造事实、关系或承诺"));
        AtomicBoolean cancelled = new AtomicBoolean();
        model.attachCancellation(first.generation(), () -> cancelled.set(true));
        model.setSource("");
        check(cancelled.get() && !model.busy() && model.replies().isEmpty());
        check(!model.finish(first.generation(), "late result"));

        model.setSource("synthetic source");
        ReplyKeyboardModel.Request request = model.begin("高情商", List.of());
        check(model.finish(request.generation(), "one"));
        check(model.replies().equals(List.of("one")));
        AtomicInteger inserts = new AtomicInteger();
        check(inserts.get() == 0);
        model.invalidateContext();
        check(!model.use("one", text -> { inserts.incrementAndGet(); return true; }));
        check(inserts.get() == 0);

        for (String reply : List.of("one", "two", "three", "four")) {
            request = model.begin("高情商", List.of());
            check(model.finish(request.generation(), reply));
        }
        check(model.replies().equals(List.of("four", "three", "two")));
        request = model.begin("高情商", List.of());
        check(model.finish(request.generation(), "four"));
        check(model.replies().equals(List.of("four", "three", "two")));
        check(model.use("three", text -> { inserts.incrementAndGet(); return true; }));
        check(inserts.get() == 1 && model.replies().isEmpty());

        model.setSource("synthetic source");
        model.setMode(ReplyKeyboardModel.Mode.POLISH);
        CommunityReplyLibrary.Template template = new CommunityReplyLibrary.Template(
            "fixture", "fixture style", "use three short sentences");
        request = model.begin("community:fixture", List.of(template));
        check(request.prompt().contains("use three short sentences") && request.prompt().contains("保持原意"));
        model.cancel();
        check(model.begin("community:fixture", List.of()) == null);
        check(model.status().contains("模板已移除"));

        model.setSource("𠮷a");
        model.deleteLastCodePoint();
        check(model.source().equals("𠮷"));
        model.deleteLastCodePoint();
        check(model.source().isEmpty());
        model.setSource("a".repeat(10_001));
        check(model.source().isEmpty() && model.status().contains("一万字"));

        Path directory = Files.createTempDirectory("msime-community-test");
        Path outside = null;
        try {
            Path file = directory.resolve("CommunityLibrary.json");
            Files.writeString(file, "[{\"id\":\"dictionary\",\"kind\":\"dictionary\",\"name\":\"ignored\",\"content\":{}},"
                + "{\"id\":\"fixture\",\"kind\":\"reply\",\"name\":\"fixture style\","
                + "\"content\":{\"prompt\":\"use \\\"kind\\\" words\"}}]");
            List<CommunityReplyLibrary.Template> templates = CommunityReplyLibrary.read(file);
            check(templates.size() == 1 && templates.get(0).id().equals("fixture"));
            Files.writeString(file, "[{\"id\":\"multiline\",\"kind\":\"reply\",\"name\":\"多行模板\","
                + "\"content\":{\"prompt\":\"第一条\\n第二条\\t带缩进\"}}]");
            templates = CommunityReplyLibrary.read(file);
            check(templates.size() == 1 && templates.get(0).prompt().equals("第一条\n第二条\t带缩进"));
            Files.writeString(file, "[]".repeat(CommunityReplyLibrary.MAXIMUM_BYTES));
            try { CommunityReplyLibrary.read(file); throw new AssertionError(); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("large")); }
            Files.writeString(file, "[" + "{\"id\":\"x\",\"kind\":\"reply\",\"name\":\"x\",\"content\":{\"prompt\":\"x\"}},".repeat(50)
                + "{\"id\":\"last\",\"kind\":\"reply\",\"name\":\"last\",\"content\":{\"prompt\":\"last\"}}]");
            try { CommunityReplyLibrary.read(file); throw new AssertionError(); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("Invalid")); }
            Files.writeString(file, "[{\"id\":\"long\",\"kind\":\"reply\",\"name\":\"x\",\"content\":{\"prompt\":\""
                + "x".repeat(2_001) + "\"}}]");
            try { CommunityReplyLibrary.read(file); throw new AssertionError(); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("Invalid")); }
            Files.writeString(file, "[{\"id\":\"same\",\"kind\":\"reply\",\"name\":\"x\",\"content\":{\"prompt\":\"x\"}},"
                + "{\"id\":\"same\",\"kind\":\"reply\",\"name\":\"y\",\"content\":{\"prompt\":\"y\"}}]");
            try { CommunityReplyLibrary.read(file); throw new AssertionError(); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("Invalid")); }

            outside = Files.createTempDirectory("msime-community-outside");
            Path linkedParent = directory.resolve("linked-parent");
            Files.createSymbolicLink(linkedParent, outside);
            Files.writeString(outside.resolve("CommunityLibrary.json"), "[]");
            try { CommunityReplyLibrary.read(linkedParent.resolve("CommunityLibrary.json")); throw new AssertionError(); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("symbolic link")); }
            Path hardlinkOutside = Files.createTempFile("msime-community-hardlink", ".json");
            Files.writeString(hardlinkOutside,
                "[{\"id\":\"fixture\",\"kind\":\"reply\",\"name\":\"fixture style\",\"content\":{\"prompt\":\"x\"}}]");
            Files.delete(file);
            Files.createLink(file, hardlinkOutside);
            try { CommunityReplyLibrary.read(file); throw new AssertionError("hard-linked library accepted"); }
            catch (java.io.IOException expected) { check(expected.getMessage().contains("single link")); }
            Files.deleteIfExists(hardlinkOutside);
        } finally {
            Files.walk(directory).sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                try { Files.delete(path); } catch (java.io.IOException error) { throw new RuntimeException(error); }
            });
            if (outside != null) {
                Files.walk(outside).sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.delete(path); } catch (java.io.IOException error) { throw new RuntimeException(error); }
                });
            }
        }
        System.out.println("Android thoughtful reply: styles, prompts, cancellation, stale guards, insertion, dedupe and community bounds passed");
    }
}
