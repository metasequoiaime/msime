package app.msime.android;

/**
 * Wraps recorded PCM in the WAV container the transcription APIs accept.
 *
 * <p>The recorder produces signed 16-bit little-endian mono samples, which is what every one of
 * these providers documents as its cheapest accepted input; the container is 44 bytes of header in
 * front of them. Doing it here rather than with a platform encoder keeps the bytes exactly as
 * captured — a lossy re-encode is not worth it for a few seconds of speech, and each provider's
 * accepted format list is shorter than its documentation suggests.
 */
public final class WavAudio {
    /** The sample rate every provider accepts and the recorder is configured for. */
    public static final int SAMPLE_RATE = 16_000;
    public static final int HEADER_BYTES = 44;

    private WavAudio() {}

    /** A WAV file for these samples, or null when there is nothing to send. */
    public static byte[] wrap(byte[] pcm, int length, int sampleRate) {
        if (pcm == null || length <= 0 || length > pcm.length || sampleRate <= 0) return null;
        // An odd byte count cannot be whole 16-bit samples; truncating is better than declaring a
        // size the data does not have, which some decoders read past.
        int samples = length - (length % 2);
        if (samples <= 0) return null;
        byte[] out = new byte[HEADER_BYTES + samples];
        int chunkSize = 36 + samples;
        int byteRate = sampleRate * 2;
        ascii(out, 0, "RIFF");
        littleEndian32(out, 4, chunkSize);
        ascii(out, 8, "WAVE");
        ascii(out, 12, "fmt ");
        littleEndian32(out, 16, 16);          // PCM subchunk size
        littleEndian16(out, 20, 1);           // format: PCM
        littleEndian16(out, 22, 1);           // channels: mono
        littleEndian32(out, 24, sampleRate);
        littleEndian32(out, 28, byteRate);
        littleEndian16(out, 32, 2);           // block align: one 16-bit sample
        littleEndian16(out, 34, 16);          // bits per sample
        ascii(out, 36, "data");
        littleEndian32(out, 40, samples);
        System.arraycopy(pcm, 0, out, HEADER_BYTES, samples);
        return out;
    }

    private static void ascii(byte[] out, int offset, String value) {
        for (int index = 0; index < value.length(); index++) {
            out[offset + index] = (byte) value.charAt(index);
        }
    }

    private static void littleEndian32(byte[] out, int offset, int value) {
        out[offset] = (byte) (value & 0xff);
        out[offset + 1] = (byte) (value >>> 8 & 0xff);
        out[offset + 2] = (byte) (value >>> 16 & 0xff);
        out[offset + 3] = (byte) (value >>> 24 & 0xff);
    }

    private static void littleEndian16(byte[] out, int offset, int value) {
        out[offset] = (byte) (value & 0xff);
        out[offset + 1] = (byte) (value >>> 8 & 0xff);
    }
}
