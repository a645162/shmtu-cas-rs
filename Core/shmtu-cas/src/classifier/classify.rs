use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 账单分类类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillCategory {
    Deposit,       // 充值
    Electricity,   // 电费
    Bath,          // 洗澡
    HotWater,      // 热水
    Cake,          // 点心
    Canteen,       // 食堂
    Library,       // 图书馆
    Hospital,      // 校医院
    Shop,          // 超市
    Laundry,       // 洗衣
    Network,       // 网络
    Transport,     // 交通
    Other,         // 其他
}

impl BillCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Deposit => "充值",
            Self::Electricity => "电费",
            Self::Bath => "洗澡",
            Self::HotWater => "热水",
            Self::Cake => "点心",
            Self::Canteen => "食堂",
            Self::Library => "图书馆",
            Self::Hospital => "校医院",
            Self::Shop => "超市",
            Self::Laundry => "洗衣",
            Self::Network => "网络",
            Self::Transport => "交通",
            Self::Other => "其他",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Deposit => "💰",
            Self::Electricity => "⚡",
            Self::Bath => "🚿",
            Self::HotWater => "♨️",
            Self::Cake => "🍰",
            Self::Canteen => "🍚",
            Self::Library => "📚",
            Self::Hospital => "🏥",
            Self::Shop => "🛒",
            Self::Laundry => "👕",
            Self::Network => "🌐",
            Self::Transport => "🚌",
            Self::Other => "💳",
        }
    }
}

/// 匹配规则（按 name 或 target 字段匹配关键词）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRule {
    #[serde(default)]
    pub name: Vec<String>,
    #[serde(default)]
    pub target: Vec<String>,
}

/// 账单分类器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillClassifier {
    pub categories: HashMap<String, CategoryRule>,
}

impl Default for BillClassifier {
    fn default() -> Self {
        Self { categories: HashMap::new() }
    }
}

impl BillClassifier {
    /// 从 JSON 字符串加载分类规则
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 从 TOML 字符串加载分类规则
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct TomlRule {
            #[serde(default)]
            match_field: String,
            #[serde(default)]
            match_names: Vec<String>,
            #[serde(default)]
            match_targets: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct TomlWrapper {
            #[serde(rename = "type", default)]
            categories: std::collections::HashMap<String, TomlRule>,
        }
        // type.toml 格式: [type.X] match_field = "..." match_names = [...] match_targets = [...]
        let wrapper: TomlWrapper = toml::from_str(toml_str)?;
        let categories = wrapper.categories.into_iter().map(|(k, v)| {
            let rule = CategoryRule {
                name: v.match_names,
                target: v.match_targets,
            };
            (k, rule)
        }).collect();
        Ok(Self { categories })
    }

    /// 从文件路径加载分类规则（自动识别格式）
    pub fn from_file(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".toml") {
            Ok(Self::from_toml(&content)?)
        } else {
            Ok(serde_json::from_str(&content)?)
        }
    }

    /// 根据 name 和 target 字段分类
    pub fn classify(&self, name: &str, target: &str) -> BillCategory {
        for (cat_name, rule) in &self.categories {
            // 按 name 字段匹配
            for kw in &rule.name {
                if name.contains(kw.as_str()) {
                    return Self::parse_category(cat_name);
                }
            }
            // 按 target 字段匹配
            for kw in &rule.target {
                if target.contains(kw.as_str()) {
                    return Self::parse_category(cat_name);
                }
            }
        }
        BillCategory::Other
    }

    fn parse_category(s: &str) -> BillCategory {
        match s {
            "deposit" => BillCategory::Deposit,
            "electricity" => BillCategory::Electricity,
            "bath" => BillCategory::Bath,
            "hot_water" => BillCategory::HotWater,
            "cake" => BillCategory::Cake,
            "canteen" => BillCategory::Canteen,
            "library" => BillCategory::Library,
            "hospital" => BillCategory::Hospital,
            "shop" => BillCategory::Shop,
            "laundry" => BillCategory::Laundry,
            "network" => BillCategory::Network,
            "transport" => BillCategory::Transport,
            _ => BillCategory::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_classifier() -> BillClassifier {
        let json = r#"{
            "deposit": {"name": ["中行云充值", "微信充值"]},
            "bath": {"target": ["淋浴", "热水"]},
            "canteen": {"target": ["食堂", "餐厅"]}
        }"#;
        BillClassifier::from_json(json).unwrap()
    }

    #[test]
    fn test_classify_deposit_by_name() {
        let c = make_classifier();
        assert_eq!(c.classify("中行云充值", "某商户"), BillCategory::Deposit);
    }

    #[test]
    fn test_classify_bath_by_target() {
        let c = make_classifier();
        assert_eq!(c.classify("消费", "淋浴"), BillCategory::Bath);
    }

    #[test]
    fn test_classify_canteen() {
        let c = make_classifier();
        assert_eq!(c.classify("消费", "食堂"), BillCategory::Canteen);
    }

    #[test]
    fn test_classify_other() {
        let c = make_classifier();
        assert_eq!(c.classify("消费", "未知商户"), BillCategory::Other);
    }
}
