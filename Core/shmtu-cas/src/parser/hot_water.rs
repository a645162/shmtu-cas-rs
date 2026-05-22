use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};

#[derive(Debug, Clone)]
pub struct HotWaterInfo {
    pub building: u32,
    pub temperature: f32,
    pub water_level: f32,
}

/// 解析热水HTML，返回 (温度, 水位百分比, 楼号) 列表
pub fn parse_hot_water_list(html: &str) -> Result<Vec<HotWaterInfo>> {
    let document = Html::parse_document(html);
    let ul_selector = Selector::parse("#tab1 > div > div > ul").unwrap();
    let li_selector = Selector::parse("li").unwrap();
    let div_bagreen_selector = Selector::parse("div.bagreen").unwrap();

    let mut list = Vec::new();

    let ul = match document.select(&ul_selector).next() {
        Some(ul) => ul,
        None => return Ok(list),
    };

    for li in ul.select(&li_selector) {
        let div = match li.select(&div_bagreen_selector).next() {
            Some(d) => d,
            None => continue,
        };

        let children: Vec<_> = div.children().filter_map(scraper::ElementRef::wrap).collect();
        if children.len() != 3 {
            continue;
        }

        let temp_text = children[0].text().collect::<String>();
        let level_text = children[1].text().collect::<String>();
        let building_text = children[2].text().collect::<String>();

        let temperature = match only_float_digits(&temp_text.replace("℃", "")).parse::<f32>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let water_level = match only_float_digits(&level_text.replace("水位", "").replace("%", ""))
            .parse::<f32>()
        {
            Ok(v) => v,
            Err(_) => continue,
        };
        let building = match only_digits(&building_text).parse::<u32>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        list.push(HotWaterInfo {
            building,
            temperature,
            water_level,
        });
    }

    Ok(list)
}

fn only_digits(s: &str) -> String {
    let re = Regex::new(r"\d+").unwrap();
    re.find_iter(s).map(|m| m.as_str()).collect()
}

fn only_float_digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_float_digits() {
        assert_eq!(only_float_digits("36.5℃"), "36.5");
        assert_eq!(only_float_digits("水位75%"), "75");
    }
}
