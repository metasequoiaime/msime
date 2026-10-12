package app.msime.android;

import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;

/** The microphone format and stop rule shared by local, HTTP, and streaming recognition. */
final class VoicePcmRecorder {
    private VoicePcmRecorder() {}

    static int minimumBufferBytes() {
        return AudioRecord.getMinBufferSize(WavAudio.SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
    }

    static AudioRecord create(int bufferBytes) {
        return new AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION,
            WavAudio.SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT, bufferBytes);
    }

    static void stopIfRecording(AudioRecord recorder) {
        try {
            if (recorder.getRecordingState() == AudioRecord.RECORDSTATE_RECORDING) {
                recorder.stop();
            }
        } catch (IllegalStateException ignored) {
            // Already stopped; release is what matters.
        }
    }
}
