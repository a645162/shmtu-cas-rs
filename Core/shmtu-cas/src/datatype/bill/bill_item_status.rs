use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BillItemStatus {
    All,
    WaitFor,
    Success,
    Failure,
}

impl BillItemStatus {
    pub fn description(self) -> &'static str {
        match self {
            BillItemStatus::All => "#all",
            BillItemStatus::WaitFor => "#waitfor",
            BillItemStatus::Success => "交易成功",
            BillItemStatus::Failure => "#fail",
        }
    }

    /// 从页面文本（如 "交易成功"）反解成枚举；未匹配返回 None
    pub fn from_text(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        match trimmed {
            "交易成功" => Some(BillItemStatus::Success),
            "#all" => Some(BillItemStatus::All),
            "#waitfor" => Some(BillItemStatus::WaitFor),
            "#fail" => Some(BillItemStatus::Failure),
            _ => None,
        }
    }
}

impl fmt::Display for BillItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}
