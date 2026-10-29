//! 读打字流评测集（`resources/eval/typing-*.tsv`），把每条消息切成用户一口气打完的块，定下每块打哪些键。
//!
//! 切块和简拼都不用随机数生成器的状态：每块的长度和要不要简拼只由种子、行号和块的序号决定（[`unit`]），所以同一份文件、同一组参数在任何机器上切出同样的块，按别的顺序打（`--order-seed`）也不变。

use msime_engine::KeyGrid;
use std::path::Path;

/// 一行属于哪一部分：预热只打不计，dev 日常看，test 只在里程碑时看。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Split {
    Warmup,
    Dev,
    Test,
}

impl Split {
    pub fn name(self) -> &'static str {
        match self {
            Split::Warmup => "warmup",
            Split::Dev => "dev",
            Split::Test => "test",
        }
    }
}

/// 一个词：汉字和它的音节（`ü` 写作 `v`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub syllables: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Word(Word),
    /// 标点、数字、字母这些不经拼音打的文字，原样写进上文，不计入分母。
    Literal(String),
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub doc: String,
    pub split: Split,
    pub tokens: Vec<Token>,
}

/// 读一份评测集，同时返回文件内容的 FNV-1a 64 位摘要，写进报告，说明量的是哪一份。
pub fn load(path: &Path) -> Result<(Vec<Row>, u64), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let text =
        String::from_utf8(bytes.clone()).map_err(|_| format!("{}: not UTF-8", path.display()))?;
    let mut rows = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') || line.starts_with("id\t") {
            continue;
        }
        rows.push(
            parse_row(line)
                .map_err(|error| format!("{}:{}: {error}", path.display(), number + 1))?,
        );
    }
    Ok((rows, fnv1a(&bytes)))
}

fn parse_row(line: &str) -> Result<Row, String> {
    let fields: Vec<&str> = line.split('\t').collect();
    // Tatoeba 那份多一列 `author`（CC BY 的署名），模拟用不上。
    let ([id, doc, split, _text, tokens] | [id, doc, split, _text, tokens, _]) = fields[..] else {
        return Err(format!("expected 5 or 6 columns, found {}", fields.len()));
    };
    let split = match split {
        "warmup" => Split::Warmup,
        "dev" => Split::Dev,
        "test" => Split::Test,
        other => return Err(format!("unknown split {other}")),
    };
    let tokens = tokens
        .split(' ')
        .filter(|token| !token.is_empty())
        .map(parse_token)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Row {
        id: id.to_owned(),
        doc: doc.to_owned(),
        split,
        tokens,
    })
}

fn parse_token(token: &str) -> Result<Token, String> {
    if let Some(literal) = token.strip_prefix('~') {
        return Ok(Token::Literal(literal.replace('_', " ")));
    }
    let (text, pinyin) = token
        .split_once('=')
        .ok_or_else(|| format!("token without a reading: {token}"))?;
    let syllables: Vec<String> = pinyin.split('\'').map(str::to_owned).collect();
    if text.chars().count() != syllables.len()
        || syllables.iter().any(|syllable| {
            syllable.is_empty() || !syllable.bytes().all(|b| b.is_ascii_lowercase())
        })
    {
        return Err(format!("reading does not match the word: {token}"));
    }
    Ok(Token::Word(Word {
        text: text.to_owned(),
        syllables,
    }))
}

/// 怎么把一条消息切成块。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chunking {
    /// 每块的目标字数取 1 加一个几何分布，平均 `mean` 字，最长 8 字；按词往块里放，放不下就收口，词不拆开。
    Sampled { mean: f64, seed: u64 },
    /// 标点之间整段打。
    Clause,
    /// 一个词一块。
    Word,
}

/// 一块最多打多少键。九键 48 位封顶（`DIGIT_LIMIT`），留出余量，也让三种键盘切出同样的块：每个字母都是一键。
pub const CHUNK_KEY_LIMIT: usize = 40;
/// 抽样切块时一块最长几个字。
const SAMPLED_MAX_CHARS: usize = 8;

/// 用户一口气打完、再去选词的一段。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub text: String,
    pub syllables: Vec<String>,
}

/// 一条消息依次是块和原样的文字。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Chunk(Chunk),
    Literal(String),
}

pub fn pieces(row: &Row, chunking: Chunking) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut current: Option<Chunk> = None;
    let mut target = 0;
    let mut drawn = 0u64;
    for token in &row.tokens {
        let word = match token {
            Token::Literal(text) => {
                close(&mut current, &mut out);
                out.push(Piece::Literal(text.clone()));
                continue;
            }
            Token::Word(word) => word,
        };
        let length = word.syllables.len();
        let keys: usize = word.syllables.iter().map(String::len).sum();
        if let Some(chunk) = &current {
            let chars = chunk.syllables.len();
            let chunk_keys: usize = chunk.syllables.iter().map(String::len).sum();
            let fits = chunk_keys + keys <= CHUNK_KEY_LIMIT
                && match chunking {
                    Chunking::Sampled { .. } => chars + length <= target,
                    Chunking::Clause => true,
                    Chunking::Word => false,
                };
            if !fits {
                close(&mut current, &mut out);
            }
        }
        let chunk = current.get_or_insert_with(|| {
            if let Chunking::Sampled { mean, seed } = chunking {
                target = sampled_length(mean, seed, &row.id, drawn);
                drawn += 1;
            }
            Chunk {
                text: String::new(),
                syllables: Vec::new(),
            }
        });
        chunk.text.push_str(&word.text);
        chunk.syllables.extend(word.syllables.iter().cloned());
    }
    close(&mut current, &mut out);
    out
}

fn close(current: &mut Option<Chunk>, out: &mut Vec<Piece>) {
    if let Some(chunk) = current.take() {
        out.push(Piece::Chunk(chunk));
    }
}

/// 抽样切块的第 `index` 块的目标字数：1 加一个成功概率 1/`mean` 的几何分布（失败次数），均值 `mean`，截在 8 字。
fn sampled_length(mean: f64, seed: u64, row: &str, index: u64) -> usize {
    let p = 1.0 / mean.max(1.0);
    let mut length = 1;
    let mut draw = 0;
    while length < SAMPLED_MAX_CHARS && unit(seed, row, index * 64 + draw) >= p {
        length += 1;
        draw += 1;
    }
    length
}

/// [0, 1) 上的一个数，只由种子、行号和序号决定。
pub fn unit(seed: u64, row: &str, index: u64) -> f64 {
    let mut state =
        fnv1a(row.as_bytes()) ^ seed.rotate_left(17) ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // splitmix64 的最后一步，把相邻的序号打散。
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^= state >> 31;
    (state >> 11) as f64 / (1u64 << 53) as f64
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// 停在声母上的方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abbreviation {
    /// 只有末音节打声母（吃饭 打 `chif`）。
    Last,
    /// 每个音节都打声母（我们 打 `wm`）。
    All,
}

/// 一个音节只打声母时打的字母：zh、ch、sh 算一个声母，零声母音节（an、e）留第一个字母，y、w 照常算声母。
pub fn initial(syllable: &str) -> &str {
    let length = if ["zh", "ch", "sh"].iter().any(|r| syllable.starts_with(r)) {
        2
    } else {
        1
    };
    &syllable[..length.min(syllable.len())]
}

/// 第 `chunk` 块要不要停在声母上：同一个数在 `share` 增大时只会从否变成是，所以 0.3 简拼的块一定也是 0.6 简拼的块。
pub fn abbreviates(seed: u64, row: &str, chunk: u64, share: f64) -> bool {
    share > 0.0 && unit(seed ^ 0xABB2, row, chunk) < share
}

/// 一块里每个音节实际打的字母。简拼了却跟没简拼一样（末音节本来就只有声母那几个字母）时返回 `None`。
pub fn typed_letters(chunk: &Chunk, abbreviation: Option<Abbreviation>) -> Option<Vec<String>> {
    let mut typed: Vec<String> = chunk.syllables.clone();
    let Some(mode) = abbreviation else {
        return Some(typed);
    };
    let last = typed.len() - 1;
    let mut changed = false;
    for (index, syllable) in typed.iter_mut().enumerate() {
        if mode == Abbreviation::Last && index != last {
            continue;
        }
        let kept = initial(syllable).to_owned();
        if kept.len() < syllable.len() {
            *syllable = kept;
            changed = true;
        }
    }
    changed.then_some(typed)
}

/// 被测的键盘。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyboard {
    Full,
    Nine,
    Fourteen,
}

impl Keyboard {
    pub fn name(self) -> &'static str {
        match self {
            Keyboard::Full => "full",
            Keyboard::Nine => "nine",
            Keyboard::Fourteen => "fourteen",
        }
    }

    pub fn grid(self) -> Option<KeyGrid> {
        match self {
            Keyboard::Full => None,
            Keyboard::Nine => Some(KeyGrid::NineKey),
            Keyboard::Fourteen => Some(KeyGrid::FourteenKey),
        }
    }

    /// 一串字母在这个键盘上按的键：全键盘是字母本身，九键是数字，14 键是组的首字母。
    pub fn keys(self, letters: &str) -> String {
        match self.grid() {
            Some(grid) => grid.encode(letters),
            None => letters.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(tokens: &str) -> Row {
        parse_row(&format!("r1\td1\ttest\t-\t{tokens}")).unwrap()
    }

    fn chunk_texts(pieces: &[Piece]) -> Vec<String> {
        pieces
            .iter()
            .map(|piece| match piece {
                Piece::Chunk(chunk) => chunk.text.clone(),
                Piece::Literal(text) => format!("~{text}"),
            })
            .collect()
    }

    #[test]
    fn tokens_parse_words_and_literals() {
        let row = row("我们=wo'men ~， 绿=lv ~ok_la");
        assert_eq!(
            row.tokens,
            vec![
                Token::Word(Word {
                    text: "我们".into(),
                    syllables: vec!["wo".into(), "men".into()]
                }),
                Token::Literal("，".into()),
                Token::Word(Word {
                    text: "绿".into(),
                    syllables: vec!["lv".into()]
                }),
                Token::Literal("ok la".into()),
            ]
        );
        assert!(parse_row("r\td\ttest\t-\t我们=wo").is_err());
        assert!(parse_row("r\td\tnope\t-\t我=wo").is_err());
        assert!(parse_row("r\td\ttest\t-\t我=Wo").is_err());
        // 第 6 列是署名，读进来的和没有这一列时一样。
        assert_eq!(
            parse_row("r\td\ttest\t-\t我=wo\tsomeone").unwrap().tokens,
            parse_row("r\td\ttest\t-\t我=wo").unwrap().tokens
        );
        assert!(parse_row("r\td\ttest\t-\t我=wo\ta\tb").is_err());
    }

    #[test]
    fn clause_and_word_chunking_follow_punctuation_and_words() {
        let row = row("今天=jin'tian 天气=tian'qi 很=hen 好=hao ~， 我们=wo'men 出去=chu'qu");
        assert_eq!(
            chunk_texts(&pieces(&row, Chunking::Clause)),
            ["今天天气很好", "~，", "我们出去"]
        );
        assert_eq!(
            chunk_texts(&pieces(&row, Chunking::Word)),
            ["今天", "天气", "很", "好", "~，", "我们", "出去"]
        );
    }

    #[test]
    fn clauses_split_at_a_word_boundary_before_the_key_limit() {
        // 每个词 12 个字母，三个是 36，第四个会超过 40。
        let word = "庄庄=zhuang'zhuang";
        let row = row(&[word; 4].join(" "));
        let pieces = pieces(&row, Chunking::Clause);
        assert_eq!(chunk_texts(&pieces), ["庄庄庄庄庄庄", "庄庄"]);
    }

    #[test]
    fn sampled_chunks_never_split_a_word_and_stay_within_eight_characters() {
        let tokens = "我=wo 今天=jin'tian 下午=xia'wu 要=yao 去=qu 公司=gong'si 开会=kai'hui 讨论=tao'lun 新=xin 项目=xiang'mu";
        let row = row(tokens);
        for seed in 0..50 {
            let pieces = pieces(&row, Chunking::Sampled { mean: 3.5, seed });
            let joined: String = chunk_texts(&pieces).concat();
            assert_eq!(joined, "我今天下午要去公司开会讨论新项目");
            for piece in &pieces {
                let Piece::Chunk(chunk) = piece else { panic!() };
                assert!(!chunk.text.is_empty() && chunk.syllables.len() <= 8);
            }
            // 同一组参数切出来永远一样。
            assert_eq!(
                pieces,
                super::pieces(&row, Chunking::Sampled { mean: 3.5, seed })
            );
        }
    }

    #[test]
    fn sampled_lengths_average_close_to_the_mean() {
        let total: usize = (0..4000)
            .map(|index| sampled_length(3.5, 7, "row", index))
            .sum();
        let mean = total as f64 / 4000.0;
        // 截在 8 字，均值比 3.5 略低。
        assert!((3.0..3.6).contains(&mean), "{mean}");
    }

    #[test]
    fn abbreviation_shares_are_nested() {
        for chunk in 0..500 {
            if abbreviates(1, "row", chunk, 0.3) {
                assert!(abbreviates(1, "row", chunk, 0.6));
            }
            assert!(!abbreviates(1, "row", chunk, 0.0));
        }
        let share = (0..2000).filter(|&c| abbreviates(1, "row", c, 0.3)).count() as f64 / 2000.0;
        assert!((0.27..0.33).contains(&share), "{share}");
    }

    #[test]
    fn initials_cut_retroflexes_as_one_initial() {
        let chunk = |syllables: &[&str]| Chunk {
            text: "字".repeat(syllables.len()),
            syllables: syllables.iter().map(|s| s.to_string()).collect(),
        };
        assert_eq!(initial("zhang"), "zh");
        assert_eq!(initial("an"), "a");
        assert_eq!(initial("e"), "e");
        assert_eq!(
            typed_letters(&chunk(&["chi", "fan"]), Some(Abbreviation::Last)),
            Some(vec!["chi".into(), "f".into()])
        );
        assert_eq!(
            typed_letters(&chunk(&["wo", "men"]), Some(Abbreviation::All)),
            Some(vec!["w".into(), "m".into()])
        );
        // 末音节只有一个字母，截了跟没截一样。
        assert_eq!(
            typed_letters(&chunk(&["ni", "e"]), Some(Abbreviation::Last)),
            None
        );
        assert_eq!(
            typed_letters(&chunk(&["ni", "e"]), None),
            Some(vec!["ni".into(), "e".into()])
        );
    }

    #[test]
    fn keyboards_encode_letters_the_way_the_hosts_send_them() {
        assert_eq!(Keyboard::Nine.keys("chifan"), "244326");
        assert_eq!(Keyboard::Nine.keys("lv"), "58");
        assert_eq!(Keyboard::Full.keys("chifan"), "chifan");
        assert_eq!(Keyboard::Fourteen.keys("ni").len(), 2);
    }
}
