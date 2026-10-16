//! 网址模式：在全拼、双拼、五笔的组字中键入 `www.`、`http:`、`https:`、`ftp.`、`ftp:`，或在由 ASCII 字母、数字组成的组字后键入 `@`（邮箱）后，后续按键原样作为 ASCII 网址输入，不再转成中文。第一行始终是显示整段网址的兜底行，后面跟着补全：邮箱后缀（`email_completions`）和顶级域名（`tld_completions`）。组字里原文是 `ht`、`www` 这类开头时，候选里另有 `https://`、`www.` 等补全（`scheme_completions`），选中后以它进入网址模式。

/// 网址模式下作为输入而非标点的非字母字符：数字和网址里出现的符号。Shift+数字行的符号必须全部列出，宿主才不会把它们当成选候选。
pub const SPELLING_SYMBOLS: &str = "0123456789-._~:/?#[]@!$&'()*+,;=%^";

/// 网址的最长长度，超出后按键被吞掉。
pub const INPUT_LIMIT: usize = 512;

/// 触发词及其触发键。组字原文必须与触发词完全相同（全部小写 ASCII 字母）。
const TRIGGERS: [(&str, &str); 4] = [("www", "."), ("http", ":"), ("https", ":"), ("ftp", ".:")];

/// 把组字转成邮箱的键：组字原文由 ASCII 字母、数字组成（`is_email_local_part`）时，按下它进入网址模式写邮箱。
const EMAIL_KEY: char = '@';

/// 常用邮箱后缀，国内常用的在前，然后是海外的。与 Android、iOS、鸿蒙宿主各自的后缀表（任何输入框里打到 `xxx@` 时候选栏给出）是同一份、同一个顺序，`scripts/test-email-suffixes.py` 核对四处一致。
pub const EMAIL_SUFFIXES: [&str; 20] = [
    "@qq.com",
    "@163.com",
    "@126.com",
    "@foxmail.com",
    "@139.com",
    "@189.cn",
    "@aliyun.com",
    "@gmail.com",
    "@outlook.com",
    "@hotmail.com",
    "@live.com",
    "@icloud.com",
    "@yahoo.com",
    "@proton.me",
    "@protonmail.com",
    "@aol.com",
    "@mail.com",
    "@gmx.com",
    "@naver.com",
    "@yahoo.co.jp",
];

/// 正在打的主机名补全的顶级域名，按给出的顺序。
const TOP_LEVEL_DOMAINS: [&str; 5] = [".com", ".cn", ".net", ".org", ".com.cn"];

/// 组字原文能补全成的网址开头：原文是左边这个词的前缀时给出右边的补全。
const SCHEME_COMPLETIONS: [(&str, &str); 4] = [
    ("https", "https://"),
    ("http", "http://"),
    ("www", "www."),
    ("ftp", "ftp://"),
];

/// 组字原文为 `raw` 时能进入网址模式的按键：触发词的触发键，原文能做邮箱的本地部分时再加上 `@`；都不是时为空。
pub fn entry_keys(raw: &str) -> String {
    let triggers = trigger_keys(raw);
    let email = is_email_local_part(raw);
    let mut keys = String::with_capacity(triggers.len() + usize::from(email));
    keys.push_str(triggers);
    if email {
        keys.push(EMAIL_KEY);
    }
    keys
}

/// 组字原文为 `raw` 时按下 `key` 是否进入网址模式。
pub fn opens(raw: &str, key: u8) -> bool {
    trigger_keys(raw).as_bytes().contains(&key)
        || (char::from(key) == EMAIL_KEY && is_email_local_part(raw))
}

/// 原文是触发词时它的触发键，否则为空。
fn trigger_keys(raw: &str) -> &'static str {
    TRIGGERS
        .iter()
        .find(|(word, _)| *word == raw)
        .map_or("", |(_, keys)| keys)
}

/// 组字原文能做邮箱的本地部分：非空，只有 ASCII 字母和数字（带撇号的拼音不算）。
fn is_email_local_part(raw: &str) -> bool {
    !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// 五笔码长上限为 4，`http` 之后的 `s` 会被码表拒绝，所以五笔在这第 5 个字母上直接进入网址模式。
pub fn wubi_continues(raw: &str, letter: u8) -> bool {
    raw == "http" && letter == b's'
}

/// 网址模式接受的按键：字母、数字和 `SPELLING_SYMBOLS` 中的符号。其余按键结束网址。
pub fn accepts(key: u8) -> bool {
    key.is_ascii_alphanumeric() || SPELLING_SYMBOLS.as_bytes().contains(&key)
}

/// 组字原文 `raw` 是 `https`、`http`、`www`、`ftp` 的前缀且至少两个字母时给出的补全，按 `SCHEME_COMPLETIONS` 的顺序：`ht`、`htt`、`http` 给 `https://` 和 `http://`，`https` 只给 `https://`。大写字母是辅助码，不算。
pub fn scheme_completions(raw: &str) -> impl Iterator<Item = &'static str> + '_ {
    SCHEME_COMPLETIONS
        .iter()
        .filter(move |(word, _)| raw.len() >= 2 && word.starts_with(raw))
        .map(|(_, completion)| *completion)
}

/// 网址模式的预编辑是 `<本地部分>@<已打的域名>` 时补全的邮箱：本地部分加上以 `@<已打的域名>` 开头（不分大小写）的每个后缀，按 `EMAIL_SUFFIXES` 的顺序。规则与 Android 的 `EmailSuffixPolicy.match` 相同：只有一个 `@`，本地部分非空且只含 ASCII 字母、数字和 `._%+-`，已打的域名只含 ASCII 字母、数字、`.` 和 `-`；已打的域名已经是一个完整后缀时没有补全。
pub fn email_completions(preedit: &str) -> Vec<String> {
    let Some((local, domain)) = preedit.split_once('@') else {
        return Vec::new();
    };
    let local_part = !local.is_empty()
        && local
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._%+-".contains(&byte));
    let typed_domain = domain
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-');
    if !local_part || !typed_domain {
        return Vec::new();
    }
    let matches = |suffix: &str| {
        suffix.len() > domain.len() && suffix[1..1 + domain.len()].eq_ignore_ascii_case(domain)
    };
    if EMAIL_SUFFIXES
        .iter()
        .any(|suffix| suffix[1..].eq_ignore_ascii_case(domain))
    {
        return Vec::new();
    }
    EMAIL_SUFFIXES
        .iter()
        .filter(|suffix| matches(suffix))
        .map(|suffix| format!("{local}{suffix}"))
        .collect()
}

/// 网址模式的预编辑以 `www.` 开头或含 `://`、正在打的主机名还没有 `.`（`www.baidu`、`https://github`、`https://www.baidu`）时补全的顶级域名：预编辑加上 `TOP_LEVEL_DOMAINS` 的每一项。主机名为空或含其他符号（已经打到路径、端口）时没有补全。
pub fn tld_completions(preedit: &str) -> Vec<String> {
    let host = match preedit.find("://") {
        Some(at) => &preedit[at + 3..],
        None => match preedit.strip_prefix("www.") {
            Some(host) => host,
            None => return Vec::new(),
        },
    };
    let label = host.strip_prefix("www.").unwrap_or(host);
    if label.is_empty()
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Vec::new();
    }
    TOP_LEVEL_DOMAINS
        .iter()
        .map(|domain| format!("{preedit}{domain}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        accepts, email_completions, entry_keys, opens, scheme_completions, tld_completions,
        wubi_continues, EMAIL_SUFFIXES, SPELLING_SYMBOLS,
    };

    #[test]
    fn only_the_exact_scheme_words_open_the_mode() {
        assert_eq!(entry_keys("www"), ".@");
        assert_eq!(entry_keys("http"), ":@");
        assert_eq!(entry_keys("https"), ":@");
        assert_eq!(entry_keys("ftp"), ".:@");
        // 不是触发词的原文只能用 `@` 进入（邮箱）；带撇号或为空时都不能。
        for raw in ["ww", "wwww", "Www", "ni", "mailto", "httpss", "abc123"] {
            assert_eq!(entry_keys(raw), "@", "{raw}");
        }
        for raw in ["w'ww", "", "xi'an"] {
            assert_eq!(entry_keys(raw), "", "{raw}");
        }
        assert!(opens("ftp", b':') && opens("ftp", b'.'));
        assert!(!opens("www", b':') && opens("www", b'@') && !opens("ni", b'.'));
        assert!(wubi_continues("http", b's'));
        assert!(!wubi_continues("http", b't') && !wubi_continues("htt", b's'));
    }

    #[test]
    fn every_listed_symbol_is_accepted_and_the_url_enders_are_not() {
        assert!(SPELLING_SYMBOLS.bytes().all(accepts));
        // Shift+数字行全部列出，宿主才不会把它们读成选候选。
        assert!("!@#$%^&*()0123456789"
            .bytes()
            .all(|key| SPELLING_SYMBOLS.as_bytes().contains(&key)));
        assert!(accepts(b'a') && accepts(b'Z') && accepts(b'7'));
        for key in *b"\"<>\\{}|` " {
            assert!(!accepts(key), "{}", key as char);
        }
    }

    #[test]
    fn scheme_completions_follow_the_typed_prefix() {
        let completions = |raw| scheme_completions(raw).collect::<Vec<_>>();
        for raw in ["ht", "htt", "http"] {
            assert_eq!(completions(raw), ["https://", "http://"], "{raw}");
        }
        assert_eq!(completions("https"), ["https://"]);
        assert_eq!(completions("ww"), ["www."]);
        assert_eq!(completions("www"), ["www."]);
        assert_eq!(completions("ft"), ["ftp://"]);
        assert_eq!(completions("ftp"), ["ftp://"]);
        for raw in ["h", "w", "", "httpss", "wwww", "Ht", "ni"] {
            assert!(completions(raw).is_empty(), "{raw}");
        }
    }

    #[test]
    fn email_completions_narrow_by_the_typed_domain() {
        let all = email_completions("zhangsan@");
        assert_eq!(all.len(), EMAIL_SUFFIXES.len());
        assert_eq!(all[0], "zhangsan@qq.com");
        assert_eq!(all[19], "zhangsan@yahoo.co.jp");
        assert_eq!(
            email_completions("abc@1"),
            ["abc@163.com", "abc@126.com", "abc@139.com", "abc@189.cn"]
        );
        // 不分大小写，本地部分照原样，后缀用表里的小写。
        assert_eq!(email_completions("Zhang.San@GMa"), ["Zhang.San@gmail.com"]);
        assert_eq!(
            email_completions("abc@yahoo.c"),
            ["abc@yahoo.com", "abc@yahoo.co.jp"]
        );
        // 已经是一个完整后缀，或者没有后缀接得上，都不给。
        for preedit in [
            "abc@qq.com",
            "abc@QQ.COM",
            "abc@yahoo.com",
            "abc@example",
            "abc",
            "@qq",
            "a@b@",
            "https://a@",
            "abc@q q",
        ] {
            assert!(email_completions(preedit).is_empty(), "{preedit}");
        }
    }

    #[test]
    fn tld_completions_follow_the_host_being_typed() {
        assert_eq!(
            tld_completions("www.baidu"),
            [
                "www.baidu.com",
                "www.baidu.cn",
                "www.baidu.net",
                "www.baidu.org",
                "www.baidu.com.cn"
            ]
        );
        assert_eq!(tld_completions("https://github")[0], "https://github.com");
        assert_eq!(
            tld_completions("https://www.my-site")[4],
            "https://www.my-site.com.cn"
        );
        for preedit in [
            "www.",
            "https://",
            "www.baidu.com",
            "https://github.com/a",
            "https://localhost:8080",
            "ftp.example",
            "abc@qq",
            "github",
        ] {
            assert!(tld_completions(preedit).is_empty(), "{preedit}");
        }
    }
}
