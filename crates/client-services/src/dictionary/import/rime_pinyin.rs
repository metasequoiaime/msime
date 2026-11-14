//! Rime 带调拼音转为水杉的无调编码；只供拼音和快捷短语导入使用。

pub(super) fn normalize(code: &str, quick_phrase: bool) -> String {
    let mut output = String::with_capacity(code.len());
    for character in code.chars().flat_map(char::to_lowercase) {
        let plain = match character {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' | 'ê' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
            'ń' | 'ň' | 'ǹ' => 'n',
            'ḿ' => 'm',
            ' ' | '\'' if quick_phrase => continue,
            // 不吞掉未知字符，交给既有编码校验拒绝这一行。
            other => other,
        };
        output.push(plain);
    }
    output
}
