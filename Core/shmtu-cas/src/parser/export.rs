use anyhow::{Context, Result};
use csv::Writer;
use std::fs::File;

use crate::datatype::bill::BillItem;

const DEFAULT_HEADERS: &[&str] = &[
    "日期",
    "时间",
    "时间(格式化)",
    "日期时间",
    "时间戳",
    "交易名称",
    "交易号",
    "对方",
    "金额",
    "付款方式",
    "状态",
];

const DEFAULT_FIELDS: &[&str] = &[
    "date_str",
    "time_str",
    "time_str_formatted",
    "date_time_formatted",
    "timestamp",
    "item_type",
    "number",
    "target_user",
    "money_str",
    "method",
    "status",
];

pub struct CsvExporter {
    headers: Vec<String>,
    fields: Vec<String>,
}

impl CsvExporter {
    pub fn new() -> Self {
        Self {
            headers: DEFAULT_HEADERS.iter().map(|s| s.to_string()).collect(),
            fields: DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn headers(mut self, headers: Vec<String>) -> Self {
        self.headers = headers;
        self
    }

    pub fn fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    pub fn export(&self, path: &str, bills: &[BillItem]) -> Result<()> {
        let file = File::create(path).context("创建CSV文件失败")?;
        let mut wtr = Writer::from_writer(file);

        wtr.write_record(&self.headers)?;

        for bill in bills {
            let row: Vec<String> = self
                .fields
                .iter()
                .map(|f| bill.get_field(f))
                .collect();
            wtr.write_record(&row)?;
        }

        wtr.flush()?;
        Ok(())
    }
}
