import android.view.KeyEvent;
import app.msime.android.KeyPressBatch;
import app.msime.android.KeyPressIds;
import java.util.HashSet;
import java.util.List;
import java.util.Map;

/**
 * The key ids this host sends to the shared statistics store, and how it batches them.
 *
 * <p>The store rejects a whole batch carrying one id outside its whitelist, so every id this host can produce has to be on it, and a key without a mapping must produce nothing rather than a guess.
 */
public final class KeyPressCountingSmoke {
    public static void main(String[] args) {
        ids();
        mapping();
        batching();
        layout();
        System.out.println("Android key press counting: ids, mapping, batching and layout passed");
    }

    private static void ids() {
        // 与 crates/client-core/src/typing_statistics.rs 的 KEY_IDS 是同样的 141 个 id。
        check(KeyPressIds.KEY_IDS.size() == 141, "the whitelist has the store's 141 ids");
        check(new HashSet<>(KeyPressIds.KEY_IDS).size() == 141, "the whitelist has no duplicate");
        // 14 键的十四个键按键面字母命名，与 Rust 的 KEY_IDS 末尾逐字相同。
        check(KeyPressIds.KEY_IDS.subList(127, 141).equals(List.of("FourteenQW", "FourteenER", "FourteenTY",
            "FourteenUI", "FourteenOP", "FourteenAS", "FourteenDF", "FourteenGH", "FourteenJK", "FourteenL",
            "FourteenZX", "FourteenCV", "FourteenBN", "FourteenM")), "the fourteen-key ids close the whitelist");
        check(KeyPressIds.isKnown("Nine0") && KeyPressIds.isKnown("SoftVoice"), "soft ids are known");
        check(!KeyPressIds.isKnown("KeyAA") && !KeyPressIds.isKnown(null) && !KeyPressIds.isKnown(""),
            "anything else is not");
    }

    private static void mapping() {
        check("KeyQ".equals(KeyPressIds.forCharacter('q')) && "KeyQ".equals(KeyPressIds.forCharacter('Q')),
            "a letter is its key in either case");
        check("Digit7".equals(KeyPressIds.forCharacter('7')), "a digit is its digit-row key");
        check("Digit1".equals(KeyPressIds.forCharacter('!')), "a shifted mark is the key that carries it");
        check("Comma".equals(KeyPressIds.forCharacter(',')) && "Slash".equals(KeyPressIds.forCharacter('?'))
            && "Semicolon".equals(KeyPressIds.forCharacter(';')) && "Quote".equals(KeyPressIds.forCharacter('"'))
            && "Minus".equals(KeyPressIds.forCharacter('_')) && "Backslash".equals(KeyPressIds.forCharacter('\\')),
            "symbol-layer marks map to the ANSI keys");
        check(KeyPressIds.forCharacter('，') == null && KeyPressIds.forCharacter('é') == null,
            "a character no ANSI key types is not counted");
        // Every printable ASCII character that maps at all maps to a whitelisted id.
        for (char character = 32; character < 127; character++) {
            String id = KeyPressIds.forCharacter(character);
            check(id != null && KeyPressIds.isKnown(id), "printable ASCII maps into the whitelist: " + character);
        }

        check("Nine1".equals(KeyPressIds.forNineKeyDigit(1)) && "Nine0".equals(KeyPressIds.forNineKeyDigit(0)),
            "nine-key cells are named by their digit");
        check(KeyPressIds.forNineKeyDigit(10) == null, "there is no tenth cell");
        check("Nine1".equals(KeyPressIds.forJapaneseKeyIndex(0)) && "Nine9".equals(KeyPressIds.forJapaneseKeyIndex(8)),
            "the kana grid's first nine keys are cells 1 to 9");
        check("Nine0".equals(KeyPressIds.forJapaneseKeyIndex(9)), "わ sits where a keypad has 0");
        check("SoftPunctuation".equals(KeyPressIds.forJapaneseKeyIndex(10)), "the 、。？！ key is punctuation");
        check(KeyPressIds.forJapaneseKeyIndex(11) == null, "there is no twelfth kana key");

        check("KeyA".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_A))
            && "KeyZ".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_Z)), "hardware letters");
        check("Digit0".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_0))
            && "Digit9".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_9)), "hardware digits");
        check("F12".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_F12)), "function keys");
        check("Numpad5".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_NUMPAD_5)), "numpad digits");
        check("Backspace".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_DEL))
            && "Delete".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_FORWARD_DEL)),
            "Android's DEL is backspace and FORWARD_DEL is delete");
        check("ControlLeft".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_CTRL_LEFT))
            && "ArrowUp".equals(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_DPAD_UP)), "modifiers and arrows");
        check(KeyPressIds.forKeyCode(KeyEvent.KEYCODE_BACK) == null
            && KeyPressIds.forKeyCode(KeyEvent.KEYCODE_VOLUME_UP) == null,
            "system keys are not keyboard keys");
        // A literal bound: KeyEvent.getMaxKeyCodeConstant() is a stub on a host JVM.
        for (int code = 0; code <= 400; code++) {
            String id = KeyPressIds.forKeyCode(code);
            check(id == null || KeyPressIds.isKnown(id), "every key code maps into the whitelist: " + code);
        }
        for (String id : KeyPressIds.KEY_IDS) {
            check(!KeyPressIds.label(id).isEmpty(), "every id has a label: " + id);
        }
        check("A".equals(KeyPressIds.label("KeyA")) && "空格".equals(KeyPressIds.label("Space"))
            && "九键 2".equals(KeyPressIds.label("Nine2")) && "14 键 QW".equals(KeyPressIds.label("FourteenQW"))
            && "14 键 L".equals(KeyPressIds.label("FourteenL")), "labels read as the keys do");
    }

    private static void batching() {
        KeyPressBatch batch = new KeyPressBatch();
        check(batch.isEmpty() && batch.drain() == null, "an empty batch flushes nothing");
        check(batch.add("KeyA", "2026-09-20") == null, "the first press opens the batch");
        batch.add("KeyA", "2026-09-20");
        batch.add("Space", "2026-09-20");
        check(batch.add("NotAKey", "2026-09-20") == null && batch.add(null, "2026-09-20") == null,
            "an unknown id is dropped rather than poisoning the batch");

        // The press after midnight hands back the old day's batch, filed under the old day.
        KeyPressBatch.Flush yesterday = batch.add("KeyB", "2026-09-21");
        check(yesterday != null && "2026-09-20".equals(yesterday.day()), "the old day flushes under its own day");
        check(yesterday.keys().equals(Map.of("KeyA", 2L, "Space", 1L)), "the old day keeps only its own presses");
        check(yesterday.presses() == 3, "the old day's presses");

        KeyPressBatch.Flush today = batch.drain();
        check(today != null && "2026-09-21".equals(today.day()) && today.keys().equals(Map.of("KeyB", 1L)),
            "the new day starts from its first press");
        check(batch.isEmpty() && batch.drain() == null, "a drained batch is empty");

        for (int index = 0; index < KeyPressBatch.FLUSH_PRESSES - 1; index++) batch.add("KeyE", "2026-09-21");
        check(!batch.full(), "a batch below the threshold waits for the timer");
        batch.add("KeyE", "2026-09-21");
        check(batch.full(), "the threshold press asks for a flush");
        KeyPressBatch.Flush full = batch.drain();
        check(full.keys().get("KeyE") == KeyPressBatch.FLUSH_PRESSES, "held presses are counted, not lost");

        batch.add("KeyA", "2026-09-22");
        batch.clear();
        check(batch.isEmpty() && batch.add("KeyA", "2026-09-23") == null,
            "a cleared batch is gone, so turning recording off drops it");
    }

    private static void layout() {
        for (List<String> row : KeyPressIds.SOFT_ROWS) {
            for (String id : row) check(KeyPressIds.isKnown(id), "a drawn soft key is whitelisted: " + id);
        }
        for (List<String> row : KeyPressIds.NINE_ROWS) {
            for (String id : row) check(KeyPressIds.isKnown(id), "a drawn nine-key cell is whitelisted: " + id);
        }
        Map<String, Long> counts = Map.of("KeyA", 5L, "Tab", 2L, "F1", 7L, "Nine2", 3L, "Escape", 0L);
        check(KeyPressIds.hasNineKey(counts) && !KeyPressIds.hasNineKey(Map.of("KeyA", 1L)),
            "the grid is drawn only when a nine-key cell was pressed");
        check(KeyPressIds.others(counts, true).equals(List.of("F1", "Tab")),
            "keys with no drawn position are listed, most pressed first, unpressed left out");
        check(KeyPressIds.others(counts, false).equals(List.of("F1", "Nine2", "Tab")),
            "without the grid its cells are listed instead of lost");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
