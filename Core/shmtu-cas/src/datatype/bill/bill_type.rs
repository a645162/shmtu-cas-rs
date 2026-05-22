use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BillType {
    All,
    NotPaid,
    Success,
    Failure,
}

impl BillType {
    pub fn description(self) -> &'static str {
        match self {
            BillType::All => "全部",
            BillType::NotPaid => "未付款",
            BillType::Success => "成功",
            BillType::Failure => "失败",
        }
    }

    pub fn tab_no(self) -> &'static str {
        match self {
            BillType::All => "1",
            BillType::Success => "2",
            BillType::NotPaid => "3",
            BillType::Failure => "4",
        }
    }
}

impl FromStr for BillType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "all" => Ok(BillType::All),
            "notpaid" | "waitfor" | "not_paid" => Ok(BillType::NotPaid),
            "success" => Ok(BillType::Success),
            "failure" | "fail" => Ok(BillType::Failure),
            other => Err(format!("未知的 BillType: {}", other)),
        }
    }
}

impl fmt::Display for BillType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}
