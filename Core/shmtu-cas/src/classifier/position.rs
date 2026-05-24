use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 位置翻译条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionEntry {
    pub position: String,
    pub room: String,
}

/// 位置翻译器 — 将"对方账户"（target_user）翻译为目标名称和房间
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionTranslator {
    pub field: String,
    pub keywords: HashMap<String, PositionEntry>,
}

impl Default for PositionTranslator {
    fn default() -> Self {
        Self {
            field: "target".to_string(),
            keywords: HashMap::new(),
        }
    }
}

impl PositionTranslator {
    /// 从 JSON 字符串加载翻译规则
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 从 TOML 字符串加载翻译规则
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        #[derive(serde::Deserialize)]
        struct TomlPosition {
            field: String,
            #[serde(default)]
            keywords: std::collections::HashMap<String, TomlEntry>,
        }
        #[derive(serde::Deserialize)]
        struct TomlEntry {
            building: String,
            room: String,
        }
        #[derive(serde::Deserialize)]
        struct TomlWrapper {
            position: Option<TomlPosition>,
        }
        // position.toml 格式: [position] field = "..." / [position.keywords."X"] building = "..." room = "..."
        let wrapper: TomlWrapper = toml::from_str(toml_str)?;
        if let Some(pos) = wrapper.position {
            Ok(Self {
                field: pos.field,
                keywords: pos.keywords.into_iter().map(|(k, v)| {
                    (k, PositionEntry { position: v.building, room: v.room })
                }).collect(),
            })
        } else {
            // 尝试直接解析（无外层 position 表）
            toml::from_str(toml_str)
        }
    }

    /// 从文件路径加载翻译规则（自动识别格式）
    pub fn from_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".toml") {
            Ok(Self::from_toml(&content)?)
        } else {
            Ok(serde_json::from_str(&content)?)
        }
    }

    /// 翻译 target_user 字段，返回 (position, room)
    pub fn translate(&self, target_user: &str) -> Option<(String, String)> {
        let trimmed = target_user.trim();
        // 精确匹配
        if let Some(entry) = self.keywords.get(trimmed) {
            return Some((entry.position.clone(), entry.room.clone()));
        }
        // 模糊匹配：检查 target_user 是否包含关键词
        for (keyword, entry) in &self.keywords {
            if trimmed.contains(keyword.as_str()) {
                return Some((entry.position.clone(), entry.room.clone()));
            }
        }
        None
    }

    /// 翻译，找不到则返回 raw
    pub fn translate_or_raw(&self, target_user: &str) -> (String, String) {
        self.translate(target_user)
            .unwrap_or_else(|| (target_user.to_string(), target_user.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_translator() -> PositionTranslator {
        let json = r#"{
            "field": "target",
            "keywords": {
                "A食堂1楼大餐厅": {"position": "海馨楼", "room": "海馨第1食堂"},
                "淋浴": {"position": "公共浴室", "room": "浴室"},
                "教育超市": {"position": "校园商业", "room": "教育超市"}
            }
        }"#;
        PositionTranslator::from_json(json).unwrap()
    }

    #[test]
    fn test_exact_match() {
        let t = make_translator();
        let (pos, room) = t.translate("A食堂1楼大餐厅").unwrap();
        assert_eq!(pos, "海馨楼");
        assert_eq!(room, "海馨第1食堂");
    }

    #[test]
    fn test_fuzzy_match() {
        let t = make_translator();
        let (pos, room) = t.translate("淋浴-北区浴室").unwrap();
        assert_eq!(pos, "公共浴室");
        assert_eq!(room, "浴室");
    }

    #[test]
    fn test_no_match() {
        let t = make_translator();
        assert!(t.translate("未知地点").is_none());
    }
}
