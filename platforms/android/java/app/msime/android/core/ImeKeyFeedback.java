package app.msime.android;

import android.content.Context;
import android.media.AudioManager;
import android.os.Build;
import android.os.VibrationEffect;
import android.view.HapticFeedbackConstants;
import android.view.View;

/** 按键反馈（扩展点）：按键音与振动；现在的行为就是原来的 playFeedback。 */
final class ImeKeyFeedback {
    private final MSIMEInputService s;

    ImeKeyFeedback(MSIMEInputService s) {
        this.s = s;
    }

    /** 按键类别：普通字符键；后续按类别区分声音与振动时在这里加。 */
    static final int KEY_STANDARD = 0;

    /** 一次按键的反馈入口；现在所有类别都走 playFeedback。 */
    void onKeyDown(View key, int keyClass) {
        playFeedback(key);
    }

    void playFeedback(View source) {
        if (s.soundEnabled) {
            AudioManager audio = (AudioManager) s.getSystemService(Context.AUDIO_SERVICE);
            if (audio != null) audio.playSoundEffect(AudioManager.FX_KEYPRESS_STANDARD);
        }
        if (!s.hapticsEnabled) return;
        if (Build.VERSION.SDK_INT >= 26 && s.vibrator != null && s.vibrator.hasVibrator()) {
            s.vibrator.vibrate(VibrationEffect.createOneShot(10, s.hapticStrength.amplitude()));
        } else {
            source.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
        }
    }
}
