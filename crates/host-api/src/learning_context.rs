//! 学习轮次与本机清除共用当前词库代次的校验，不从目录名推断资源身份。

use crate::{engine_schemes, HostOptions};
use msime_client_services::account::learning_sync::round::{RoundContext, RoundError};
use msime_engine::RuntimePaths;
use serde_json::Value;
use std::path::PathBuf;

pub(crate) struct Context {
    options: crate::EngineOptions,
    paths: RuntimePaths,
    state: PathBuf,
    content_id: String,
    main_dictionary: bool,
}

impl Context {
    pub(crate) fn parse(mut document: Value) -> Result<Self, String> {
        let invalid = || "learning_invalid_options".to_owned();
        // 与词库管理入口相同，Linux 宿主附带的候选皮肤目录不属于 Engine 选项。
        if let Some(object) = document.as_object_mut() {
            object.remove("candidate_skin_catalog");
        }
        let options = HostOptions::from_document(document).ok_or_else(invalid)?;
        if options.api_version != 1 || options.preferences.validate().is_err() {
            return Err(invalid());
        }
        if options.read_only_state == Some(true) {
            return Err("learning_read_only".to_owned());
        }
        let state = options
            .preferences_directory
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(invalid)?;
        // 查询这个会话的资源清单，而非后台最新选择；旧会话可能仍持有上一代。
        let specification = crate::dictionary_updates::specification_for_options(&options)
            .map_err(|_| invalid())?;
        let content_id = specification.generation().map_err(|_| invalid())?;
        let paths = RuntimePaths {
            resources: PathBuf::from(&options.resources),
            user_data: PathBuf::from(&options.user_data),
            cache: PathBuf::from(&options.cache),
            dictionaries: PathBuf::from(&options.dictionaries),
        };
        paths.validate().map_err(|_| invalid())?;
        // 不能让旧运行时选项按新词库标识上传，也不从自由目录名猜 content_id。
        if paths.dictionaries != paths.user_data.join("dictionaries").join(&content_id) {
            return Err(invalid());
        }
        let main_dictionary = engine_schemes(options.edition()).reads_main_dictionary();
        Ok(Self {
            options: options.into_engine_options(),
            paths,
            state,
            content_id,
            main_dictionary,
        })
    }

    pub(crate) fn reset(&self) -> Result<Value, RoundError> {
        crate::dictionary::reset::under_maintenance(&self.options, Some(&self.state))
            .map_err(|_| RoundError::Local)
    }

    pub(crate) fn borrowed(&self) -> RoundContext<'_> {
        RoundContext {
            paths: &self.paths,
            state_directory: &self.state,
            content_id: &self.content_id,
            main_dictionary: self.main_dictionary,
        }
    }
}
