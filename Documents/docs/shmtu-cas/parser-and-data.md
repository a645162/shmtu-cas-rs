# 解析器与数据模型

## `datatype::bill`

该模块导出：

- `BillItem` — 核心数据载体
- `BillItemStatus` — 交易状态枚举
- `BillType` — 账单类型枚举
- `sum_money` — 金额求和工具函数

## `BillItem`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillItem {
    // === 时间 ===
    pub date_str: String,               // "2026.05.21"
    pub time_str: String,               // "123045"
    pub time_str_formatted: String,     // "12:30:45"
    pub date_time_formatted: String,    // "2026.05.21 12:30:45"
    pub end_date_time_formatted: String,// 合并条目的最晚时间
    pub timestamp: i64,                 // Unix 时间戳（秒）
    pub end_timestamp: i64,             // 合并条目的最晚时间戳

    // === 交易信息 ===
    pub item_type: String,              // "消费"、"充值" 等
    pub number: String,                 // 单条有值，合并为空串
    pub number_list: Vec<String>,       // 所有交易号（合并条目包含多个）
    pub target_user: String,            // 对方账户

    // === 金额 ===
    pub money_str: String,              // "12.50"
    pub money: f32,                     // 数值形式

    // === 其他 ===
    pub method: String,                 // "刷卡"、"在线充值" 等
    pub status_str: String,             // "交易成功" 等
    pub is_combined: bool,              // 是否为合并条目
}
```

### 字段详解

#### 时间字段

| 字段 | 类型 | 单条 | 合并 |
|------|------|------|------|
| `date_str` | `String` | 原始日期 | 最早条目的日期 |
| `time_str` | `String` | 原始时间 | 最早条目的时间 |
| `time_str_formatted` | `String` | 格式化时间 | 最早条目的格式化时间 |
| `date_time_formatted` | `String` | 完整日期时间 | 最早条目 |
| `end_date_time_formatted` | `String` | = date_time_formatted | 最晚条目 |
| `timestamp` | `i64` | Unix 时间戳 | 最早时间戳 |
| `end_timestamp` | `i64` | = timestamp | 最晚时间戳 |

#### 交易号字段

| 字段 | 类型 | 单条 | 合并 |
|------|------|------|------|
| `number` | `String` | 唯一交易号 | 空串 `""` |
| `number_list` | `Vec<String>` | `[number]` | 多个交易号，按时间升序 |

**JSON 序列化示例（单条）：**

```json
{
    "date_str": "2026.05.21",
    "time_str": "123045",
    "time_str_formatted": "12:30:45",
    "date_time_formatted": "2026.05.21 12:30:45",
    "end_date_time_formatted": "2026.05.21 12:30:45",
    "timestamp": 1747804245,
    "end_timestamp": 1747804245,
    "item_type": "水控消费",
    "number": "20260521135336290726",
    "number_list": ["20260521135336290726"],
    "target_user": "A食堂1楼大餐厅",
    "money_str": "12.50",
    "money": 12.5,
    "method": "刷卡",
    "status_str": "交易成功",
    "is_combined": false
}
```

**JSON 序列化示例（合并条目）：**

```json
{
    "number": "",
    "number_list": ["20260521135336290726", "20260521135336290727"],
    "money_str": "25.00",
    "money": 25.0,
    "is_combined": true
}
```

### 方法详解

#### `BillItem::new_single`

```rust
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
) -> Self
```

创建单条账单。`end_date_time_formatted` 和 `end_timestamp` 自动设为与开始时间相同，`number_list` 自动设为 `[number]`，`is_combined` 设为 `false`。

**示例：**

```rust
let item = BillItem::new_single(
    "2026.05.21".into(),
    "123045".into(),
    "12:30:45".into(),
    "2026.05.21 12:30:45".into(),
    1747804245,
    "水控消费".into(),
    "20260521135336290726".into(),
    "A食堂1楼大餐厅".into(),
    "12.50".into(),
    12.5,
    "刷卡".into(),
    "交易成功".into(),
);
```

#### `BillItem::merge`

```rust
pub fn merge(items: Vec<BillItem>) -> BillItem
```

合并多条账单。按时间升序排列，金额求和，`number` 置空，`number_list` 合并。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `items` | `Vec<BillItem>` | 至少 1 条账单 |

**返回值：** `BillItem` -- 合并后的条目（`is_combined = true`）

**规则：**
- 0 条 -> panic
- 1 条 -> 返回原样
- 多条 -> 按时间排序，取首尾时间，金额求和

**示例：**

```rust
let merged = BillItem::merge(vec![item_a, item_b]);
assert!(merged.is_combined);
assert_eq!(merged.number, "");
assert_eq!(merged.number_list, vec!["N001", "N002"]);
assert!((merged.money - 30.0).abs() < 0.01);
```

#### `BillItem::merge_with`

```rust
pub fn merge_with(&self, other: &BillItem) -> BillItem
```

与另一条合并的快捷方法，等价于 `BillItem::merge(vec![self.clone(), other.clone()])`。

#### `BillItem::status`

```rust
pub fn status(&self) -> BillItemStatus
```

解析 `status_str` 为枚举，未识别返回 `BillItemStatus::All`。

#### `BillItem::get_field`

```rust
pub fn get_field(&self, field: &str) -> String
```

按字段名取值，用于 CSV 导出。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `field` | `&str` | 字段名 |

**支持的字段名：**

| field | 返回值 |
|-------|--------|
| `"date_str"` | 日期 |
| `"time_str"` | 时间 |
| `"time_str_formatted"` | 格式化时间 |
| `"date_time_formatted"` | 完整日期时间 |
| `"end_date_time_formatted"` | 结束日期时间 |
| `"timestamp"` | Unix 时间戳字符串 |
| `"end_timestamp"` | 结束时间戳字符串 |
| `"item_type"` | 交易名称 |
| `"number"` | 交易号 |
| `"number_list"` | 逗号分隔的交易号列表 |
| `"target_user"` | 对方账户 |
| `"money_str"` | 金额字符串 |
| `"money"` | 金额数值字符串（两位小数） |
| `"method"` | 支付方式 |
| `"status"` / `"status_str"` | 状态 |
| `"is_combined"` | "true" / "false" |
| 其他 | 空串 |

#### `BillItem::cmp_by_time` / `cmp_by_money`

```rust
pub fn cmp_by_time(&self, other: &Self) -> Ordering
pub fn cmp_by_money(&self, other: &Self) -> Ordering
```

分别按时间戳和金额排序比较器。

### 相等比较

`BillItem` 的 `PartialEq` 基于交易号列表（`number_list`），而非所有字段。两条 `number_list` 相同的账单被视为相等。

### Display

```rust
// 单条输出格式
// "2026.05.21 12:30:45 | 水控消费 | A食堂1楼大餐厅 | 12.50 | 交易成功"

// 合并输出格式
// "2026.05.21 12:30:45 - 2026.05.21 12:35:00 | 水控消费 | 25.00 | [2条合并]"
```

### `sum_money`

```rust
pub fn sum_money(items: &[BillItem]) -> f32
```

对一组账单求金额总和。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `items` | `&[BillItem]` | 账单切片 |

**返回值：** `f32` -- 金额总和

**示例：**

```rust
let total = sum_money(&[item1, item2, item3]);
println!("总消费: {:.2}", total);
```

## `BillType`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillType {
    All,
    NotPaid,
    Success,
    Failure,
}
```

| 变体 | `description()` | `tab_no()` |
|------|-----------------|------------|
| `All` | "全部" | `"1"` |
| `NotPaid` | "未付款" | `"3"` |
| `Success` | "成功" | `"2"` |
| `Failure` | "失败" | `"4"` |

### 方法

```rust
pub fn description(self) -> &'static str   // 中文描述
pub fn tab_no(self) -> &'static str        // 对应 ecard 的 tabNo 参数
```

### `FromStr`

支持从字符串解析：

```rust
use std::str::FromStr;

assert_eq!(BillType::from_str("all"), Ok(BillType::All));
assert_eq!(BillType::from_str("success"), Ok(BillType::Success));
assert_eq!(BillType::from_str("notpaid"), Ok(BillType::NotPaid));
assert_eq!(BillType::from_str("not_paid"), Ok(BillType::NotPaid));
assert_eq!(BillType::from_str("waitfor"), Ok(BillType::NotPaid));
assert_eq!(BillType::from_str("failure"), Ok(BillType::Failure));
assert_eq!(BillType::from_str("fail"), Ok(BillType::Failure));
```

## `BillItemStatus`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillItemStatus {
    All,
    WaitFor,
    Success,
    Failure,
}
```

| 变体 | `description()` |
|------|-----------------|
| `All` | "#all" |
| `WaitFor` | "#waitfor" |
| `Success` | "交易成功" |
| `Failure` | "#fail" |

### 方法

```rust
pub fn description(self) -> &'static str
pub fn from_text(text: &str) -> Option<Self>
```

`from_text` 从页面文本反解枚举：

```rust
assert_eq!(BillItemStatus::from_text("交易成功"), Some(BillItemStatus::Success));
assert_eq!(BillItemStatus::from_text("未知状态"), None);
```

## parser 模块

### `BillParseResult`

```rust
pub struct BillParseResult {
    pub bills: Vec<BillItem>,
    pub total_pages: u32,
}
```

解析一页账单 HTML 的结果，包含账单列表和总页数。

### 账单页解析

| 函数 | 签名 | 说明 |
|------|------|------|
| `parse_bill_page` | `(html: &str) -> Result<BillParseResult>` | 一次性解析整页：账单 + 总页数 |
| `parse_bill_list` | `(html: &str) -> Result<Vec<BillItem>>` | 只取账单条目列表 |
| `get_total_pages` | `(html: &str) -> Result<u32>` | 只抽取分页信息 |

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `html` | `&str` | ecard 账单页 HTML |

**返回值与错误：**
- `parse_bill_page` -> `Result<BillParseResult>`，包含 `bills` 和 `total_pages`
- `parse_bill_list` -> `Result<Vec<BillItem>>`，仅账单列表
- `get_total_pages` -> `Result<u32>`，找不到分页信息返回 `1`

**示例：**

```rust
let html = epay.get_bill(1, "1").await?;
let result = parse_bill_page(&html)?;
println!("总页数: {}", result.total_pages);
println!("本页条数: {}", result.bills.len());
for bill in &result.bills {
    println!("{} | {} | {:.2}", bill.date_time_formatted, bill.item_type, bill.money);
}
```

### 单行账单解析

```rust
pub fn parse_bill_item(row: ElementRef<'_>) -> Option<BillItem>
pub fn parse_bill_item_list(document: &scraper::Html) -> Result<Vec<BillItem>>
```

`parse_bill_item` 将 HTML `<tr>` 元素转为 `BillItem`，解析失败返回 `None`。`parse_bill_item_list` 遍历页面中所有匹配的 `<tr>` 行。

## `parser::hot_water`

```rust
#[derive(Debug, Clone)]
pub struct HotWaterInfo {
    pub building: u32,       // 楼号
    pub temperature: f32,    // 温度
    pub water_level: f32,    // 水位百分比 (%)
}

pub fn parse_hot_water_list(html: &str) -> Result<Vec<HotWaterInfo>>
```

解析热水信息页面 HTML，返回各楼栋的温度和水位数据。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `html` | `&str` | 从 `WechatAuth::get_hot_water()` 获取的 HTML |

**返回值：** `Result<Vec<HotWaterInfo>>`

**示例：**

```rust
let html = wechat.get_hot_water().await?;
let info_list = parse_hot_water_list(&html)?;
for info in &info_list {
    println!("{}号楼: {:.1}℃, 水位{:.0}%", info.building, info.temperature, info.water_level);
}
```

## `parser::export::CsvExporter`

```rust
pub struct CsvExporter { /* headers, fields */ }
```

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `() -> Self` | 默认表头和字段映射 |
| `headers` | `(mut self, headers: Vec<String>) -> Self` | 自定义表头 |
| `fields` | `(mut self, fields: Vec<String>) -> Self` | 自定义字段 |
| `export` | `(&self, path: &str, bills: &[BillItem]) -> Result<()>` | 导出到 CSV 文件 |

### 默认字段映射

| 表头 | 字段 |
|------|------|
| 日期 | date_str |
| 时间 | time_str |
| 时间(格式化) | time_str_formatted |
| 日期时间 | date_time_formatted |
| 时间戳 | timestamp |
| 交易名称 | item_type |
| 交易号 | number |
| 对方 | target_user |
| 金额 | money_str |
| 付款方式 | method |
| 状态 | status |

### `CsvExporter::export`

```rust
pub fn export(&self, path: &str, bills: &[BillItem]) -> Result<()>
```

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `path` | `&str` | 输出 CSV 文件路径 |
| `bills` | `&[BillItem]` | 要导出的账单列表 |

**示例：**

```rust
// 默认导出
CsvExporter::new().export("bills.csv", &bills)?;

// 自定义表头和字段
CsvExporter::new()
    .headers(vec!["日期".into(), "金额".into(), "商户".into()])
    .fields(vec!["date_time_formatted".into(), "money_str".into(), "target_user".into()])
    .export("simple_bills.csv", &bills)?;
```

## `classifier` 模块

### `BillClassifier`

账单分类器，根据交易名称和对方账户的关键词匹配，将账单分为语义类别。

```rust
pub struct BillClassifier {
    pub categories: HashMap<String, CategoryRule>,
}
```

#### `BillCategory`

```rust
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
```

| 方法 | 签名 | 说明 |
|------|------|------|
| `display_name` | `(&self) -> &'static str` | 中文显示名 |
| `emoji` | `(&self) -> &'static str` | 对应 emoji |

#### `CategoryRule`

```rust
pub struct CategoryRule {
    pub name: Vec<String>,     // 匹配 item_type 的关键词
    pub target: Vec<String>,   // 匹配 target_user 的关键词
}
```

#### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `from_json` | `(json: &str) -> Result<Self, serde_json::Error>` | 从 JSON 加载规则 |
| `from_toml` | `(toml_str: &str) -> Result<Self, toml::de::Error>` | 从 TOML 加载规则 |
| `from_file` | `(path: &Path) -> Result<Self>` | 从文件加载（自动识别格式） |
| `classify` | `(&self, name: &str, target: &str) -> BillCategory` | 分类一条账单 |

**JSON 规则格式：**

```json
{
    "deposit": {"name": ["中行云充值", "微信充值"]},
    "bath": {"target": ["淋浴", "热水"]},
    "canteen": {"target": ["食堂", "餐厅"]}
}
```

**示例：**

```rust
let classifier = BillClassifier::from_json(r#"{
    "deposit": {"name": ["中行云充值", "微信充值"]},
    "canteen": {"target": ["食堂", "餐厅"]}
}"#)?;

assert_eq!(classifier.classify("中行云充值", "某商户"), BillCategory::Deposit);
assert_eq!(classifier.classify("消费", "食堂"), BillCategory::Canteen);
assert_eq!(classifier.classify("消费", "未知"), BillCategory::Other);
```

### `PositionTranslator`

将对方账户名（`target_user`）翻译为更友好的楼栋/房间信息。

```rust
pub struct PositionTranslator {
    pub field: String,                        // 匹配字段名
    pub keywords: HashMap<String, PositionEntry>,
}

pub struct PositionEntry {
    pub position: String,  // 楼栋/区域名
    pub room: String,      // 具体位置
}
```

#### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `from_json` | `(json: &str) -> Result<Self, serde_json::Error>` | 从 JSON 加载 |
| `from_toml` | `(toml_str: &str) -> Result<Self, toml::de::Error>` | 从 TOML 加载 |
| `from_file` | `(path: &Path) -> Result<Self>` | 从文件加载 |
| `translate` | `(&self, target_user: &str) -> Option<(String, String)>` | 精确/模糊翻译 |
| `translate_or_raw` | `(&self, target_user: &str) -> (String, String)` | 翻译或返回原文 |

**匹配策略：** 先精确匹配 `keywords` 的 key，再模糊匹配（`target_user` 包含 key）。

**JSON 格式：**

```json
{
    "field": "target",
    "keywords": {
        "A食堂1楼大餐厅": {"position": "海馨楼", "room": "海馨第1食堂"},
        "淋浴": {"position": "公共浴室", "room": "浴室"}
    }
}
```

**示例：**

```rust
let translator = PositionTranslator::from_json(r#"{
    "field": "target",
    "keywords": {
        "A食堂1楼大餐厅": {"position": "海馨楼", "room": "海馨第1食堂"},
        "淋浴": {"position": "公共浴室", "room": "浴室"}
    }
}"#)?;

// 精确匹配
let (pos, room) = translator.translate("A食堂1楼大餐厅").unwrap();
assert_eq!(pos, "海馨楼");

// 模糊匹配
let (pos, room) = translator.translate("淋浴-北区浴室").unwrap();
assert_eq!(pos, "公共浴室");

// 无匹配
assert!(translator.translate("未知地点").is_none());
let (pos, room) = translator.translate_or_raw("未知地点");
assert_eq!(pos, "未知地点");
```

## Tauri 中的实际使用

Tauri 应用中，`BillStoreImpl` 实现了 `BillStore` trait，同时利用 `PositionTranslator` 在写入合并表时自动翻译对方账户：

```rust
// 来自 shmtu-terminal-tauri/src/db/store.rs
impl shmtu_cas::sync::BillStore for BillStoreImpl {
    fn contains(&self, number: &str) -> bool {
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        // 过滤已知交易号，暂存到缓冲区
        self.pending_bills.extend(deduped_bills);
    }
}

// 写入合并表时自动翻译位置
impl BillStoreImpl {
    async fn append_to_merged(&self, bill: &BillItem, now: &str) -> AppResult<()> {
        let (position, room) = self.resolve_position_and_room(&bill.target_user);
        // position/room 写入数据库
    }

    fn resolve_position_and_room(&self, target_user: &str) -> (String, String) {
        self.translator.translate(target_user)
            .unwrap_or_else(|| self.translator.translate_or_raw(target_user))
    }
}
```

账单查询 API 通过 Tauri command 暴露给前端：

```rust
// 来自 shmtu-terminal-tauri/src/commands/bill.rs
#[tauri::command]
pub async fn query_bills(
    state: State<'_, AppState>,
    params: BillQueryParams,
) -> Result<BillQueryResult, String> {
    // 支持按 identity_id、account_id、bill_type、keyword、日期范围筛选
    // 支持分页
}
```

前端 TypeScript 调用示例：

```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke('query_bills', {
    params: {
        identityId: 1,
        billType: 'all',
        page: 1,
        pageSize: 20,
        keyword: '食堂',
        dateStart: '2026-05-01',
        dateEnd: '2026-05-31',
    },
});
// result.items — 账单列表
// result.total — 总数
```
