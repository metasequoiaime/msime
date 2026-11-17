package app.msime.android;

import android.content.Intent;
import android.os.Bundle;
import android.os.ResultReceiver;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;

/** 翻译窗口返回时重新核对原编辑器；窗口原文使用另一条输入连接，不能比较连接对象身份。 */
final class ImeTranslationEntry {
    static final String EDITOR = "app.msime.android.translation-source";
    static final String RECEIVER = "translation_receiver";
    static final String TEXT = "translation_text";
    static final String REPLY = "translation_reply";
    static final String NO_LEARNING = "translation_no_learning";
    static final int ACCEPTED = 1, INVALID = 2;
    private final MSIMEInputService s;
    private long generation;
    private String retained = "";

    ImeTranslationEntry(MSIMEInputService service) { s = service; }

    @SuppressWarnings("deprecation")
    void open() {
        s.closeMoreTools();
        EditorInfo info = s.getCurrentInputEditorInfo();
        // 键盘正连着翻译窗口自己的原文框：窗口已在前台，再点「翻译」不能作废它持有的接收器。
        if (info != null && EDITOR.equals(info.privateImeOptions)) return;
        // 上次未能交付的译文只给窗口预览复制，新的回插目标照常按当前编辑器取。
        TranslationReturnTarget target = s.voiceInsertionReady() ? capture() : null;
        long operation = ++generation;
        // 旧窗口持有的接收器此时已作废，整栈清掉换成新实例，不能把旧窗口带回前台。
        Intent intent = new Intent().setClassName(s, "app.msime.android.TranslationActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK
                | Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS);
        if (!retained.isEmpty()) intent.putExtra(TEXT, retained);
        // 原编辑器禁止个性化学习时，窗口原文框同样禁止，在窗口里打的字不进个人词库。
        if (info != null && (!EditorPolicy.allowLearning(info.imeOptions) || EditorPolicy.password(info.inputType)))
            intent.putExtra(NO_LEARNING, true);
        if (target != null) intent.putExtra(RECEIVER, new ResultReceiver(s.main) {
            @Override protected void onReceiveResult(int code, Bundle data) {
                ResultReceiver reply = data == null ? null : data.getParcelable(REPLY);
                if (reply == null) return;
                String text = data.getString(TEXT, "");
                if (operation != generation || code != android.app.Activity.RESULT_OK
                        || text.isEmpty() || text.length() > 4096) {
                    reply.send(INVALID, Bundle.EMPTY);
                    return;
                }
                // 先作废接收器，再等待原编辑器恢复，重复消息不能另起一次插入。
                long delivery = ++generation;
                retained = text;
                // 原编辑器要等窗口关闭后才重新连上，这里只确认译文已由输入法接管；写不进去时保留在 retained，下次打开窗口可复制。
                reply.send(ACCEPTED, Bundle.EMPTY);
                deliver(delivery, target, text, 30);
            }
        });
        try { s.startActivity(intent); retained = ""; }
        catch (RuntimeException unavailable) { s.notice("翻译窗口无法打开"); }
    }

    private void deliver(long operation, TranslationReturnTarget target, String text, int remaining) {
        if (operation != generation) return;
        TranslationReturnTarget current = s.voiceInsertionReady() ? capture() : null;
        if (target.same(current)) {
            generation++;
            if (s.commitText(text, TypingSource.LOCAL)) retained = "";
            else s.notice("编辑器拒绝插入，译文保留在翻译窗口");
            return;
        }
        if (remaining > 0) s.main.postDelayed(() -> deliver(operation, target, text, remaining - 1), 100);
        else {
            generation++;
            s.notice("输入位置已变化，译文保留在翻译窗口，可复制后粘贴");
        }
    }

    private TranslationReturnTarget capture() {
        EditorInfo info = s.getCurrentInputEditorInfo();
        if (info == null || info.packageName == null || s.connection == null
                || EDITOR.equals(info.privateImeOptions)) return null;
        try {
            ExtractedTextRequest request = new ExtractedTextRequest();
            request.hintMaxChars = 4096;
            request.hintMaxLines = 100;
            ExtractedText extracted = s.connection.getExtractedText(request, 0);
            if (extracted == null || extracted.text == null || extracted.startOffset < 0
                    || extracted.selectionStart < 0 || extracted.selectionEnd < 0
                    || extracted.selectionStart > extracted.text.length() || extracted.selectionEnd > extracted.text.length()) return null;
            String before = s.editorContext(true);
            String after = s.editorContext(false);
            if (before == null || after == null) return null;
            return new TranslationReturnTarget(info.packageName, info.fieldId, info.inputType,
                extracted.startOffset + extracted.selectionStart, extracted.startOffset + extracted.selectionEnd,
                before, s.selectedEditorText(), after);
        } catch (RuntimeException unavailable) { return null; }
    }


    void destroy() { generation++; retained = ""; }
}
