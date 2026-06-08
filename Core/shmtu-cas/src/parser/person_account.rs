use anyhow::Result;
use scraper::{Html, Selector};

/// 一卡通个人账户页解析结果
///
/// 对应 `/epay/personaccount/index` 接口的 HTML。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PersonAccountInfo {
    // 头部
    pub real_name: String,
    pub real_name_auth_status: String,

    // 资金&安全信息
    pub cash_balance: f64,
    pub cash_balance_raw: String,
    pub security_question_status: String,
    pub register_date: String,

    // 基本信息
    pub student_id: String,
    pub email: String,
    pub nickname: String,
    pub gender: String,
    pub class_name: String,
    pub mobile: String,
    pub fixed_line: String,
    pub id_type: String,
    pub id_number: String,
    pub remark: String,
    pub user_type: String,

    // CSRF
    pub csrf_token: String,
    pub csrf_header: String,
}

fn strip_trailing_colon(text: &str) -> String {
    text.trim_end_matches(|c| c == ':' || c == '：')
        .trim()
        .to_string()
}

fn parse_kv_tbody(
    document: &Html,
    tbody_sel: &Selector,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(tbody) = document.select(tbody_sel).next() {
        for tr in tbody.select(&Selector::parse("tr").unwrap()) {
            let tds: Vec<_> = tr.select(&Selector::parse("td").unwrap()).collect();
            if tds.len() >= 2 {
                let key = strip_trailing_colon(&tds[0].text().collect::<String>());
                let value = tds[1].text().collect::<String>().trim().to_string();
                if !key.is_empty() {
                    map.insert(key, value);
                }
            }
        }
    }
    map
}

fn parse_otherinfo_tables(
    document: &Html,
    panel_sel: &Selector,
) -> std::collections::HashMap<String, String> {
    let mut merged = std::collections::HashMap::new();
    if let Some(panel) = document.select(panel_sel).next() {
        let tbody_sel = Selector::parse("tbody").unwrap();
        for tbody in panel.select(&tbody_sel) {
            for tr in tbody.select(&Selector::parse("tr").unwrap()) {
                let tds: Vec<_> = tr.select(&Selector::parse("td").unwrap()).collect();
                if tds.len() >= 2 {
                    let key = strip_trailing_colon(&tds[0].text().collect::<String>());
                    let value = tds[1].text().collect::<String>().trim().to_string();
                    if !key.is_empty() {
                        merged.insert(key, value);
                    }
                }
            }
        }
    }
    merged
}

fn extract_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let idx = text.find(marker)?;
    let start = idx + marker.len();
    if start >= text.len() {
        return Some("");
    }
    let tail = text[start..].trim();
    // 截断到下一个分隔符(包括 nbsp 字符, 因为 scraper 不会把   转成普通空格)
    let end = tail
        .find(|c: char| c.is_whitespace() || c == '\u{00a0}')
        .unwrap_or(tail.len());
    Some(tail[..end].trim())
}

/// 一次性解析整个 HTML 页面
pub fn parse_person_account(html: &str) -> Result<PersonAccountInfo> {
    let document = Html::parse_document(html);

    // 1) panel-title 标题: "姓名：xxx  实名认证:已认证"
    //    使用正则精确提取, 避免 &nbsp; / 多空格等边界问题
    let title_sel = Selector::parse(".panel-title").unwrap();
    let title_text = document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();

    // 把   (nbsp) 替换成普通空格, 再做正则匹配
    let normalized_title: String = title_text
        .chars()
        .map(|c| if c == '\u{00a0}' { ' ' } else { c })
        .collect();

    let real_name = extract_after(&normalized_title, "姓名：")
        .or_else(|| extract_after(&normalized_title, "姓名:"))
        .unwrap_or("")
        .to_string();
    let real_name_auth_status = extract_after(&normalized_title, "实名认证:")
        .or_else(|| extract_after(&normalized_title, "实名认证："))
        .unwrap_or("")
        .to_string();

    // 2) CSRF token
    let csrf_token = document
        .select(&Selector::parse("meta[name=_csrf]").unwrap())
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("")
        .to_string();
    let csrf_header = document
        .select(&Selector::parse("meta[name=_csrf_header]").unwrap())
        .next()
        .and_then(|el| el.value().attr("content"))
        .unwrap_or("X-CSRF-TOKEN")
        .to_string();

    // 3) 基本信息表
    let base_info_map = parse_kv_tbody(&document, &Selector::parse("#baseinfo tbody").unwrap());

    // 4) 资金&安全信息表
    let other_info_map = parse_otherinfo_tables(&document, &Selector::parse("#otherinfo").unwrap());

    // 资金
    let cash_balance_raw = other_info_map
        .get("现金资金")
        .map(|s| s.replace("元", "").trim().to_string())
        .unwrap_or_default();
    let cash_balance: f64 = cash_balance_raw.parse().unwrap_or(0.0);

    let get = |map: &std::collections::HashMap<String, String>, key: &str| -> String {
        map.get(key).cloned().unwrap_or_default()
    };

    Ok(PersonAccountInfo {
        real_name,
        real_name_auth_status,
        cash_balance,
        cash_balance_raw,
        security_question_status: get(&other_info_map, "安全保护问题"),
        register_date: get(&other_info_map, "注册时间"),
        student_id: get(&base_info_map, "学工号"),
        email: get(&base_info_map, "电子邮箱"),
        nickname: get(&base_info_map, "昵称"),
        gender: get(&base_info_map, "性别"),
        class_name: get(&base_info_map, "班级"),
        mobile: get(&base_info_map, "手机"),
        fixed_line: get(&base_info_map, "固话"),
        id_type: get(&base_info_map, "证件类型"),
        id_number: get(&base_info_map, "证件号码"),
        remark: get(&base_info_map, "备注"),
        user_type: get(&base_info_map, "用户类型"),
        csrf_token,
        csrf_header,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_HTML: &str = r#"
    <!DOCTYPE html>
    <html lang="zh">
    <head>
        <meta charset="utf-8">
        <meta name="_csrf" content="test-csrf-token-xyz"/>
        <meta name="_csrf_header" content="X-CSRF-TOKEN"/>
        <title>账户管理</title>
    </head>
    <body>
        <h4 class="panel-title">姓名：张三 实名认证:已认证</h4>
        <div id="baseinfo"><table><tbody>
            <tr><td>学工号：</td><td>202430500099</td></tr>
            <tr><td>电子邮箱：</td><td>zs@example.com</td></tr>
            <tr><td>真实姓名：</td><td>张三</td></tr>
            <tr><td>昵称：</td><td></td></tr>
            <tr><td>性别：</td><td>女</td></tr>
            <tr><td>班级：</td><td>航运2024-1</td></tr>
            <tr><td>手机：</td><td>13800138000</td></tr>
            <tr><td>固话：</td><td>021-12345678</td></tr>
            <tr><td>证件类型：</td><td>身份证</td></tr>
            <tr><td>证件号码：</td><td>310101199901011234</td></tr>
            <tr><td>备注：</td><td></td></tr>
            <tr><td>用户类型：</td><td>本科生</td></tr>
        </tbody></table></div>
        <div id="otherinfo">
            <table><tbody>
                <tr><td>现金资金：</td><td>123.45 元</td></tr>
            </tbody></table>
            <table><tbody>
                <tr><td>安全保护问题：</td><td>您已经设置安全保护问题</td></tr>
                <tr><td>注册时间：</td><td>2024年09月01日</td></tr>
            </tbody></table>
        </div>
    </body>
    </html>
    "#;

    #[test]
    fn test_parse_person_account_full() {
        let info = parse_person_account(FIXTURE_HTML).unwrap();
        assert_eq!(info.real_name, "张三");
        assert_eq!(info.real_name_auth_status, "已认证");
        assert_eq!(info.cash_balance_raw, "123.45");
        assert!((info.cash_balance - 123.45).abs() < 1e-6);
        assert_eq!(info.security_question_status, "您已经设置安全保护问题");
        assert_eq!(info.register_date, "2024年09月01日");
        assert_eq!(info.student_id, "202430500099");
        assert_eq!(info.email, "zs@example.com");
        assert_eq!(info.gender, "女");
        assert_eq!(info.class_name, "航运2024-1");
        assert_eq!(info.mobile, "13800138000");
        assert_eq!(info.fixed_line, "021-12345678");
        assert_eq!(info.id_type, "身份证");
        assert_eq!(info.id_number, "310101199901011234");
        assert_eq!(info.user_type, "本科生");
        assert_eq!(info.csrf_token, "test-csrf-token-xyz");
        assert_eq!(info.csrf_header, "X-CSRF-TOKEN");
    }

    #[test]
    fn test_parse_person_account_merges_otherinfo_tbodies() {
        // 关键回归: #otherinfo 下两张表必须合并
        let info = parse_person_account(FIXTURE_HTML).unwrap();
        assert!(!info.security_question_status.is_empty());
        assert!(!info.register_date.is_empty());
    }

    #[test]
    fn test_parse_person_account_empty_html() {
        let info = parse_person_account("<html><body></body></html>").unwrap();
        assert_eq!(info.real_name, "");
        assert_eq!(info.cash_balance, 0.0);
        assert_eq!(info.cash_balance_raw, "");
        assert_eq!(info.student_id, "");
    }

    #[test]
    fn test_parse_person_account_invalid_cash_balance() {
        let html = r#"
        <html><body>
            <div id="otherinfo"><table><tbody>
                <tr><td>现金资金：</td><td>非数字 元</td></tr>
            </tbody></table></div>
        </body></html>
        "#;
        let info = parse_person_account(html).unwrap();
        assert_eq!(info.cash_balance_raw, "非数字");
        assert_eq!(info.cash_balance, 0.0);
    }

    /// 关键回归: panel-title 中的 &nbsp; 不能让姓名/实名认证字段被吞掉整段
    #[test]
    fn test_parse_person_account_with_nbsp_in_title() {
        let html = r#"
        <html><body>
            <h4 class="panel-title">姓名：张三&nbsp;&nbsp;实名认证:已认证</h4>
            <div id="baseinfo"><table><tbody>
                <tr><td>学工号：</td><td>202430500099</td></tr>
            </tbody></table></div>
            <div id="otherinfo"><table><tbody>
                <tr><td>现金资金：</td><td>100 元</td></tr>
            </tbody></table></div>
        </body></html>
        "#;
        let info = parse_person_account(html).unwrap();
        assert_eq!(info.real_name, "张三");
        assert_eq!(info.real_name_auth_status, "已认证");
        assert_eq!(info.student_id, "202430500099");
        assert_eq!(info.cash_balance_raw, "100");
    }
}
