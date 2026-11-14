package app.msime.android;

import android.content.Intent;
import android.os.Bundle;
import android.os.ResultReceiver;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;

/** 翻译窗口返回时重新核对原编辑器；窗口原文使用另一条输入连接，不能比较连接对象身份。 */
final class ImeTranslationEntry {
    private final MSIMEInputService s;
    private long generation;
    private String retained = "";

    ImeTranslationEntry(MSIMEInputService service) { s = service; }

    void open() {
        s.closeMoreTools();
        TranslationReturnTarget target = retained.isEmpty() && s.voiceInsertionReady() ? capture() : null;
        long operation = ++generation;
        Intent intent = new Intent(s, TranslationActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS);
        if (!retained.isEmpty()) intent.putExtra(TranslationActivity.TEXT, retained);
        if (target != null) intent.putExtra(TranslationActivity.RECEIVER, new ResultReceiver(s.main) {
            @Override protected void onReceiveResult(int code, Bundle data) {
                if (operation != generation || code != android.app.Activity.RESULT_OK || data == null) return;
                String text = data.getString(TranslationActivity.TEXT, "");
                if (text.isEmpty() || text.length() > 4096) return;
                // 先作废接收器，再等待原编辑器恢复，重复消息不能另起一次插入。
                long delivery = ++generation;
                retained = text;
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
                || TranslationActivity.EDITOR.equals(info.privateImeOptions)) return null;
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
