//! 组字光标在 `preedit` 里的位置（`EngineSnapshot::preedit_caret`）。
//!
//! `caret_position` 是按键原文 `editing_text` 里的字节偏移，而宿主画在行内和候选窗里的是 `preedit`。两者相同时偏移照搬；全拼只在原文的字母之间加了显示用的隔音符（`nihao` → `ni'hao`），按字母数换过去就行；双拼选了「拼音分词」时 `preedit` 是展开后的全拼（`uiui` → `shi'shi`），字母数对不上，只能按音节换：光标前有几个完整的双拼单位，就落在第几个显示音节之后。以前这一步由各宿主按字母数自己猜，Windows 在双拼上因此把光标画进了音节中间（#10632），macOS 只好一律放在末尾。

/// 显示用的分隔符：引擎插入的隔音符和用户打的空格。
fn separator(value: u8) -> bool {
    value == b'\'' || value == b' '
}

fn scalar_count(text: &str) -> usize {
    text.chars().count()
}

/// `text` 里去掉分隔符的字母序列；含字母和分隔符以外的字符时为 `None`。
fn letters(text: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(text.len());
    for &value in text.as_bytes() {
        if separator(value) {
            continue;
        }
        if !value.is_ascii_alphabetic() {
            return None;
        }
        output.push(value);
    }
    Some(output)
}

/// `preedit` 只是在 `editing` 的字母之间加了分隔符（全拼分词）时，把字母光标带过分隔符：落在音节边界上的光标停在插入的隔音符前面，紧跟在用户自己打的分隔符后面的光标越过此处的分隔符。与 macOS `MSIMEPreeditCaretPosition`、Windows `preedit_display_caret` 原有的映射相同。
fn through_separators(editing: &str, preedit: &str, caret: usize) -> Option<usize> {
    let typed = letters(editing)?;
    if typed.is_empty() || letters(preedit)? != typed {
        return None;
    }
    let editing = editing.as_bytes();
    let preedit = preedit.as_bytes();
    let mut remaining = editing[..caret]
        .iter()
        .filter(|&&value| !separator(value))
        .count();
    let mut position = 0;
    while position < preedit.len() && remaining > 0 {
        if !separator(preedit[position]) {
            remaining -= 1;
        }
        position += 1;
    }
    if caret > 0 && separator(editing[caret - 1]) {
        while position < preedit.len() && separator(preedit[position]) {
            position += 1;
        }
    }
    Some(position)
}

/// `preedit` 的每个音节对应原文的一个单位（`boundaries` 是 `segment_raw_boundaries`）时，光标前有几个完整单位就落在第几个显示音节之后。光标在一个单位中间时按单位起点算，和光标前缀解码（`prefix_end`）取的是同一段：候选也只是那几个完整单位的。显示音节数和原文单位数对不上（末尾的辅助码、显示不是一音节一段）时不猜，交给调用方放在末尾。
fn through_units(
    editing: &str,
    preedit: &str,
    caret: usize,
    boundaries: &[usize],
) -> Option<usize> {
    if !preedit.is_ascii() || boundaries.len() < 2 {
        return None;
    }
    let raw = editing.as_bytes();
    let unit = |start: usize, end: usize| {
        raw.get(start..end)
            .is_some_and(|slice| slice.iter().any(|&value| !separator(value)))
    };
    let units = boundaries
        .windows(2)
        .filter(|pair| unit(pair[0], pair[1]))
        .count();
    let before = boundaries
        .windows(2)
        .filter(|pair| pair[1] <= caret && unit(pair[0], pair[1]))
        .count();
    let display = preedit.as_bytes();
    let mut pieces = Vec::new();
    let mut start = None;
    for (index, &value) in display.iter().enumerate() {
        if separator(value) {
            if let Some(begin) = start.take() {
                pieces.push((begin, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(begin) = start {
        pieces.push((begin, display.len()));
    }
    if pieces.len() != units {
        return None;
    }
    let mut position = match before {
        0 => 0,
        count => pieces[count - 1].1,
    };
    if caret > 0 && separator(raw[caret - 1]) {
        while position < display.len() && separator(display[position]) {
            position += 1;
        }
    }
    Some(position)
}

/// 光标在 `preedit` 里的位置，按 Unicode 标量计。`caret` 是 `editing` 里的字节偏移，`boundaries` 是 `editing` 的单位边界（没有可编辑单位时为空）。光标在末尾、或两段文字之间没有可靠的对应（韩文音节、假名、笔画字形、换掉了字母的纠错显示）时在 `preedit` 末尾：光标画错位置比画在末尾更糟。
pub(super) fn preedit_caret(
    editing: &str,
    preedit: &str,
    caret: usize,
    boundaries: &[usize],
) -> usize {
    let end = scalar_count(preedit);
    if caret >= editing.len() || !editing.is_char_boundary(caret) {
        return end;
    }
    if editing == preedit {
        return scalar_count(&editing[..caret]);
    }
    // 下面两种对应都只在 ASCII 的 `preedit` 上成立，字节偏移就是标量数。
    through_separators(editing, preedit, caret)
        .or_else(|| through_units(editing, preedit, caret, boundaries))
        .unwrap_or(end)
}

#[cfg(test)]
mod tests {
    use super::preedit_caret;

    #[test]
    fn identical_text_keeps_the_caret() {
        assert_eq!(preedit_caret("nihao", "nihao", 2, &[0, 2, 5]), 2);
        assert_eq!(preedit_caret("nihao", "nihao", 5, &[0, 2, 5]), 5);
    }

    #[test]
    fn quanpin_separators_carry_the_letter_caret() {
        let boundaries = [0, 2, 5];
        let positions: Vec<_> = (0..=5)
            .map(|caret| preedit_caret("nihao", "ni'hao", caret, &boundaries))
            .collect();
        // 音节边界（原文 2）落在插入的隔音符前面。
        assert_eq!(positions, vec![0, 1, 2, 4, 5, 6]);
        // 用户自己打的隔音符：紧跟其后的光标越过它。
        assert_eq!(preedit_caret("ni'hao", "ni'hao", 3, &[0, 2, 3, 6]), 3);
        assert_eq!(preedit_caret("xi'an", "xi'an", 3, &[0, 2, 3, 5]), 3);
    }

    #[test]
    fn expanded_shuangpin_lands_after_whole_syllables() {
        // 小鹤 `uiui` 是 shi'shi：原文两个键一个音节，显示三个字母一个音节。
        let boundaries = [0, 2, 4];
        let positions: Vec<_> = (0..=4)
            .map(|caret| preedit_caret("uiui", "shi'shi", caret, &boundaries))
            .collect();
        // 末尾在显示末尾；单位边界在第一个音节后面、隔音符前面；单位中间按单位起点。
        assert_eq!(positions, vec![0, 0, 3, 3, 7]);
        // 末尾还在打的半个单位：`uiu` 是 shi'sh。
        assert_eq!(preedit_caret("uiu", "shi'sh", 3, &[0, 2, 3]), 6);
        assert_eq!(preedit_caret("uiu", "shi'sh", 2, &[0, 2, 3]), 3);
    }

    #[test]
    fn expanded_shuangpin_with_typed_separators() {
        // `ni'hc` 显示为 ni'hao：用户打的隔音符和显示的隔音符是同一个。
        let boundaries = [0, 2, 3, 5];
        assert_eq!(preedit_caret("ni'hc", "ni'hao", 2, &boundaries), 2);
        assert_eq!(preedit_caret("ni'hc", "ni'hao", 3, &boundaries), 3);
        assert_eq!(preedit_caret("ni'hc", "ni'hao", 5, &boundaries), 6);
        // 末尾打了隔音符还没有下一个音节。
        assert_eq!(preedit_caret("ui'", "shi'", 3, &[0, 2, 3]), 4);
        assert_eq!(preedit_caret("ui'", "shi'", 2, &[0, 2, 3]), 3);
    }

    #[test]
    fn unmatched_displays_put_the_caret_at_the_end() {
        // 音节数对不上（例如末尾的辅助码单独成一个单位却不单独显示）时不猜。
        assert_eq!(preedit_caret("nihcab", "ni'hao", 2, &[0, 2, 4, 6]), 6);
        // 韩文音节、假名这类不是字母的显示。
        assert_eq!(preedit_caret("gks", "한", 1, &[]), 1);
        assert_eq!(preedit_caret("kana", "かな", 2, &[]), 2);
        // 没有单位边界的方案。
        assert_eq!(preedit_caret("abc", "xyz", 1, &[]), 3);
        // 光标越界。
        assert_eq!(preedit_caret("ui", "shi", 9, &[0, 2]), 3);
        assert_eq!(preedit_caret("", "", 0, &[]), 0);
    }
}
