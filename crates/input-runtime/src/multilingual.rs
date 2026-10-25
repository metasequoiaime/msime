//! 混合候选的展示与提交共用语言分段，不修改引擎的选择身份。

use crate::{CandidateEdge, EngineResult, InputEngine, Runtime, SelectionOutcome};
use msime_client_core::chinese_conversion::{convert_to_traditional, TraditionalVariant};
use msime_engine::multilingual::{Conversion, Language, MAX_INPUT_BYTES};

const SOURCE: u8 = msime_engine::CandidateSource::Multilingual as u8;

pub(crate) struct SelectedMultilingual {
    prefix: String,
    conversion: Conversion,
    edge: Option<(String, Language)>,
}

impl<E: InputEngine> Runtime<E> {
    /// 混合候选由共享层逐段输出，宿主不得再整行转换。
    pub fn set_multilingual_output(&mut self, traditional: bool, variant: TraditionalVariant) {
        if self.multilingual_output != (traditional, variant) {
            self.multilingual_output = (traditional, variant);
            self.candidates_epoch = self.candidates_epoch.wrapping_add(1);
        }
    }

    fn multilingual_conversion(&self, seat: usize) -> Option<&Conversion> {
        if self.cached.candidate_sources.get(seat) != Some(&SOURCE) {
            return None;
        }
        let composition = self.cached.multilingual_conversion.as_ref()?;
        let raw = &composition.raw_input;
        if raw.is_empty()
            || raw.len() > MAX_INPUT_BYTES
            || !raw.bytes().all(|value| value.is_ascii_lowercase())
            || raw != &self.cached.editing_text
            || self.cached.candidate_codes.get(seat) != Some(raw)
        {
            return None;
        }
        let conversion = &composition.conversion;
        let mut remaining = self.cached.candidates.get(seat)?.as_str();
        let mut end = 0;
        let first_language = conversion.segments.first()?.language;
        let mut mixed = false;
        for segment in &conversion.segments {
            if segment.input.start != end
                || segment.input.end <= end
                || segment.input.end > raw.len()
                || segment.text.is_empty()
            {
                return None;
            }
            remaining = remaining.strip_prefix(&segment.text)?;
            end = segment.input.end;
            mixed |= segment.language != first_language;
        }
        (end == raw.len() && remaining.is_empty() && mixed).then_some(conversion)
    }

    pub(crate) fn multilingual_candidate(&self, seat: usize, text: &str) -> String {
        if self.multilingual_output.0 {
            if let Some(conversion) = self.multilingual_conversion(seat) {
                return converted_segments(conversion, self.multilingual_output.1);
            }
        }
        text.to_owned()
    }

    pub(crate) fn capture_multilingual_selection(
        &mut self,
        seat: usize,
        edge: Option<CandidateEdge>,
    ) {
        self.selected_multilingual = self.multilingual_conversion(seat).and_then(|conversion| {
            let edge = match edge {
                Some(edge) => {
                    let mut segments = conversion.segments.iter().filter_map(|segment| {
                        let text = edge.character(&segment.text);
                        (!text.is_empty()).then_some((text.to_owned(), segment.language))
                    });
                    let (text, language) = match edge {
                        CandidateEdge::FirstHan => segments.next(),
                        CandidateEdge::LastHan => segments.next_back(),
                    }?;
                    Some((text, language))
                }
                None => None,
            };
            Some(SelectedMultilingual {
                prefix: self.phrase_prefix.clone(),
                conversion: conversion.clone(),
                edge,
            })
        });
    }

    pub(crate) fn commits_multilingual(&self) -> bool {
        self.facts.selection.is_some_and(|selection| {
            selection.source == SOURCE && selection.outcome == SelectionOutcome::Committed
        })
    }

    pub(crate) fn convert_multilingual_commit(&self, result: &mut EngineResult) {
        if !result.has_commit || !self.multilingual_output.0 {
            return;
        }
        let Some(selected) = &self.selected_multilingual else {
            return;
        };
        let Some(tail) = result
            .commit
            .strip_prefix(&selected.prefix)
            .and_then(|remaining| {
                remaining.strip_prefix(
                    selected
                        .edge
                        .as_ref()
                        .map_or(self.selected_text.as_str(), |(text, _)| text.as_str()),
                )
            })
        else {
            return;
        };
        let variant = self.multilingual_output.1;
        let mut converted = convert_to_traditional(&selected.prefix, variant);
        match &selected.edge {
            Some((text, Language::Chinese)) => {
                converted.push_str(&convert_to_traditional(text, variant))
            }
            Some((text, _)) => converted.push_str(text),
            None => converted.push_str(&converted_segments(&selected.conversion, variant)),
        }
        converted.push_str(tail);
        result.commit = converted;
    }
}

fn converted_segments(conversion: &Conversion, variant: TraditionalVariant) -> String {
    let mut converted = String::new();
    for segment in &conversion.segments {
        if segment.language == Language::Chinese {
            converted.push_str(&convert_to_traditional(&segment.text, variant));
        } else {
            converted.push_str(&segment.text);
        }
    }
    converted
}
