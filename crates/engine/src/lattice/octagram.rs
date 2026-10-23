//! Rime octagram（八股文）`.gram` 语法模型的只读查找端。
//!
//! Copyright (c) 2014, RIME Developers；移植来源固定于
//! `57d18b9f58e5284bd891d559f6bdd16cf60341e9`，完整许可见
//! `resources/licenses/librime-octagram-BSD-3-Clause.txt`。
//!
//! 格式与查询语义按 lotem/librime-octagram（BSD-3-Clause）移植，读取端即旧版
//! Windows 实现 `engine/ngram/octagram/octagram_gram.cpp` 的同款逻辑。文件是
//! 「Metadata + Darts 双数组 Trie 镜像」：键为「上下文尾部字符 + 词首字符」经
//! `encode` 压缩的字节串，值为 `int(ln(共现频次) * 10000)`。它不是概率语言
//! 模型：`query` 给出的是搭配强度信号——命中取 `max(值/10000 + 搭配惩罚)`，
//! 全部未命中返回 `non_collocation_penalty`。词表零耦合：我们词库里没有的词
//! 只是拿不到加成，不会报错。
//!
//! 头部布局（rime `MappedFile` 上的 Metadata，偏移 0）：`format` char[32]、
//! uint32 checksum、uint32 `double_array_size`（单位数，非字节）、int32
//! `OffsetPtr<char>`（相对自身字段的偏移）。双数组单元 4 字节。
//!
//! 装载沿用 `ngram.rs` 的 mmap 约定：`.gram` 经资源包以原子重命名安装
//! （`.incoming` 改名就位，绝不就地改写），映射随对象存活；万象模型的镜像接近
//! 400 MB，整读进内存不可取，RSS 只随命中的页增长。

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use lru::LruCache;
use memmap2::{Mmap, MmapOptions};

/// `query` 返回自然对数域的搭配强度；词格主分是 log10 口径，装配方乘
/// `f64::consts::LOG10_E`（= 1/ln(10)）换域，weight 才是纯标定系数。
/// 打分参数。默认 = 万象现行（`wanxiang.schema.yaml` 的 6/2/-14/-6/-100/-18），
/// 与旧版出厂模板 `config.default.toml` 的 `[association]` 键一致。octagram
/// 自身默认 `collocation_max_length=4`、`collocation_min_length=3`、
/// `collocation_penalty = non_collocation_penalty = -12`，我们不采用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GramConfig {
    /// 上下文与词首各取 `collocation_max_length - 1` 个码点，上限 8。
    pub collocation_max_length: i32,
    /// 搭配长度（上下文现存码点数 + 命中键码点数）低于它时吃更狠的 weak 惩罚。
    pub collocation_min_length: i32,
    pub collocation_penalty: f64,
    /// 全部窗口未命中、上下文为空或模型无效时的常数分。
    pub non_collocation_penalty: f64,
    pub weak_collocation_penalty: f64,
    /// 句尾词的「词+$」rear 项惩罚。
    pub rear_penalty: f64,
}

impl Default for GramConfig {
    fn default() -> Self {
        Self {
            collocation_max_length: 6,
            collocation_min_length: 2,
            collocation_penalty: -14.0,
            non_collocation_penalty: -6.0,
            weak_collocation_penalty: -100.0,
            rear_penalty: -18.0,
        }
    }
}

/// `gram_encoding.cc` / `gram_db.cc` 的常量。
pub const MAX_ENCODED_UNICODE: usize = 8;
const MAX_RESULTS: usize = 8;
const VALUE_SCALE: f64 = 10000.0;

/// 双数组单元数上限：万象镜像约 400 MB（一亿单元），放宽到它的四倍挡恶意计数。
const MAX_UNITS: usize = 400_000_000;

const FORMAT_MAGIC: &[u8] = b"Rime::Grammar/";
const HEADER_BYTES: usize = 44;
const FORMAT_OFFSET: usize = 0;
const ARRAY_SIZE_OFFSET: usize = 36;
const OFFSET_PTR_OFFSET: usize = 40;
const UNIT_BYTES: usize = 4;

/// 一条 `.gram` 镜像：mmap 只读视图 + 双数组单元的字节区间。
pub struct GramDb {
    bytes: Mmap,
    /// 双数组镜像起点在映射里的字节偏移。
    array_at: usize,
    /// 单元数（4 字节一单元）。
    array_units: usize,
}

/// `common_prefix_search` 的一条命中：值与命中键的字节长度（不是码点数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GramMatch {
    pub value: i32,
    pub length: usize,
}

static SHARED: OnceLock<Mutex<LruCache<PathBuf, Option<Arc<GramDb>>>>> = OnceLock::new();
const SHARED_CACHE_CAPACITY: usize = 4;

impl GramDb {
    /// 文件缺失、过短、魔数错误或数组越界时返回 `None`；调用方按无模型运行，
    /// 与缺少 `msime-bigram.bin` 时的处理一致。
    #[allow(unsafe_code)]
    pub fn load(path: &Path) -> Option<GramDb> {
        if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
            return None;
        }
        let mut file = crate::paths::open_file_no_follow(path).ok()?;
        let size = file.metadata().ok()?.len() as usize;
        if size < HEADER_BYTES {
            return None;
        }
        // 头 44 字节先读出来校验，校验过了才把整文件映射上：魔数不对的文件连映射都不建。
        use std::io::Read;
        let mut header = [0u8; HEADER_BYTES];
        file.read_exact(&mut header).ok()?;
        if &header[FORMAT_OFFSET..FORMAT_OFFSET + FORMAT_MAGIC.len()] != FORMAT_MAGIC {
            return None;
        }
        let array_units = u32::from_le_bytes(
            header[ARRAY_SIZE_OFFSET..ARRAY_SIZE_OFFSET + 4]
                .try_into()
                .expect("four size bytes"),
        ) as usize;
        let offset = i32::from_le_bytes(
            header[OFFSET_PTR_OFFSET..OFFSET_PTR_OFFSET + 4]
                .try_into()
                .expect("four offset bytes"),
        );
        if offset < 0 || array_units > MAX_UNITS {
            return None;
        }
        let array_at = OFFSET_PTR_OFFSET + offset as usize;
        if array_at > size || array_units > (size - array_at) / UNIT_BYTES {
            return None;
        }
        // SAFETY: 与 `NgramTable::load` 同一份映射契约——`.gram` 由资源包以
        // `.incoming` 改名安装，文件字节在映射存活期间不变；镜像边界上面已校验。
        let bytes = unsafe { MmapOptions::new().map(&file) }.ok()?;
        Some(GramDb {
            bytes,
            array_at,
            array_units,
        })
    }

    /// 按路径共享模型实例；缺失或非法的文件同样被记住（`None`），不会在每次
    /// `set_sentence_association` 后重复探测。与 `NgramTable::shared` 同一范式。
    pub fn shared(path: &Path) -> Option<Arc<GramDb>> {
        let mut loaded = SHARED
            .get_or_init(|| {
                Mutex::new(LruCache::new(
                    NonZeroUsize::new(SHARED_CACHE_CAPACITY).unwrap(),
                ))
            })
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = loaded.get(path) {
            return cached.clone();
        }
        let db = GramDb::load(path).map(Arc::new);
        loaded.put(path.to_path_buf(), db.clone());
        db
    }

    /// 摘掉指定路径的缓存引用（设置侧删除模型包后调用）；仍被会话持有的实例
    /// 由 `Arc` 兜底，最后一个引用松开才真正 unmap，被删的 `.gram` 随之可物理删除。
    /// 资源包卸载通过 Host API 调用；活跃会话的引用仍保留到会话重建。
    pub fn evict_shared(path: &Path) {
        if let Some(cache) = SHARED.get() {
            cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop(path);
        }
    }

    #[cfg(test)]
    fn unit_count(&self) -> usize {
        self.array_units
    }

    /// 越界返回 `None` 而不是 panic：损坏的镜像最多损失一次搭配命中。
    fn unit(&self, index: usize) -> Option<u32> {
        if index >= self.array_units {
            return None;
        }
        let at = self.array_at + index * UNIT_BYTES;
        Some(u32::from_le_bytes(
            self.bytes[at..at + UNIT_BYTES]
                .try_into()
                .expect("four unit bytes"),
        ))
    }

    // ---- darts 双数组（darts-clone 0.32）的只读部分 --------------------------

    /// `traverse`：从 `node_pos` 出发按 `key` 逐字节走转移，走完返回落点。
    /// 中途标签不符或越界都返回 `None`。
    fn traverse(&self, key: &[u8], mut node_pos: usize) -> Option<usize> {
        let mut id = node_pos;
        let mut unit = self.unit(id)?;
        for &byte in key {
            id ^= (offset_of(unit) as usize) ^ usize::from(byte);
            unit = self.unit(id)?;
            if label_of(unit) != u32::from(byte) {
                return None;
            }
            node_pos = id;
        }
        Some(node_pos)
    }

    /// `commonPrefixSearch`：从 `node_pos` 出发沿 `key` 走，每个节点若有 `\0`
    /// 叶孩子就记录一条命中（值 = 叶单元的 value，长度 = 已匹配键的字节数）。
    fn common_prefix_search(&self, key: &[u8], mut node_pos: usize, results: &mut Vec<GramMatch>) {
        let mut unit = match self.unit(node_pos) {
            Some(unit) => unit,
            None => return,
        };
        node_pos ^= offset_of(unit) as usize;
        for (index, &byte) in key.iter().enumerate() {
            node_pos ^= usize::from(byte);
            let Some(next) = self.unit(node_pos) else {
                return;
            };
            unit = next;
            if label_of(unit) != u32::from(byte) {
                return;
            }
            node_pos ^= offset_of(unit) as usize;
            if has_leaf(unit) && results.len() < MAX_RESULTS {
                if let Some(leaf) = self.unit(node_pos) {
                    results.push(GramMatch {
                        value: leaf as i32 & 0x7FFF_FFFF,
                        length: index + 1,
                    });
                }
            }
        }
    }

    /// `GramDb::Lookup`：先 `traverse` 整段上下文，再从落点做词首的公共前缀查找。
    fn lookup(&self, context_encoded: &[u8], word_encoded: &[u8], results: &mut Vec<GramMatch>) {
        if context_encoded.is_empty() {
            return;
        }
        let Some(node_pos) = self.traverse(context_encoded, 0) else {
            return;
        };
        self.common_prefix_search(word_encoded, node_pos, results);
    }

    /// `Octagram::Query`。`context`、`word` 都是原始 UTF-8；`is_rear` 对应句尾
    /// 词的「词+$」rear 项。返回值是自然对数域的搭配强度，未命中为
    /// `config.non_collocation_penalty`。
    pub fn query(&self, context: &str, word: &str, is_rear: bool, config: &GramConfig) -> f64 {
        let mut result = config.non_collocation_penalty;
        if context.is_empty() {
            return result;
        }
        // 上下文取尾部 collocation_max_length-1 个码点，词取头部同样数量，
        // 再各自 encode。collocation_len = 上下文现存码点数 + 命中键的码点数，
        // 决定吃 collocation 惩罚还是更狠的 weak 惩罚。
        let n = MAX_ENCODED_UNICODE
            .min(config.collocation_max_length.saturating_sub(1).max(0) as usize);
        if n < 1 {
            return result;
        }
        let context_tail = last_codepoints(context, n);
        let mut remaining_windows = codepoint_count(context_tail) as i32;
        let context_query = encode(context_tail);
        let word_head = first_codepoints(word, n);
        let word_head_len = codepoint_count(word_head) as i32;
        let word_query = encode(word_head);

        let mut context_ptr = 0usize;
        let mut matches = Vec::with_capacity(MAX_RESULTS);
        while remaining_windows > 0 && context_ptr < context_query.len() {
            let at_window_start = context_ptr == 0;
            matches.clear();
            self.lookup(&context_query[context_ptr..], &word_query, &mut matches);
            for m in &matches {
                let scaled = scale_value(m.value);
                let match_len = unicode_length(&word_query, m.length) as i32;
                let collocation_len = remaining_windows + match_len;
                let whole_query = at_window_start && m.length == word_query.len();
                let penalty = if collocation_len >= config.collocation_min_length || whole_query {
                    config.collocation_penalty
                } else {
                    config.weak_collocation_penalty
                };
                result = result.max(scaled + penalty);
            }
            remaining_windows -= 1;
            context_ptr = next_unicode(&context_query, context_ptr);
        }

        if is_rear && word_head_len == codepoint_count(word) as i32 {
            // 句尾标记只在「词未被头部截断」时查：截过的词查 $ 无意义。
            matches.clear();
            self.lookup(&word_query, b"$", &mut matches);
            if let Some(first) = matches.first() {
                result = result.max(scale_value(first.value) + config.rear_penalty);
            }
        }
        result
    }
}

/// `scale_value`：负值不可能是合法频次分，一律按 -1 处理。
fn scale_value(value: i32) -> f64 {
    if value >= 0 {
        value as f64 / VALUE_SCALE
    } else {
        -1.0
    }
}

// ---- 双数组单元的位段（darts-clone 的 DoubleArrayUnit）------------------------

fn label_of(unit: u32) -> u32 {
    unit & 0x8000_00FF
}

fn has_leaf(unit: u32) -> bool {
    (unit >> 8) & 1 == 1
}

fn offset_of(unit: u32) -> u32 {
    (unit >> 10) << ((unit & 0x200) >> 6)
}

// ---- gram_encoding.cc 的移植 -------------------------------------------------

/// 读一个原始 UTF-8 码点并推进 `i`。续字节缺位或非法序列按 Latin-1 单字节
/// 兜底：搭配查询遇到乱码最多损失一次命中，不值得为它抛错。
fn next_codepoint(text: &str, i: &mut usize) -> u32 {
    let bytes = text.as_bytes();
    let lead = bytes[*i];
    *i += 1;
    // 续字节：只在确实存在且形态合法时消费，值按需拼接。
    let take = |i: &mut usize| -> u32 {
        if *i < bytes.len() && (bytes[*i] & 0xC0) == 0x80 {
            let b = bytes[*i];
            *i += 1;
            u32::from(b & 0x3F)
        } else {
            0
        }
    };
    if lead < 0x80 {
        return u32::from(lead);
    }
    if (lead & 0xE0) == 0xC0 {
        return (u32::from(lead & 0x1F) << 6) | take(i);
    }
    if (lead & 0xF0) == 0xE0 {
        let high = take(i);
        let low = take(i); // 两次调用必须先后求值，不能写进同一个表达式
        return (u32::from(lead & 0x0F) << 12) | (high << 6) | low;
    }
    if (lead & 0xF8) == 0xF0 {
        let b1 = take(i);
        let b2 = take(i);
        let b3 = take(i);
        return (u32::from(lead & 0x07) << 18) | (b1 << 12) | (b2 << 6) | b3;
    }
    u32::from(lead)
}

pub fn encode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let u = next_codepoint(text, &mut i);
        if u < 0x80 {
            // 编码串里不允许出现 NUL（历史格式按 C 字符串用），0 被转成 0xE0 占位。
            out.push(if u == 0 { 0xE0 } else { u as u8 });
        } else if (0x4000..0xA000).contains(&u) {
            // CJK 常用区压成 2 字节：首字节平移进 0x4080-0xA07F，避开 ASCII 与
            // 0xE0 变长前缀。
            if (u & 0xFF) == 0 {
                out.push(0xE1);
                out.push(((u >> 8) + 0x40) as u8);
            } else {
                out.push(((u >> 8) + 0x40) as u8);
                out.push((u & 0xFF) as u8);
            }
        } else {
            let mut u = u;
            let mut bits = 32i32;
            while bits > 0 && (u & 0xFE00_0000) == 0 {
                bits -= 7;
                u <<= 7;
            }
            let mut bytes_to_encode = (bits + 6) / 7;
            out.push(0xE0 | bytes_to_encode as u8);
            while bytes_to_encode > 0 {
                bytes_to_encode -= 1;
                out.push((((u >> 25) & 0x7F) | 0x80) as u8);
                u <<= 7;
            }
        }
    }
    out
}

/// 编码串上的游标推进（`advance`）：一个编码码点占 1 字节（ASCII）、2 字节
/// （CJK 常用区）或 1+首字节低 4 位（扩展区变长）。越界时停在末尾。
fn next_unicode(encoded: &[u8], pos: usize) -> usize {
    let Some(&first) = encoded.get(pos) else {
        return pos;
    };
    let step = if (first & 0x80) == 0 {
        1
    } else if (first & 0xF0) == 0xE0 {
        usize::from(first & 0x0F) + 1
    } else {
        2
    };
    (pos + step).min(encoded.len())
}

/// 编码串前 `byte_length` 字节里的码点数（`unicode_length`）。
fn unicode_length(encoded: &[u8], byte_length: usize) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < byte_length && i < encoded.len() {
        i = next_unicode(encoded, i);
        len += 1;
    }
    len
}

/// 原始 UTF-8 串的码点数与首/尾 n 个码点。
pub fn codepoint_count(text: &str) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < text.len() {
        next_codepoint(text, &mut i);
        n += 1;
    }
    n
}

pub fn first_codepoints(text: &str, n: usize) -> &str {
    let mut seen = 0;
    let mut i = 0;
    while i < text.len() && seen < n {
        next_codepoint(text, &mut i);
        seen += 1;
    }
    &text[..i]
}

pub fn last_codepoints(text: &str, n: usize) -> &str {
    let total = codepoint_count(text);
    if total <= n {
        return text;
    }
    let mut skip = total - n;
    let mut i = 0;
    while i < text.len() && skip > 0 {
        next_codepoint(text, &mut i);
        skip -= 1;
    }
    &text[i..]
}

#[cfg(test)]
pub(in crate::lattice) mod fixture {
    // ---- 测试侧的双数组构造器 ------------------------------------------------
    //
    // 只为 fixture 存在：把「编码键 -> 值」排成一棵字节前缀树，再按 darts-clone
    // 的布局规则落成单元数组。每个节点试探最小可用 offset（<256，免移位标志位），
    // 子槽 = 节点下标 ^ offset ^ 子标签；键的末尾以 \0 叶单元存值，父单元置
    // has_leaf 位。布局合法性的判据是读取端能逐项查回。

    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::{FORMAT_MAGIC, HEADER_BYTES, UNIT_BYTES};

    #[derive(Default)]
    pub struct FixtureNode {
        children: BTreeMap<u8, FixtureNode>,
        value: Option<i32>,
    }

    impl FixtureNode {
        pub fn insert(&mut self, key: &[u8], value: i32) {
            let mut node = self;
            for &byte in key {
                node = node.children.entry(byte).or_default();
            }
            node.value = Some(value);
        }
    }

    /// 把 `entries` 的（编码键, 值）写成一份合法的 `.gram` 文件字节。
    pub fn build_gram(entries: &[(Vec<u8>, i32)]) -> Vec<u8> {
        let mut root = FixtureNode::default();
        for (key, value) in entries {
            assert!(!key.is_empty());
            root.insert(key, *value);
        }
        // BFS 布局：每个节点带自己的下标、入边标签与挑好的 offset。
        struct Layout {
            node: FixtureNode,
            index: usize,
            label: u32,
            offset: u32,
        }
        let mut nodes = vec![Layout {
            node: root,
            index: 0,
            label: 0,
            offset: 0,
        }];
        let mut units: BTreeMap<usize, u32> = BTreeMap::new();
        let mut used: std::collections::HashSet<usize> = [0].into_iter().collect();
        let mut frontier = 0;
        while frontier < nodes.len() {
            let node_index = nodes[frontier].index;
            let (labels, has_leaf_child) = {
                let node = &nodes[frontier].node;
                (
                    node.children.keys().copied().collect::<Vec<u8>>(),
                    node.value.is_some(),
                )
            };
            // 需要的槽位：每个子标签一个；有值时另加 label-0 的叶槽。
            let mut needed: Vec<u8> = labels.clone();
            if has_leaf_child {
                needed.push(0);
            }
            let mut offset = 1u32;
            let slots = loop {
                assert!(offset < 256, "fixture trie too dense for a small offset");
                let slots: Vec<usize> = needed
                    .iter()
                    .map(|&label| node_index ^ offset as usize ^ usize::from(label))
                    .collect();
                let distinct = slots.iter().collect::<std::collections::HashSet<_>>();
                if distinct.len() == slots.len() && slots.iter().all(|slot| !used.contains(slot)) {
                    break slots;
                }
                offset += 1;
            };
            nodes[frontier].offset = offset;
            // 叶槽（label 0）与子槽一并占用；叶单元 = MSB | value。
            for (&label, &slot) in needed.iter().zip(&slots) {
                used.insert(slot);
                if label == 0 {
                    units.insert(
                        slot,
                        0x8000_0000 | nodes[frontier].node.value.unwrap() as u32,
                    );
                }
            }
            for (&label, &slot) in labels.iter().zip(&slots[..labels.len()]) {
                let child = std::mem::take(nodes[frontier].node.children.get_mut(&label).unwrap());
                nodes.push(Layout {
                    node: child,
                    index: slot,
                    label: u32::from(label),
                    offset: 0,
                });
            }
            frontier += 1;
        }
        // 写每个节点的单元：offset<<10 | has_leaf<<8 | 入边标签。
        for node in &nodes {
            let has_leaf = u32::from(node.node.value.is_some());
            units.insert(
                node.index,
                (node.offset << 10) | (has_leaf << 8) | node.label,
            );
        }

        let array_units = used.iter().copied().max().unwrap_or(0) + 1;
        let mut bytes = Vec::with_capacity(HEADER_BYTES + array_units * UNIT_BYTES);
        let mut format = [0u8; 32];
        format[..FORMAT_MAGIC.len()].copy_from_slice(FORMAT_MAGIC);
        bytes.extend_from_slice(&format);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // checksum 沿袭上游不校验
        bytes.extend_from_slice(&(array_units as u32).to_le_bytes());
        bytes.extend_from_slice(&4i32.to_le_bytes()); // OffsetPtr：相对字段偏移
        for index in 0..array_units {
            bytes.extend_from_slice(&units.get(&index).copied().unwrap_or(0).to_le_bytes());
        }
        bytes
    }

    pub fn gram_file(dir: &Path, name: &str, entries: &[(Vec<u8>, i32)]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, build_gram(entries)).unwrap();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    // ---- encode / 码点工具 ---------------------------------------------------

    #[test]
    fn encode_matches_gram_encoding() {
        // ASCII 一字节。
        assert_eq!(encode("abc"), b"abc");
        // NUL 占位 0xE0。
        assert_eq!(encode("\0"), vec![0xE0]);
        // CJK 常用区两字节：中 U+4E2D -> [0x8E, 0x2D]，国 U+56FD -> [0x96, 0xFD]。
        assert_eq!(encode("中国"), vec![0x8E, 0x2D, 0x96, 0xFD]);
        // 低位为 0 的 CJK：一 U+4E00 -> [0xE1, 0x8E]。
        assert_eq!(encode("一"), vec![0xE1, 0x8E]);
        // 常用区之外（0xA000 边界、扩展区）走变长：é U+00E9 -> 0xE2 前缀 + 两个 7bit 组。
        assert_eq!(encode("é"), vec![0xE2, 0x8E, 0xC8]);
        // emoji U+1F600：3 个 7bit 组。
        let emoji = encode("😀");
        assert_eq!(emoji[0], 0xE0 | 3);
        assert_eq!(emoji.len(), 4);
    }

    /// 真实 `.gram` 的人工验证探针：`GRAM_PROBE_PATH=<file>` 指向资源锁里的
    /// 八股文紧凑模型，校验模型身份并对照独立读取器的分值。
    #[test]
    #[ignore = "需要锁定的八股文紧凑模型：GRAM_PROBE_PATH=<path> cargo test -p msime-engine --lib real_gram -- --ignored"]
    fn real_gram_loads_and_queries() {
        use sha2::{Digest, Sha256};

        let path = std::env::var_os("GRAM_PROBE_PATH")
            .expect("手动探针需要 GRAM_PROBE_PATH 指定锁定的八股文紧凑模型");
        let db = GramDb::load(Path::new(&path)).expect("八股文紧凑模型能加载");
        let lock: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../resources/grammar-models.lock.json"
        ))
        .expect("语法模型资源锁能解析");
        let model = lock["models"]
            .as_array()
            .expect("资源锁包含模型列表")
            .iter()
            .find(|model| model["id"] == "baguwen-compact")
            .expect("资源锁包含八股文紧凑模型");
        let artifact = &model["artifacts"][0];
        assert_eq!(
            db.bytes.len() as u64,
            artifact["size"].as_u64().expect("资源锁包含模型大小"),
            "模型大小必须与锁定的八股文紧凑模型一致"
        );
        assert_eq!(
            hex::encode(Sha256::digest(&db.bytes[..])),
            artifact["sha256"].as_str().expect("资源锁包含模型 SHA-256"),
            "模型 SHA-256 必须与锁定的八股文紧凑模型一致"
        );

        // 固定 darts-clone e40ce4627526985a7767444b6ed6893ab6ff8983 的独立读取结果。
        // 模型分值以 1/10000 为步长，1e-9 容差只容纳浮点舍入。
        let config = GramConfig::default();
        for (context, word, expected_units) in [
            ("一名", "医生", -40_443),
            ("一名", "一生", -38_286),
            ("北京", "大学", -3_926),
            ("北京", "大雪", -26_763),
        ] {
            let expected = f64::from(expected_units) / 10_000.0;
            let actual = db.query(context, word, true, &config);
            eprintln!("{context}+{word}={actual:.4}，参考={expected:.4}");
            assert!(
                (actual - expected).abs() < 1e-9,
                "{context}+{word} 的分值应为 {expected:?}，实际 {actual:?}"
            );
        }
    }

    #[test]
    fn encoded_cursor_helpers() {
        let enc = encode("中国ab一");
        // 中[2] 国[2] a[1] b[1] 一[2]
        assert_eq!(unicode_length(&enc, enc.len()), 5);
        assert_eq!(next_unicode(&enc, 0), 2);
        assert_eq!(next_unicode(&enc, 2), 4);
        assert_eq!(next_unicode(&enc, 4), 5);
        assert_eq!(next_unicode(&enc, 6), 8);
        assert_eq!(next_unicode(&enc, 8), 8, "停在末尾不越界");
        // 前缀字节内的码点数（前缀查找的 match.length 语义）。
        assert_eq!(unicode_length(&enc, 4), 2);
        assert_eq!(unicode_length(&enc, 3), 2, "截在半个码点处也算一个码点");
    }

    #[test]
    fn raw_codepoint_helpers() {
        assert_eq!(codepoint_count("中国人民"), 4);
        assert_eq!(first_codepoints("中国人民", 2), "中国");
        assert_eq!(last_codepoints("中国人民", 2), "人民");
        assert_eq!(last_codepoints("中国", 8), "中国");
        assert_eq!(first_codepoints("ab中", 2), "ab");
    }

    // ---- 装载与回退 -----------------------------------------------------------

    #[test]
    fn load_rejects_missing_and_broken_files() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        assert!(GramDb::load(&d.join("absent.gram")).is_none(), "缺失文件");

        let short = d.join("short.gram");
        std::fs::write(&short, b"Rime::Gram").unwrap();
        assert!(GramDb::load(&short).is_none(), "不足头部");

        let magic = d.join("magic.gram");
        std::fs::write(&magic, [b"Xime::Grammar/".as_slice(), &[0; 60]].concat()).unwrap();
        assert!(GramDb::load(&magic).is_none(), "魔数不符");

        // 魔数对但双数组镜像越界。
        let oob = d.join("oob.gram");
        let mut bytes = Vec::new();
        let mut format = [0u8; 32];
        format[..FORMAT_MAGIC.len()].copy_from_slice(FORMAT_MAGIC);
        bytes.extend_from_slice(&format);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1000u32.to_le_bytes());
        bytes.extend_from_slice(&4i32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        std::fs::write(&oob, &bytes).unwrap();
        assert!(GramDb::load(&oob).is_none(), "镜像越界");

        // 负偏移。
        let neg = d.join("neg.gram");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&format);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(-4i32).to_le_bytes());
        bytes.extend_from_slice(&[0; 64]);
        std::fs::write(&neg, &bytes).unwrap();
        assert!(GramDb::load(&neg).is_none(), "负偏移");
    }

    // ---- 查询语义 -------------------------------------------------------------

    /// 单条「上下文+词首」记录的模型：命中给 collocation_penalty 加成。
    #[test]
    fn query_scores_a_known_collocation() {
        let dir = tempfile::tempdir().unwrap();
        let config = GramConfig::default();
        let key = [encode("中国"), encode("人民")].concat();
        // ln(频次)*10000：取 12.0 -> 120000，比 collocation_penalty 高得多。
        let path = gram_file(dir.path(), "m.gram", &[(key, 120000)]);
        let db = GramDb::load(&path).expect("fixture loads");
        assert!(db.unit_count() > 1);

        let score = db.query("我爱中国", "人民", false, &config);
        // 窗口从尾部 5 码点「爱中国」起，逐后缀试「国」命中：scaled 12 + (-14) = -2。
        assert!(
            (score - (-2.0)).abs() < 1e-6,
            "尾部窗口命中后的分，实际 {score}"
        );

        let miss = db.query("我爱中国", "首都", false, &config);
        assert_eq!(miss, config.non_collocation_penalty, "未命中给常数分");
        assert_eq!(
            db.query("", "人民", false, &config),
            config.non_collocation_penalty
        );
    }

    #[test]
    fn query_uses_the_rear_entry_for_sentence_final_words() {
        let dir = tempfile::tempdir().unwrap();
        let config = GramConfig::default();
        let path = gram_file(
            dir.path(),
            "rear.gram",
            &[([encode("人民"), encode("$")].concat(), 160000)],
        );
        let db = GramDb::load(&path).expect("fixture loads");
        // 词+$ 命中：scaled 16 + (-18) = -2，比 non_collocation -6 高。
        assert!((db.query("无关上下文", "人民", true, &config) + 2.0).abs() < 1e-6);
        // 非句尾不查 $。
        assert_eq!(
            db.query("无关上下文", "人民", false, &config),
            config.non_collocation_penalty
        );
        // 词被头部截断时（词长 > collocation_max_length-1）不查 $。
        let long_word = "人民万岁万岁万万岁";
        assert_eq!(
            db.query("无关", long_word, true, &config),
            config.non_collocation_penalty
        );
    }

    #[test]
    fn query_bounds_extreme_configured_window_lengths() {
        let dir = tempfile::tempdir().unwrap();
        let path = gram_file(
            dir.path(),
            "window-limits.gram",
            &[
                ([encode("名"), encode("医")].concat(), 180000),
                ([encode("一名"), encode("医")].concat(), 200000),
                ([encode("医生"), encode("$")].concat(), 190000),
            ],
        );
        let db = GramDb::load(&path).expect("合成模型能加载");
        for (length, expected) in [
            (i32::MIN, -6.0),
            (i32::MIN + 1, -6.0),
            (-5, -6.0),
            (0, -6.0),
            (1, -6.0),
            (2, 4.0),
            (3, 6.0),
            (6, 6.0),
            (9, 6.0),
            (i32::MAX, 6.0),
        ] {
            let config = GramConfig {
                collocation_max_length: length,
                ..GramConfig::default()
            };
            for is_rear in [false, true] {
                assert_eq!(
                    db.query("一名", "医生", is_rear, &config),
                    expected,
                    "窗口长度={length}，句尾={is_rear}"
                );
            }
        }
    }

    #[test]
    fn query_takes_the_best_window() {
        let dir = tempfile::tempdir().unwrap();
        let config = GramConfig::default();
        let entries = [
            ([encode("国"), encode("人民")].concat(), 120000),
            ([encode("我爱中国"), encode("人民")].concat(), 180000),
        ];
        let path = gram_file(dir.path(), "w.gram", &entries);
        let db = GramDb::load(&path).expect("fixture loads");
        // 首窗口「我爱中国」全键命中 18 + (-14) = 4，压过尾窗「国」的 12-14 = -2。
        let score = db.query("我爱中国", "人民", false, &config);
        assert!(
            (score - 4.0).abs() < 1e-6,
            "取所有窗口的最大值，实际 {score}"
        );
    }

    #[test]
    fn load_rejects_empty_array_outside_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = vec![0u8; HEADER_BYTES];
        bytes[..FORMAT_MAGIC.len()].copy_from_slice(FORMAT_MAGIC);
        bytes[OFFSET_PTR_OFFSET..HEADER_BYTES].copy_from_slice(&1000i32.to_le_bytes());
        let path = dir.path().join("outside.gram");
        std::fs::write(&path, bytes).unwrap();
        assert!(GramDb::load(&path).is_none());
    }

    #[test]
    fn shared_caches_hits_and_misses() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.gram");
        assert!(GramDb::shared(&missing).is_none());
        let path = gram_file(
            dir.path(),
            "s.gram",
            &[([encode("a"), encode("b")].concat(), 50000)],
        );
        let first = GramDb::shared(&path).expect("loads");
        let second = GramDb::shared(&path).expect("same instance");
        assert!(Arc::ptr_eq(&first, &second));
        GramDb::evict_shared(&path);
        let third = GramDb::shared(&path).expect("reloads after evict");
        assert!(!Arc::ptr_eq(&first, &third));
        GramDb::evict_shared(&path);
    }
}
