//! Zinnia 二进制模型（`recognizer.cpp` 的 `open` / `classify`）：每个字一个线性一对多 SVM。布局见 `docs/data-formats.md` §14；随包的 `handwriting-zh_CN.model`（Tegaki zh_CN 0.3）原样可读。
//!
//! Ported from zinnia (<https://github.com/taku910/zinnia> at 581faa8f), Copyright (c) 2005-2007 Taku Kudo, under its 3-clause BSD license, whose text ships with the product notices as `Zinnia-LICENSE.txt`.
//!
//! The model stays in the read-only mapping `recognizer::load` makes, as zinnia's `Mmap` did (recognizer.cpp:94,104): only the labels and each class's weight range are decoded at load, and the weights are read from the mapped bytes while scoring, so the 26.8 MB of weights are clean, file-backed pages the system can evict rather than heap held for the life of the process.

use std::cmp::Ordering;
use std::ops::Range;

use memmap2::Mmap;

use super::features::FeatureNode;

/// `DIC_MAGIC_ID`: the first word is this XOR the file size.
const MAGIC: u32 = 0x0EF7_1821;
/// `DIC_VERSION`.
const VERSION: u32 = 1;
const LABEL_BYTES: usize = 16;
const NODE_BYTES: usize = 8;

struct Class {
    label: String,
    bias: f32,
    /// Byte range of the class's `(index, value)` weight records in `Model::bytes`, the `-1` terminator excluded.
    nodes: Range<usize>,
}

pub(super) struct Model {
    /// The whole model file; the weights are decoded from it on every `classify`.
    bytes: Mmap,
    classes: Vec<Class>,
}

/// Why a model file was rejected. The caller reports every case as the reference's single "cannot open" error; the reason is kept for tests.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ModelError {
    Magic,
    Version(u32),
    Truncated,
    TrailingBytes,
    Label,
}

impl Model {
    pub(super) fn parse(bytes: Mmap) -> Result<Self, ModelError> {
        let classes = Self::parse_classes(&bytes)?;
        Ok(Self { bytes, classes })
    }

    fn parse_classes(bytes: &[u8]) -> Result<Vec<Class>, ModelError> {
        let mut reader = Reader { bytes, offset: 0 };
        let magic = reader.u32()?;
        if u64::from(magic ^ MAGIC) != bytes.len() as u64 {
            return Err(ModelError::Magic);
        }
        let version = reader.u32()?;
        if version != VERSION {
            return Err(ModelError::Version(version));
        }
        let count = reader.u32()? as usize;
        // Every class takes at least a label, a bias and a terminator, so a count the file cannot hold is rejected before allocating for it.
        if count > bytes.len() / (LABEL_BYTES + 4 + NODE_BYTES) {
            return Err(ModelError::Truncated);
        }
        let mut classes = Vec::with_capacity(count);
        for _ in 0..count {
            let label = reader.take(LABEL_BYTES)?;
            let end = label
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(LABEL_BYTES);
            let label = std::str::from_utf8(&label[..end])
                .map_err(|_| ModelError::Label)?
                .to_owned();
            let bias = f32::from_bits(reader.u32()?);
            let start = reader.offset;
            let end = loop {
                let node = reader.take(NODE_BYTES)?;
                if node_at(node, 0).index == -1 {
                    break reader.offset - NODE_BYTES;
                }
            };
            classes.push(Class {
                label,
                bias,
                nodes: start..end,
            });
        }
        if reader.offset != bytes.len() {
            return Err(ModelError::TrailingBytes);
        }
        Ok(classes)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// The `nbest` highest-scoring labels with their scores, best first. Equal scores put the class stored later in the file first: the reference breaks ties on the label pointer into its mapping.
    pub(super) fn classify(&self, features: &[FeatureNode], nbest: usize) -> Vec<(&str, f32)> {
        let mut scores: Vec<(usize, f32)> = self
            .classes
            .iter()
            .enumerate()
            .map(|(index, class)| {
                let score = f64::from(class.bias) + dot(&self.bytes[class.nodes.clone()], features);
                (index, score as f32)
            })
            .collect();
        let best_first = |left: &(usize, f32), right: &(usize, f32)| {
            right.1.total_cmp(&left.1).then(right.0.cmp(&left.0))
        };
        let nbest = nbest.min(scores.len());
        if nbest == 0 {
            return Vec::new();
        }
        if nbest < scores.len() {
            scores.select_nth_unstable_by(nbest - 1, best_first);
            scores.truncate(nbest);
        }
        scores.sort_unstable_by(best_first);
        scores
            .into_iter()
            .map(|(index, score)| (self.classes[index].label.as_str(), score))
            .collect()
    }
}

/// The `index`-th `(i32 index, f32 value)` little-endian weight record of `records`.
fn node_at(records: &[u8], index: usize) -> FeatureNode {
    let offset = index * NODE_BYTES;
    let word = |at: usize| {
        u32::from_le_bytes([
            records[at],
            records[at + 1],
            records[at + 2],
            records[at + 3],
        ])
    };
    FeatureNode {
        index: word(offset) as i32,
        value: f32::from_bits(word(offset + 4)),
    }
}

/// The sparse merge-join over two index-sorted lists, the weights decoded from their records as they are reached: `float` products summed in `double`.
fn dot(weights: &[u8], features: &[FeatureNode]) -> f64 {
    let count = weights.len() / NODE_BYTES;
    let (mut left, mut right) = (0, 0);
    let mut sum = 0.0_f64;
    while left < count && right < features.len() {
        let weight = node_at(weights, left);
        match weight.index.cmp(&features[right].index) {
            Ordering::Equal => {
                sum += f64::from(weight.value * features[right].value);
                left += 1;
                right += 1;
            }
            Ordering::Less => left += 1,
            Ordering::Greater => right += 1,
        }
    }
    sum
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], ModelError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ModelError::Truncated)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ModelError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, ModelError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A class to encode: label, bias and `(index, value)` weights.
    pub(in crate::handwriting) type TestClass<'a> = (&'a str, f32, &'a [(i32, f32)]);

    /// A model file in the zinnia layout, for tests that must not depend on the 26.8 MB shipped model.
    pub(in crate::handwriting) fn encode(classes: &[TestClass]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(VERSION.to_le_bytes());
        body.extend((classes.len() as u32).to_le_bytes());
        for (label, bias, weights) in classes {
            let mut padded = [0_u8; LABEL_BYTES];
            padded[..label.len()].copy_from_slice(label.as_bytes());
            body.extend(padded);
            body.extend(bias.to_le_bytes());
            for (index, value) in weights.iter().chain(&[(-1, 0.0)]) {
                body.extend(index.to_le_bytes());
                body.extend(value.to_le_bytes());
            }
        }
        let size = (body.len() + 4) as u32;
        let mut file = (size ^ MAGIC).to_le_bytes().to_vec();
        file.extend(body);
        file
    }

    /// Parse in-memory bytes through the same read-only mapping type `recognizer::load` produces, over anonymous memory.
    pub(in crate::handwriting) fn parse(bytes: &[u8]) -> Result<Model, ModelError> {
        let mut map = memmap2::MmapMut::map_anon(bytes.len()).expect("anonymous map");
        map.copy_from_slice(bytes);
        Model::parse(map.make_read_only().expect("read-only map"))
    }

    fn weights(model: &Model, class: usize) -> Vec<FeatureNode> {
        let records = &model.bytes[model.classes[class].nodes.clone()];
        (0..records.len() / NODE_BYTES)
            .map(|index| node_at(records, index))
            .collect()
    }

    fn features(items: &[(i32, f32)]) -> Vec<FeatureNode> {
        items
            .iter()
            .map(|&(index, value)| FeatureNode { index, value })
            .collect()
    }

    #[test]
    fn parses_labels_bias_and_weights() {
        let file = encode(&[("一", 0.5, &[(1, 2.0), (3, -1.0)]), ("十", -0.25, &[])]);
        let model = parse(&file).unwrap();
        assert_eq!(model.classes.len(), 2);
        assert_eq!(model.classes[0].label, "一");
        assert_eq!(model.classes[0].bias, 0.5);
        assert_eq!(weights(&model, 0), features(&[(1, 2.0), (3, -1.0)]));
        assert!(weights(&model, 1).is_empty());
        // The weights stay in the mapped file: each class holds only its byte range there.
        assert_eq!(model.classes[0].nodes.len(), 2 * NODE_BYTES);
        assert!(model.classes[1].nodes.is_empty());
    }

    #[test]
    fn rejects_broken_files() {
        let good = encode(&[("一", 0.5, &[(1, 2.0)])]);
        assert_eq!(parse(&[]).err(), Some(ModelError::Truncated));

        let mut extra = good.clone();
        extra.extend([0; 8]);
        assert_eq!(parse(&extra).err(), Some(ModelError::Magic));

        let mut version = good.clone();
        version[4] = 2;
        assert_eq!(parse(&version).err(), Some(ModelError::Version(2)));

        // Without its terminator the node walk runs off the end.
        let mut cut = good[..good.len() - 8].to_vec();
        let size = cut.len() as u32 ^ MAGIC;
        cut[..4].copy_from_slice(&size.to_le_bytes());
        assert_eq!(parse(&cut).err(), Some(ModelError::Truncated));

        // A count larger than the classes present: the last class is followed by nothing.
        let mut count = good.clone();
        count[8] = 2;
        assert_eq!(parse(&count).err(), Some(ModelError::Truncated));

        let mut label = good;
        label[12] = 0xFF;
        assert_eq!(parse(&label).err(), Some(ModelError::Label));
    }

    #[test]
    fn scores_are_bias_plus_sparse_dot() {
        let file = encode(&[
            ("a", 0.0, &[(0, 1.0), (2, 1.0)]),
            ("b", 1.0, &[(1, 5.0)]),
            ("c", 0.0, &[(2, 3.0), (7, 9.0)]),
        ]);
        let model = parse(&file).unwrap();
        let input = features(&[(0, 1.0), (2, 0.5), (5, 4.0)]);
        assert_eq!(
            model.classify(&input, 12),
            vec![("c", 1.5), ("a", 1.5), ("b", 1.0)]
        );
        assert_eq!(model.classify(&input, 1), vec![("c", 1.5)]);
        assert!(model.classify(&input, 0).is_empty());
    }
}
