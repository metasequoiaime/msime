//! 当前输入方案。十个具体方案共用同样的五个操作：用枚举，不用 trait 对象。

use std::sync::Arc;

use crate::cantonese::{CantoneseCandidate, CantoneseScheme, Inventory};
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::japanese::JapaneseRomajiScheme;
use crate::korean::KoreanScheme;
use crate::language_dictionary::LanguageDictionary;
use crate::quanpin::QuanpinScheme;
use crate::shuangpin::profile::profile;
use crate::shuangpin::ShuangpinScheme;
use crate::stroke::StrokeScheme;
use crate::tibetan::TibetanScheme;
use crate::types::{QueryRequest, SchemeKey, SchemeType, ShuangpinProfileKind, WordItem};
use crate::vietnamese::{InputMethod, ToneStyle, VietnameseScheme};
use crate::wubi::scheme::WubiScheme;
use crate::zhuyin::scheme::ZhuyinScheme;

pub enum Scheme {
    Quanpin(QuanpinScheme),
    Shuangpin(ShuangpinScheme),
    Wubi(WubiScheme),
    Japanese(JapaneseRomajiScheme),
    Korean(KoreanScheme),
    Cantonese(CantoneseScheme),
    /// Boxed: the editor's state is several times the size of every other scheme's.
    Zhuyin(Box<ZhuyinScheme>),
    Vietnamese(VietnameseScheme),
    Tibetan(TibetanScheme),
    Stroke(StrokeScheme),
}

impl Scheme {
    /// ime_session.cpp:371-386; the profile only matters for shuangpin, the input method and tone style only for Vietnamese, the syllable inventory only for Cantonese and the open `msime-zhuyin.db` only for Zhuyin. Neither can be built without its dictionary (`LANGUAGE_DICTIONARY_UNAVAILABLE`), which exists once the scheme has been activated.
    pub fn new(
        scheme: SchemeType,
        profile_kind: ShuangpinProfileKind,
        vietnamese_method: InputMethod,
        vietnamese_style: ToneStyle,
        cantonese_inventory: Option<Arc<Inventory>>,
        zhuyin_dictionary: Option<LanguageDictionary>,
    ) -> Result<Self> {
        let unavailable = || EngineError::failed(diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE);
        Ok(match scheme {
            SchemeType::Quanpin => Self::Quanpin(QuanpinScheme::new()),
            SchemeType::Shuangpin => Self::Shuangpin(ShuangpinScheme::new(profile(profile_kind))),
            SchemeType::Wubi => Self::Wubi(WubiScheme::new()),
            SchemeType::JapaneseRomaji => Self::Japanese(JapaneseRomajiScheme::new()),
            SchemeType::Korean => Self::Korean(KoreanScheme::new()),
            SchemeType::Cantonese => {
                let inventory = cantonese_inventory.ok_or_else(unavailable)?;
                Self::Cantonese(CantoneseScheme::new(inventory))
            }
            SchemeType::Zhuyin => {
                let dictionary = zhuyin_dictionary.ok_or_else(unavailable)?;
                Self::Zhuyin(Box::new(ZhuyinScheme::new(dictionary)))
            }
            SchemeType::Vietnamese => {
                Self::Vietnamese(VietnameseScheme::new(vietnamese_method, vietnamese_style))
            }
            SchemeType::Tibetan => Self::Tibetan(TibetanScheme::new()),
            // 笔画方案自己不持有 `msime-stroke.db`：词典留在 registry，查询时按请求读。
            SchemeType::Stroke => Self::Stroke(StrokeScheme::new()),
        })
    }

    pub fn scheme_type(&self) -> SchemeType {
        match self {
            Self::Quanpin(_) => SchemeType::Quanpin,
            Self::Shuangpin(_) => SchemeType::Shuangpin,
            Self::Wubi(_) => SchemeType::Wubi,
            Self::Japanese(_) => SchemeType::JapaneseRomaji,
            Self::Korean(_) => SchemeType::Korean,
            Self::Cantonese(_) => SchemeType::Cantonese,
            Self::Zhuyin(_) => SchemeType::Zhuyin,
            Self::Vietnamese(_) => SchemeType::Vietnamese,
            Self::Tibetan(_) => SchemeType::Tibetan,
            Self::Stroke(_) => SchemeType::Stroke,
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Quanpin(scheme) => scheme.reset(),
            Self::Shuangpin(scheme) => scheme.reset(),
            Self::Wubi(scheme) => scheme.reset(),
            Self::Japanese(scheme) => scheme.reset(),
            Self::Korean(scheme) => scheme.reset(),
            Self::Cantonese(scheme) => scheme.reset(),
            Self::Zhuyin(scheme) => scheme.reset(),
            Self::Vietnamese(scheme) => scheme.reset(),
            Self::Tibetan(scheme) => scheme.reset(),
            Self::Stroke(scheme) => scheme.reset(),
        }
    }

    pub fn handle_key(&mut self, key: SchemeKey) {
        match self {
            Self::Quanpin(scheme) => scheme.handle_key(key),
            Self::Shuangpin(scheme) => scheme.handle_key(key),
            Self::Wubi(scheme) => scheme.handle_key(key),
            Self::Japanese(scheme) => scheme.handle_key(key),
            Self::Korean(scheme) => scheme.handle_key(key),
            Self::Cantonese(scheme) => scheme.handle_key(key),
            // The session drives Zhuyin through `ImeSession::handle_zhuyin_key`, which reports whether the editor claimed a key and whether reading `msime-zhuyin.db` failed; a scheme key reaching it here changes nothing.
            Self::Zhuyin(_) => {}
            Self::Vietnamese(scheme) => scheme.handle_key(key),
            Self::Tibetan(scheme) => scheme.handle_key(key),
            Self::Stroke(scheme) => scheme.handle_key(key),
        }
    }

    pub fn build_request(&self) -> QueryRequest {
        match self {
            Self::Quanpin(scheme) => scheme.build_request(),
            Self::Shuangpin(scheme) => scheme.build_request(),
            Self::Wubi(scheme) => scheme.build_request(),
            Self::Japanese(scheme) => scheme.build_request(),
            Self::Korean(scheme) => scheme.build_request(),
            Self::Cantonese(scheme) => scheme.build_request(),
            Self::Zhuyin(scheme) => scheme.build_request(),
            Self::Vietnamese(scheme) => scheme.build_request(),
            Self::Tibetan(scheme) => scheme.build_request(),
            Self::Stroke(scheme) => scheme.build_request(),
        }
    }

    #[allow(dead_code)]
    pub fn preedit(&self) -> String {
        match self {
            Self::Quanpin(scheme) => scheme.preedit(),
            Self::Shuangpin(scheme) => scheme.preedit(),
            Self::Wubi(scheme) => scheme.preedit(),
            Self::Japanese(scheme) => scheme.preedit(),
            Self::Korean(scheme) => scheme.preedit(),
            Self::Cantonese(scheme) => scheme.preedit(),
            Self::Zhuyin(scheme) => scheme.preedit(),
            Self::Vietnamese(scheme) => scheme.preedit(),
            Self::Tibetan(scheme) => scheme.preedit(),
            Self::Stroke(scheme) => scheme.preedit(),
        }
    }

    /// Wubi, Cantonese and Stroke keep no case, so they only take the plain letters. Zhuyin keeps the caret at the end and has no host edit to take.
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        match self {
            Self::Quanpin(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Shuangpin(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Wubi(scheme) => scheme.set_raw_input(raw),
            Self::Japanese(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Korean(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Cantonese(scheme) => scheme.set_raw_input(raw),
            Self::Zhuyin(_) => {}
            Self::Vietnamese(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Tibetan(scheme) => scheme.set_raw_input(raw, raw_with_cases),
            Self::Stroke(scheme) => scheme.set_raw_input(raw),
        }
    }

    pub fn as_wubi_mut(&mut self) -> Option<&mut WubiScheme> {
        match self {
            Self::Wubi(scheme) => Some(scheme),
            _ => None,
        }
    }

    pub fn as_wubi(&self) -> Option<&WubiScheme> {
        match self {
            Self::Wubi(scheme) => Some(scheme),
            _ => None,
        }
    }

    pub fn as_zhuyin_mut(&mut self) -> Option<&mut ZhuyinScheme> {
        match self {
            Self::Zhuyin(scheme) => Some(scheme.as_mut()),
            _ => None,
        }
    }

    pub fn as_zhuyin(&self) -> Option<&ZhuyinScheme> {
        match self {
            Self::Zhuyin(scheme) => Some(scheme.as_ref()),
            _ => None,
        }
    }

    /// The `msime-zhuyin.db` connection a Zhuyin scheme holds, `None` for every other scheme.
    pub fn into_zhuyin_dictionary(self) -> Option<LanguageDictionary> {
        match self {
            Self::Zhuyin(scheme) => Some(scheme.into_dictionary()),
            _ => None,
        }
    }

    /// Takes the letters a Cantonese candidate covers out of the composition, as `CantoneseScheme::select`; returns whether letters are left composing. False, with nothing changed, for every other scheme.
    pub fn select_cantonese(&mut self, item: &WordItem) -> bool {
        let Self::Cantonese(scheme) = self else {
            return false;
        };
        scheme.select(&CantoneseCandidate {
            text: item.word.clone(),
            weight: item.weight,
            key: item.canonical_pinyin.clone(),
            syllables: item.canonical_pinyin.split(' ').count(),
            end: item.pinyin.len(),
        })
    }
}
