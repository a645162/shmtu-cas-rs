use super::bill_item_status::BillItemStatus;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillItem {
    // === 时间 ===
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: String,
    /// 开始时间（单条 = 唯一时间；合并 = 最早时间）
    pub date_time_formatted: String,
    /// 结束时间（单条 = 开始时间；合并 = 最晚时间）
    pub end_date_time_formatted: String,
    /// 开始时间戳
    pub timestamp: i64,
    /// 结束时间戳
    pub end_timestamp: i64,

    // === 交易信息 ===
    pub item_type: String,
    /// 单条有值，合并为空串
    pub number: String,
    /// 所有交易号（单条 = [number]，合并 = 多个，按时间升序）
    pub number_list: Vec<String>,
    pub target_user: String,

    // === 金额 ===
    pub money_str: String,
    pub money: f32,

    // === 其他 ===
    pub method: String,
    pub status_str: String,
    /// 是否为合并条目（只读语义：只由 merge 生成）
    pub is_combined: bool,
}

impl BillItem {
    /// 创建单条账单。
    pub fn new_single(
        date_str: String,
        time_str: String,
        time_str_formatted: String,
        date_time_formatted: String,
        timestamp: i64,
        item_type: String,
        number: String,
        target_user: String,
        money_str: String,
        money: f32,
        method: String,
        status_str: String,
    ) -> Self {
        Self {
            end_date_time_formatted: date_time_formatted.clone(),
            end_timestamp: timestamp,
            number_list: vec![number.clone()],
            is_combined: false,
            date_str,
            time_str,
            time_str_formatted,
            date_time_formatted,
            timestamp,
            item_type,
            number,
            target_user,
            money_str,
            money,
            method,
            status_str,
        }
    }

    /// 合并多条账单（单条/混合/合并条目均可）。按时间升序排列，金额求和。
    /// 1 条直接返回原样；0 条 panic。
    pub fn merge(items: Vec<BillItem>) -> BillItem {
        assert!(!items.is_empty(), "不能合并空的账单列表");
        if items.len() == 1 {
            return items.into_iter().next().unwrap();
        }

        let mut sorted = items;
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let first = &sorted[0];
        let last = sorted.last().unwrap();

        let total_money: f32 = sorted.iter().map(|b| b.money).sum();

        let mut all_numbers: Vec<String> = Vec::new();
        for item in &sorted {
            all_numbers.extend(item.number_list.iter().cloned());
        }

        BillItem {
            date_str: first.date_str.clone(),
            time_str: first.time_str.clone(),
            time_str_formatted: first.time_str_formatted.clone(),
            date_time_formatted: first.date_time_formatted.clone(),
            end_date_time_formatted: last.end_date_time_formatted.clone(),
            timestamp: first.timestamp,
            end_timestamp: last.end_timestamp,
            item_type: first.item_type.clone(),
            number: String::new(),
            number_list: all_numbers,
            target_user: first.target_user.clone(),
            money_str: format!("{:.2}", total_money),
            money: total_money,
            method: first.method.clone(),
            status_str: first.status_str.clone(),
            is_combined: true,
        }
    }

    /// 与另一条合并的快捷方法。
    pub fn merge_with(&self, other: &BillItem) -> BillItem {
        BillItem::merge(vec![self.clone(), other.clone()])
    }

    /// 解析后的状态枚举。未识别返回 BillItemStatus::All。
    pub fn status(&self) -> BillItemStatus {
        BillItemStatus::from_text(&self.status_str).unwrap_or(BillItemStatus::All)
    }

    /// CSV 导出按字段名取值。
    pub fn get_field(&self, field: &str) -> String {
        match field {
            "date_str" => self.date_str.clone(),
            "time_str" => self.time_str.clone(),
            "time_str_formatted" => self.time_str_formatted.clone(),
            "date_time_formatted" => self.date_time_formatted.clone(),
            "end_date_time_formatted" => self.end_date_time_formatted.clone(),
            "timestamp" => self.timestamp.to_string(),
            "end_timestamp" => self.end_timestamp.to_string(),
            "item_type" => self.item_type.clone(),
            "number" => self.number.clone(),
            "number_list" => self.number_list.join(","),
            "target_user" => self.target_user.clone(),
            "money_str" => self.money_str.clone(),
            "money" => format!("{:.2}", self.money),
            "method" => self.method.clone(),
            "status" | "status_str" => self.status_str.clone(),
            "is_combined" => self.is_combined.to_string(),
            _ => String::new(),
        }
    }

    // === 比较 ===

    /// 按时间排序比较。
    pub fn cmp_by_time(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }

    /// 按金额排序比较。
    pub fn cmp_by_money(&self, other: &Self) -> Ordering {
        self.money
            .partial_cmp(&other.money)
            .unwrap_or(Ordering::Equal)
    }
}

// === 相等比较：基于交易号列表 ===

impl PartialEq for BillItem {
    fn eq(&self, other: &Self) -> bool {
        self.number_list == other.number_list
    }
}

impl Eq for BillItem {}

// === Display ===

impl fmt::Display for BillItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_combined {
            write!(
                f,
                "{} - {} | {} | {:.2} | [{}条合并]",
                self.date_time_formatted,
                self.end_date_time_formatted,
                self.item_type,
                self.money,
                self.number_list.len()
            )
        } else {
            write!(
                f,
                "{} | {} | {} | {} | {}",
                self.date_time_formatted,
                self.item_type,
                self.target_user,
                self.money_str,
                self.status_str
            )
        }
    }
}

// === 工具函数 ===

/// 对一组账单求金额总和。
pub fn sum_money(items: &[BillItem]) -> f32 {
    items.iter().map(|b| b.money).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single(timestamp: i64, number: &str, money: f32) -> BillItem {
        BillItem::new_single(
            "2026.05.21".into(),
            "123045".into(),
            "12:30:45".into(),
            "2026.05.21 12:30:45".into(),
            timestamp,
            "消费".into(),
            number.into(),
            "食堂".into(),
            format!("{:.2}", money),
            money,
            "刷卡".into(),
            "交易成功".into(),
        )
    }

    #[test]
    fn test_equality_same_number() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N001", 20.0);
        assert_eq!(a, b); // same number_list → equal
    }

    #[test]
    fn test_equality_different_number() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(100, "N002", 10.0);
        assert_ne!(a, b);
    }

    #[test]
    fn test_merge_basic() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        let merged = BillItem::merge(vec![a, b]);

        assert!(merged.is_combined);
        assert_eq!(merged.number, "");
        assert_eq!(merged.number_list, vec!["N001", "N002"]);
        assert!((merged.money - 30.0).abs() < 0.01);
        assert_eq!(merged.timestamp, 100);
        assert_eq!(merged.end_timestamp, 200);
        assert_eq!(merged.date_time_formatted, "2026.05.21 12:30:45");
        assert_eq!(merged.end_date_time_formatted, "2026.05.21 12:30:45");
    }

    #[test]
    fn test_merge_combined_with_single() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        let combined = BillItem::merge(vec![a, b]);

        let c = make_single(300, "N003", 5.0);
        let re_merged = BillItem::merge(vec![combined, c]);

        assert!(re_merged.is_combined);
        assert_eq!(re_merged.number_list, vec!["N001", "N002", "N003"]);
        assert!((re_merged.money - 35.0).abs() < 0.01);
        assert_eq!(re_merged.end_timestamp, 300);
    }

    #[test]
    fn test_merge_with() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        let merged = a.merge_with(&b);

        assert!(merged.is_combined);
        assert_eq!(merged.number_list.len(), 2);
    }

    #[test]
    fn test_cmp_by_time() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        assert_eq!(a.cmp_by_time(&b), Ordering::Less);
    }

    #[test]
    fn test_cmp_by_money() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        assert_eq!(a.cmp_by_money(&b), Ordering::Less);
    }

    #[test]
    fn test_sum_money() {
        let items = vec![
            make_single(100, "N001", 10.0),
            make_single(200, "N002", 20.5),
        ];
        assert!((sum_money(&items) - 30.5).abs() < 0.01);
    }

    #[test]
    fn test_display_single() {
        let item = make_single(100, "N001", 10.0);
        let s = format!("{}", item);
        assert!(s.contains("消费"));
        assert!(!s.contains("合并"));
    }

    #[test]
    fn test_display_combined() {
        let a = make_single(100, "N001", 10.0);
        let b = make_single(200, "N002", 20.0);
        let merged = BillItem::merge(vec![a, b]);
        let s = format!("{}", merged);
        assert!(s.contains("2条合并"));
        assert!(s.contains(" - "));
    }
}
