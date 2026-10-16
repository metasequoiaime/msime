//! 测试用的最小搜狗细胞词库：按 client-services `dictionary::scel` 的布局现场拼出字节，内容都是自造的词，不提交任何真实词库。

/// 每组是（读音的音节, 同音的词）。拼音表由出现过的音节按首次出现的顺序组成；音节写成 `?` 时用一个不在表里的序号，那一组的读音读不出。
pub(crate) fn scel(groups: &[(&[&str], &[&str])]) -> Vec<u8> {
    let mut table: Vec<&str> = Vec::new();
    for (reading, _) in groups {
        for syllable in *reading {
            if *syllable != "?" && !table.contains(syllable) {
                table.push(syllable);
            }
        }
    }
    let mut bytes = vec![0u8; 0x2628];
    bytes[..12].copy_from_slice(b"\x40\x15\x00\x00DCS\x01\x01\x00\x00\x00");
    let entries: usize = groups.iter().map(|(_, words)| words.len()).sum();
    bytes[0x120..0x124].copy_from_slice(&(groups.len() as u32).to_le_bytes());
    bytes[0x124..0x128].copy_from_slice(&(entries as u32).to_le_bytes());
    let name = utf16("合成词库");
    bytes[0x130..0x130 + name.len()].copy_from_slice(&name);
    bytes[0x1540..0x1544].copy_from_slice(&(table.len() as u32).to_le_bytes());
    let mut at = 0x1544;
    for (index, syllable) in table.iter().enumerate() {
        let mut row = (index as u16).to_le_bytes().to_vec();
        row.extend_from_slice(&((syllable.len() * 2) as u16).to_le_bytes());
        row.extend(utf16(syllable));
        bytes[at..at + row.len()].copy_from_slice(&row);
        at += row.len();
    }
    for (reading, words) in groups {
        bytes.extend_from_slice(&(words.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&((reading.len() * 2) as u16).to_le_bytes());
        for syllable in *reading {
            let index = table
                .iter()
                .position(|known| known == syllable)
                .unwrap_or(999);
            bytes.extend_from_slice(&(index as u16).to_le_bytes());
        }
        for (ordinal, word) in words.iter().enumerate() {
            let word = utf16(word);
            bytes.extend_from_slice(&(word.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&word);
            bytes.extend_from_slice(&10u16.to_le_bytes());
            bytes.extend_from_slice(&(ordinal as u32 + 1).to_le_bytes());
            bytes.extend_from_slice(&[0; 6]);
        }
    }
    bytes
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
