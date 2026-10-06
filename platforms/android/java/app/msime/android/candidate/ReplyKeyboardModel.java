package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** Platform-independent state for the dedicated thoughtful-reply keyboard. */
public final class ReplyKeyboardModel {
    public enum Mode { REPLY, POLISH }

    public record Style(String label, String emoji) {}
    public record Request(long generation, String source, String prompt, String style) {}

    @FunctionalInterface public interface Inserter { boolean insert(String text); }

    public static final List<Style> STYLES = List.of(
        new Style("专属回复", "😁"), new Style("暖心关怀", "🥰"), new Style("捧场王", "📣"),
        new Style("恋人", "😍"), new Style("幽默风趣", "🌪"), new Style("成熟稳重", "👔"),
        new Style("土味情话", "💬"), new Style("高情商", "🤩"), new Style("委婉拒绝", "🙌"));

    private String source = "";
    private Mode mode = Mode.REPLY;
    private String style = "高情商";
    private String status = "粘贴 TA 的话，再选择回复方式";
    private final List<String> replies = new ArrayList<>(3);
    private boolean busy;
    private long generation;
    private Runnable cancellation;

    public String source() { return source; }
    public Mode mode() { return mode; }
    public String style() { return style; }
    public String status() { return status; }
    public List<String> replies() { return List.copyOf(replies); }
    public boolean busy() { return busy; }

    public void setSource(String value) {
        resetResults();
        String next = value == null ? "" : value;
        if (next.codePointCount(0, next.length()) > AiPolishConfiguration.MAXIMUM_TEXT_CODE_POINTS) {
            status = "每次最多粘贴一万字";
            return;
        }
        source = next;
        status = source.isEmpty() ? "剪贴板里没有文字" : "选择下方风格生成，内容仅在点击风格时发送";
    }

    public void deleteLastCodePoint() {
        if (source.isEmpty()) return;
        int end = source.offsetByCodePoints(source.length(), -1);
        setSource(source.substring(0, end));
    }

    public void setMode(Mode value) {
        Objects.requireNonNull(value);
        if (mode == value) return;
        mode = value;
        resetResults();
        status = source.isEmpty() ? "粘贴文字，再选择润色方式" : "选择下方风格生成，内容仅在点击风格时发送";
    }

    public Request begin(String requestedStyle, List<CommunityReplyLibrary.Template> templates) {
        if (busy) return null;
        if (!AiPolishConfiguration.acceptableText(source)) {
            status = "先点粘贴，放入 TA 的话";
            return null;
        }
        String requested = requestedStyle == null || requestedStyle.isEmpty() ? style : requestedStyle;
        CommunityReplyLibrary.Template template = null;
        if (requested.startsWith("community:")) {
            String id = requested.substring("community:".length());
            for (CommunityReplyLibrary.Template item : templates) {
                if (item.id().equals(id)) { template = item; break; }
            }
            if (template == null) {
                status = "模板已移除，请重新选择";
                return null;
            }
        } else if (STYLES.stream().noneMatch(item -> item.label().equals(requested))) {
            status = "回复方式无效，请重新选择";
            return null;
        }
        if (!style.equals(requested)) replies.clear();
        style = requested;
        busy = true;
        long id = ++generation;
        String display = template == null ? requested : template.name();
        status = "正在生成 · " + display;
        String prompt;
        if (template != null) {
            prompt = (mode == Mode.POLISH ? "润色用户原文，保持原意。" : "用户内容是对方发来的话，请代拟回复。")
                + "\n" + template.prompt() + "\n只输出可直接使用的一条回复，不编造事实或承诺。";
        } else if (mode == Mode.POLISH) {
            prompt = "请以" + requested + "的语气润色用户文字，保持原意，不编造事实或承诺。"
                + "只输出一条简短自然的成稿，不加标题、解释或引号。";
        } else {
            prompt = "用户内容是对方发来的话，请代拟一条" + requested + "风格的回复。"
                + "尊重对方且有边界，不编造事实、关系或承诺。只输出一条简短自然、可以直接发送的回复，不加标题、解释或引号。";
        }
        return new Request(id, source, prompt, requested);
    }

    public void attachCancellation(long requestGeneration, Runnable action) {
        Objects.requireNonNull(action);
        if (!busy || generation != requestGeneration) action.run();
        else cancellation = action;
    }

    public boolean finish(long requestGeneration, String result) {
        if (!busy || generation != requestGeneration) return false;
        cancellation = null;
        busy = false;
        if (!AiPolishConfiguration.acceptableText(result)) {
            status = "回复为空或过长，请重试";
            return false;
        }
        if (!replies.contains(result)) {
            replies.add(0, result);
            while (replies.size() > 3) replies.remove(replies.size() - 1);
        }
        status = "点选回复插入输入框";
        return true;
    }

    public boolean fail(long requestGeneration, String message) {
        if (!busy || generation != requestGeneration) return false;
        cancellation = null;
        busy = false;
        status = message;
        return true;
    }

    public boolean use(String reply, Inserter inserter) {
        if (!replies.contains(reply) || !inserter.insert(reply)) {
            invalidateContext();
            return false;
        }
        resetResults();
        status = "已插入，请在聊天应用中确认发送";
        return true;
    }

    public void cancel() {
        resetResults();
        status = "已取消";
    }

    public void chooseStyle() {
        resetResults();
        status = "选择下方风格生成，内容仅在点击风格时发送";
    }

    public void invalidateContext() {
        invalidate("输入位置已变化，请重新选择回复方式");
    }

    public void invalidate(String message) {
        resetResults();
        status = Objects.requireNonNull(message);
    }

    public void showStatus(String message) {
        status = Objects.requireNonNull(message);
    }

    public void resetResults() {
        Runnable action = cancellation;
        cancellation = null;
        generation++;
        busy = false;
        replies.clear();
        if (action != null) action.run();
    }
}
