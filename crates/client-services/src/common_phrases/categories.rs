//! 常用语的本机分类；归属、名称和默认展开项与正文共用同一次锁。

use super::*;
use std::collections::HashMap;

pub const MAX_CATEGORIES: usize = 6;
const MAX_NAME_UTF16: usize = 20;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommonPhraseCategory {
    pub id: Uuid,
    pub name: String,
}

/// 备份用名称和正文关联，恢复到另一台设备时不沿用原设备的记录 UUID。
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommonPhraseCategoryBackup {
    pub id: Uuid,
    pub name: String,
    pub texts: Vec<String>,
}

pub(super) fn all() -> String {
    "all".into()
}

pub(super) fn is_all(value: &str) -> bool {
    value == "all"
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && crate::text::is_bounded_utf16(name, MAX_NAME_UTF16)
        && !name.chars().any(char::is_control)
        && !["全部", "未分类", "不展开"].contains(&name)
}

fn expanded_exists(document: &CommonPhrases, value: &str) -> bool {
    value.is_empty()
        || value == "all"
        || document
            .categories
            .iter()
            .any(|category| category.id.to_string() == value)
}

pub(super) fn validate(document: &CommonPhrases) -> Result<(), ()> {
    if document.categories.len() > MAX_CATEGORIES
        || !expanded_exists(document, &document.expanded_category)
    {
        return Err(());
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for category in &document.categories {
        if category.id.is_nil()
            || !valid_name(&category.name)
            || !ids.insert(category.id)
            || !names.insert(category.name.as_str())
        {
            return Err(());
        }
    }
    if document
        .phrases
        .iter()
        .any(|phrase| phrase.category.is_some_and(|id| !ids.contains(&id)))
    {
        return Err(());
    }
    Ok(())
}

fn check_name(
    document: &CommonPhrases,
    id: Option<Uuid>,
    name: &str,
) -> Result<(), CommonPhrasesError> {
    if !valid_name(name) {
        return Err(CommonPhrasesError::CategoryInvalid);
    }
    if document
        .categories
        .iter()
        .any(|category| Some(category.id) != id && category.name == name)
    {
        return Err(CommonPhrasesError::CategoryDuplicate);
    }
    Ok(())
}

impl CommonPhrasesStore {
    pub fn add_category(&self, name: &str) -> Result<CommonPhrases, CommonPhrasesError> {
        let name = name.trim();
        self.update(|document| {
            check_name(document, None, name)?;
            if document.categories.len() >= MAX_CATEGORIES {
                return Err(CommonPhrasesError::CategoryLimit);
            }
            document.categories.push(CommonPhraseCategory {
                id: Uuid::new_v4(),
                name: name.into(),
            });
            Ok(())
        })
    }

    pub fn rename_category(
        &self,
        id: Uuid,
        name: &str,
    ) -> Result<CommonPhrases, CommonPhrasesError> {
        let name = name.trim();
        self.update(|document| {
            check_name(document, Some(id), name)?;
            document
                .categories
                .iter_mut()
                .find(|category| category.id == id)
                .ok_or(CommonPhrasesError::NotFound)?
                .name = name.into();
            Ok(())
        })
    }

    pub fn remove_category(&self, id: Uuid) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            let index = document
                .categories
                .iter()
                .position(|category| category.id == id)
                .ok_or(CommonPhrasesError::NotFound)?;
            document.categories.remove(index);
            for phrase in &mut document.phrases {
                if phrase.category == Some(id) {
                    phrase.category = None;
                }
            }
            if document.expanded_category == id.to_string() {
                document.expanded_category = all();
            }
            Ok(())
        })
    }

    pub fn categorize(
        &self,
        id: Uuid,
        category: Option<Uuid>,
    ) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            if category
                .is_some_and(|id| !document.categories.iter().any(|category| category.id == id))
            {
                return Err(CommonPhrasesError::NotFound);
            }
            let index = position(document, id)?;
            document.phrases[index].category = category;
            Ok(())
        })
    }

    pub fn set_expanded_category(
        &self,
        category: &str,
    ) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            if !expanded_exists(document, category) {
                return Err(CommonPhrasesError::NotFound);
            }
            document.expanded_category = category.into();
            Ok(())
        })
    }

    /// 备份合并只给本次新增的记录分配分类；同名分类复用，已有归属和展开偏好保留。
    pub fn import_categories(
        &self,
        categories: &[CommonPhraseCategoryBackup],
        expanded: &str,
        phrase_ids: &[Uuid],
    ) -> Result<CommonPhrases, CommonPhrasesError> {
        if categories.len() > MAX_CATEGORIES {
            return Err(CommonPhrasesError::CategoryLimit);
        }
        self.update(|document| {
            let adopt_default = document.categories.is_empty();
            let mut mapped = HashMap::new();
            let mut names = HashSet::new();
            let mut texts = HashSet::new();
            for category in categories {
                if category.id.is_nil() || !valid_name(&category.name) {
                    return Err(CommonPhrasesError::CategoryInvalid);
                }
                if mapped.contains_key(&category.id) || !names.insert(&category.name) {
                    return Err(CommonPhrasesError::CategoryDuplicate);
                }
                let id = match document
                    .categories
                    .iter()
                    .find(|item| item.name == category.name)
                {
                    Some(item) => item.id,
                    None => {
                        if document.categories.len() >= MAX_CATEGORIES {
                            return Err(CommonPhrasesError::CategoryLimit);
                        }
                        let id = Uuid::new_v4();
                        document.categories.push(CommonPhraseCategory {
                            id,
                            name: category.name.clone(),
                        });
                        id
                    }
                };
                mapped.insert(category.id, id);
                for text in &category.texts {
                    if !valid_phrase_text(text) || !texts.insert(text) {
                        return Err(CommonPhrasesError::Invalid);
                    }
                    for phrase in &mut document.phrases {
                        if phrase.pack.is_none()
                            && phrase.text == *text
                            && phrase_ids.contains(&phrase.id)
                        {
                            phrase.category = Some(id);
                        }
                    }
                }
            }
            let expanded = if expanded == "all" || expanded.is_empty() {
                expanded.to_owned()
            } else {
                mapped
                    .get(
                        &Uuid::parse_str(expanded)
                            .map_err(|_| CommonPhrasesError::CategoryInvalid)?,
                    )
                    .ok_or(CommonPhrasesError::CategoryInvalid)?
                    .to_string()
            };
            if adopt_default {
                document.expanded_category = expanded;
            }
            Ok(())
        })
    }
}
