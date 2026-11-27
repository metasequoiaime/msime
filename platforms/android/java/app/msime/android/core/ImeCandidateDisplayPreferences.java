package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONObject;

/** 候选区的显示选择跟随实时偏好；旧启动副本不能让已关闭的行重新出现。 */
final class ImeCandidateDisplayPreferences {
    private ImeCandidateDisplayPreferences() {}

    static void apply(MSIMEInputService service, JSONObject preferences,
            ClipboardHistoryRetentionPolicy.Source source) {
        if (source != ClipboardHistoryRetentionPolicy.Source.LIVE) return;
        service.applyCandidateGlossPreference(preferences);
        service.spellingPosition = CandidatePreeditStylePolicy.normalize(
            InputViewValuePolicy.textOr(preferences, "touch_spelling_position", null));
        service.spellingEditorEnabled = preferences == null
            || preferences.optBoolean("touch_spelling_editor", true);
        service.candidateCloudBadge = preferences == null
            || preferences.optBoolean(CandidateSourceMark.CLOUD_PREFERENCE, true);
        service.candidateAiBadgeStyle = CandidateSourceMark.aiStyle(preferences == null ? null
            : InputViewValuePolicy.textOr(preferences, CandidateSourceMark.AI_STYLE_PREFERENCE, null));
    }

    /** 老缓存没有这些字段时沿用字段初值，不把缺字段当成新的用户选择。 */
    static void restoreHint(MSIMEInputService service, JSONObject hint) {
        if (hint.has("candidate_english_gloss") || hint.has("candidate_pronunciation"))
            service.applyCandidateGlossPreference(hint);
        if (hint.has("touch_spelling_position"))
            service.spellingPosition = CandidatePreeditStylePolicy.normalize(
                InputViewValuePolicy.textOr(hint, "touch_spelling_position", null));
        if (hint.has("touch_spelling_editor"))
            service.spellingEditorEnabled = hint.optBoolean("touch_spelling_editor", true);
        if (hint.has(CandidateSourceMark.CLOUD_PREFERENCE))
            service.candidateCloudBadge = hint.optBoolean(CandidateSourceMark.CLOUD_PREFERENCE, true);
        if (hint.has(CandidateSourceMark.AI_STYLE_PREFERENCE))
            service.candidateAiBadgeStyle = CandidateSourceMark.aiStyle(
                InputViewValuePolicy.textOr(hint, CandidateSourceMark.AI_STYLE_PREFERENCE, null));
    }
}
