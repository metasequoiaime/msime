//! 从实际偏好类型生成键表；只有命令行工具启用，不进入默认宿主依赖图。

use super::{HelpcodeSchema, Preferences, TouchOneHandedSidePanel};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde_json::Value;
use std::borrow::Cow;

impl JsonSchema for HelpcodeSchema {
    fn schema_name() -> Cow<'static, str> {
        "HelpcodeSchema".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = String::json_schema(generator);
        schema.insert(
            "description".into(),
            "辅助码方案名称，亦可为已安装的自定义表名称".into(),
        );
        schema
    }
}

impl JsonSchema for TouchOneHandedSidePanel {
    fn schema_name() -> Cow<'static, str> {
        "TouchOneHandedSidePanel".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = String::json_schema(generator);
        schema.insert("examples".into(), serde_json::json!(["off", "number_pad"]));
        schema
    }
}

/// 完整偏好对象的 JSON Schema。描述字段形状与出厂默认值；跨字段业务规则仍以共享校验为准。
pub fn json_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Preferences))
        .expect("preference schema is JSON");
    let defaults = serde_json::to_value(Preferences::default()).expect("defaults are JSON");
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for (key, value) in defaults.as_object().expect("preferences are an object") {
            if let Some(field) = properties.get_mut(key) {
                field["default"] = value.clone();
            }
        }
    }
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_omitted_optional_fields_and_empty_array_element_types() {
        let schema = json_schema();
        assert!(schema["properties"]["usage_reporting_consent"].is_object());
        assert_eq!(
            schema["properties"]["next_word_prediction"]["default"],
            false
        );
        assert_eq!(
            schema["properties"]["vim_mode_apps"]["default"],
            serde_json::json!([])
        );
        assert!(schema["properties"]["vim_mode_apps"]["items"].is_object());
        assert!(schema["properties"]["app_input_mode_rules"]["additionalProperties"].is_object());
        assert_eq!(
            schema["properties"]["candidate_page_size"]["default"],
            Preferences::default().candidate_page_size
        );
        assert!(
            schema["$defs"]["SentenceAssociationPreferences"]["properties"]
                ["sentence_collocation_weight"]
                .is_object()
        );
    }
}
