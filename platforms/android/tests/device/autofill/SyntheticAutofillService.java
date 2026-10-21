package app.msime.android.test;

import android.app.PendingIntent;
import android.app.assist.AssistStructure;
import android.app.slice.Slice;
import android.app.slice.SliceSpec;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.CancellationSignal;
import android.service.autofill.AutofillService;
import android.service.autofill.Dataset;
import android.service.autofill.FillCallback;
import android.service.autofill.FillRequest;
import android.service.autofill.FillResponse;
import android.service.autofill.InlinePresentation;
import android.service.autofill.SaveCallback;
import android.service.autofill.SaveRequest;
import android.view.View;
import android.view.autofill.AutofillId;
import android.view.autofill.AutofillValue;
import android.view.inputmethod.InlineSuggestionsRequest;
import android.widget.RemoteViews;
import java.util.List;

/** 合成外部提供方：同时提供旧菜单和内联呈现，由系统执行多字段填充。 */
@SuppressWarnings("deprecation")
public final class SyntheticAutofillService extends AutofillService {
    static final String USERNAME = "fixture@example.invalid";
    static final String PASSWORD = "Synthetic-Only-42";
    static final String LABEL = "Synthetic login";
    static volatile int requests;
    static volatile int inlineRequests;
    static volatile boolean empty;

    @Override public void onFillRequest(FillRequest request, CancellationSignal cancellation, FillCallback callback) {
        InlineSuggestionsRequest inline = Build.VERSION.SDK_INT >= 30 ? request.getInlineSuggestionsRequest() : null;
        if (inline != null) inlineRequests++;
        requests++;
        if (empty) { callback.onSuccess(null); return; }
        AssistStructure structure = request.getFillContexts().get(request.getFillContexts().size() - 1).getStructure();
        AutofillId username = null;
        AutofillId password = null;
        for (int window = 0; window < structure.getWindowNodeCount(); window++) {
            AssistStructure.ViewNode root = structure.getWindowNodeAt(window).getRootViewNode();
            if (username == null) username = find(root, View.AUTOFILL_HINT_USERNAME);
            if (password == null) password = find(root, View.AUTOFILL_HINT_PASSWORD);
        }
        if (username == null || password == null) { callback.onSuccess(null); return; }
        RemoteViews menu = new RemoteViews("android", android.R.layout.simple_list_item_1);
        menu.setTextViewText(android.R.id.text1, LABEL);
        Dataset.Builder dataset = new Dataset.Builder(menu);
        if (Build.VERSION.SDK_INT >= 30 && inline != null) {
            InlinePresentation presentation = presentation(inline);
            dataset.setValue(username, AutofillValue.forText(USERNAME), menu, presentation);
            dataset.setValue(password, AutofillValue.forText(PASSWORD), menu, presentation);
        } else {
            dataset.setValue(username, AutofillValue.forText(USERNAME));
            dataset.setValue(password, AutofillValue.forText(PASSWORD));
        }
        callback.onSuccess(new FillResponse.Builder().addDataset(dataset.build()).build());
    }

    @android.annotation.TargetApi(30)
    private InlinePresentation presentation(InlineSuggestionsRequest request) {
        PendingIntent attribution = PendingIntent.getActivity(this, 0,
            new Intent(this, InlineAutofillEditorActivity.class), PendingIntent.FLAG_IMMUTABLE);
        // 合成提供方按公开的 AndroidX V1 Slice 协议发送载荷；产品仍使用官方库生成样式。
        Slice.Builder slice = new Slice.Builder(Uri.parse("inline.slice"),
            new SliceSpec("androidx.autofill.inline.ui.version:v1", 1));
        slice.addText(LABEL, null, List.of("inline_title"));
        slice.addAction(attribution, new Slice.Builder(slice)
            .addHints(List.of("inline_attribution")).build(), null);
        return new InlinePresentation(slice.build(), request.getInlinePresentationSpecs().get(0), false);
    }

    private static AutofillId find(AssistStructure.ViewNode node, String hint) {
        String[] hints = node.getAutofillHints();
        if (hints != null) for (String candidate : hints) if (hint.equals(candidate)) return node.getAutofillId();
        for (int child = 0; child < node.getChildCount(); child++) {
            AutofillId id = find(node.getChildAt(child), hint);
            if (id != null) return id;
        }
        return null;
    }

    @Override public void onSaveRequest(SaveRequest request, SaveCallback callback) { callback.onSuccess(); }
}
