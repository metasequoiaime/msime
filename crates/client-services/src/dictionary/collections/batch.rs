use super::*;

impl DictionaryCollectionsStore {
    /// 多选停用或启用：先验证所有目标，再在同一集合锁内提交，避免无效项造成部分修改。
    pub fn set_enabled_many(
        &self,
        ids: &[String],
        enabled: bool,
    ) -> Result<DictionaryCollectionsView> {
        let ids = selection(ids)?;
        self.change(|state| {
            validate_selection(state, &ids)?;
            for id in ids {
                if state.metadata(id)?.enabled == enabled {
                    continue;
                }
                if enabled {
                    state.metadata_mut(id)?.enabled = true;
                    state.enable(id)?;
                } else {
                    state.disable(id)?;
                    state.metadata_mut(id)?.enabled = false;
                }
            }
            Ok(())
        })
    }

    /// 多选删除：沿用停用的词条归属保护，保留未选集合与用户原有的词。
    pub fn delete_many(&self, ids: &[String]) -> Result<DictionaryCollectionsView> {
        let ids = selection(ids)?;
        self.change(|state| {
            validate_selection(state, &ids)?;
            for id in ids {
                let index = state.position(id)?;
                if state.index.collections[index].enabled {
                    state.disable(id)?;
                }
                state.index.collections.remove(index);
                state.entries.remove(&id);
                state.deleted.push(id);
            }
            Ok(())
        })
    }
}

fn selection(ids: &[String]) -> Result<Vec<Uuid>> {
    if ids.is_empty() || ids.len() > MAX_COLLECTIONS {
        return Err(DictionaryCollectionsError::Invalid);
    }
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    for value in ids {
        let id = parse_id(value)?;
        if seen.insert(id) {
            selected.push(id);
        }
    }
    Ok(selected)
}

fn validate_selection(state: &State, ids: &[Uuid]) -> Result<()> {
    for id in ids {
        state.metadata(*id)?;
    }
    Ok(())
}
