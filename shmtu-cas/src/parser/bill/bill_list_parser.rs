use anyhow::Result;
use scraper::Html;

use crate::datatype::bill::BillItem;

use super::bill_item_parser::parse_bill_item_list;
use super::page_count_parser::get_total_pages_from_document;

pub struct BillParseResult {
    pub bills: Vec<BillItem>,
    pub total_pages: u32,
}

/// 一次性解析整页：账单条目 + 总页数。对齐 C# 的 `BillHtmlParser.Parse()`。
pub fn parse_bill_page(html: &str) -> Result<BillParseResult> {
    let document = Html::parse_document(html);
    let bills = parse_bill_item_list(&document)?;
    let total_pages = get_total_pages_from_document(&document)?;
    Ok(BillParseResult { bills, total_pages })
}

/// 只取账单条目列表。
pub fn parse_bill_list(html: &str) -> Result<Vec<BillItem>> {
    let document = Html::parse_document(html);
    parse_bill_item_list(&document)
}
