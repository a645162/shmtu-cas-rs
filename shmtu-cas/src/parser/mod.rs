pub mod export;
pub mod hot_water;

use anyhow::Result;
use chrono::NaiveDateTime;
use regex::Regex;
use scraper::{Html, Selector};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BillItem {
    pub date_str: String,
    pub time_str: String,
    pub time_str_formatted: String,
    pub date_time_formatted: String,
    pub timestamp: i64,
    pub item_type: String,
    pub number: String,
    pub target_user: String,
    pub money_str: String,
    pub money: f32,
    pub method: String,
    pub status: String,
}

impl BillItem {
    pub fn get_field(&self, field: &str) -> String {
        match field {
            "date_str" => self.date_str.clone(),
            "time_str" => self.time_str.clone(),
            "time_str_formatted" => self.time_str_formatted.clone(),
            "date_time_formatted" => self.date_time_formatted.clone(),
            "timestamp" => self.timestamp.to_string(),
            "item_type" => self.item_type.clone(),
            "number" => self.number.clone(),
            "target_user" => self.target_user.clone(),
            "money_str" => self.money_str.clone(),
            "money" => format!("{:.2}", self.money),
            "method" => self.method.clone(),
            "status" => self.status.clone(),
            _ => String::new(),
        }
    }
}

pub fn parse_bill_list(html: &str) -> Result<Vec<BillItem>> {
    let document = Html::parse_document(html);
    let tr_selector = Selector::parse("span > table > tbody > tr").unwrap();
    let td_selector = Selector::parse("td").unwrap();
    let div_selector = Selector::parse("div").unwrap();

    let mut bills = Vec::new();

    for row in document.select(&tr_selector) {
        let tds: Vec<_> = row.select(&td_selector).collect();
        if tds.len() < 6 {
            continue;
        }

        let divs: Vec<_> = tds[0].select(&div_selector).collect();
        let (date_str, time_str) = if divs.len() >= 2 {
            (extract_text(&divs[0]), extract_text(&divs[1]))
        } else {
            let text = extract_text(&tds[0]);
            let parts: Vec<&str> = text.split_whitespace().collect();
            (
                parts.first().unwrap_or(&"").to_string(),
                parts.get(1).unwrap_or(&"").to_string(),
            )
        };

        let time_str_formatted = format_time(&time_str);
        let date_time_formatted = format!("{} {}", date_str, time_str_formatted);
        let timestamp = NaiveDateTime::parse_from_str(&date_time_formatted, "%Y.%m.%d %H:%M:%S")
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(0);

        let deal_divs: Vec<_> = tds[1].select(&div_selector).collect();
        let (item_type, number) = if deal_divs.len() >= 2 {
            (
                extract_text(&deal_divs[0]),
                only_digits(&extract_text(&deal_divs[1])),
            )
        } else {
            let text = extract_text(&tds[1]);
            (text, String::new())
        };

        let target_user = extract_text(&tds[2]);
        let money_str = extract_text(&tds[3]);
        let money: f32 = money_str.trim().parse().unwrap_or(0.0);
        let method = extract_text(&tds[4]);
        let status = extract_text(&tds[5]);

        bills.push(BillItem {
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
            status,
        });
    }

    Ok(bills)
}

pub fn get_total_pages(html: &str) -> Result<u32> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("div > table > tbody > tr > td").unwrap();

    for el in document.select(&selector) {
        let text = el.text().collect::<String>();
        let re = Regex::new(r"当前(\d+)/(\d+)页")?;
        if let Some(caps) = re.captures(&text) {
            let total: u32 = caps[2].parse().unwrap_or(1);
            return Ok(total);
        }
    }

    Ok(1)
}

fn extract_text(element: &scraper::ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

fn format_time(time_str: &str) -> String {
    let digits: String = time_str.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 6 {
        format!("{}:{}:{}", &digits[0..2], &digits[2..4], &digits[4..6])
    } else {
        time_str.to_string()
    }
}

fn only_digits(s: &str) -> String {
    let re = Regex::new(r"\d+").unwrap();
    re.find_iter(s).map(|m| m.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        assert_eq!(format_time("123045"), "12:30:45");
        assert_eq!(format_time("080000"), "08:00:00");
        assert_eq!(format_time("12:30:45"), "12:30:45");
    }

    #[test]
    fn test_only_digits() {
        assert_eq!(only_digits("交易号: 202401010001"), "202401010001");
        assert_eq!(only_digits("abc123def456"), "123456");
    }
}
