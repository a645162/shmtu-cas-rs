use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};

/// 从账单页 HTML 中读取总页数（"当前 X/Y 页"），找不到返回 1。
pub fn get_total_pages(html: &str) -> Result<u32> {
    let document = Html::parse_document(html);
    get_total_pages_from_document(&document)
}

pub fn get_total_pages_from_document(document: &Html) -> Result<u32> {
    let selector = Selector::parse("div > table > tbody > tr > td").unwrap();
    let re = Regex::new(r"当前(\d+)/(\d+)页")?;

    for el in document.select(&selector) {
        let text = el.text().collect::<String>();
        if let Some(caps) = re.captures(&text) {
            let total: u32 = caps[2].parse().unwrap_or(1);
            return Ok(total);
        }
    }

    Ok(1)
}
