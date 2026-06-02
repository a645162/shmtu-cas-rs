# 解析器与数据模型

## `datatype::bill`

该模块导出：

- `BillItem`
- `BillItemStatus`
- `BillType`
- `sum_money`

## `BillItem`

`BillItem` 是核心数据载体，承载：

- 日期与时间
- 交易名称
- 交易号
- 对方账户
- 金额
- 支付方式
- 交易状态

它还提供了几类实用方法：

- `new_single(...)`
- `merge(...)`
- `merge_with(...)`
- `status()`
- `get_field(field)`
- 比较器与金额工具

设计上它既能承接 parser 输出，也能承接导出层与同步层的数据交换。

## `BillType`

`BillType` 既是业务筛选枚举，也是同步层构造 `tabNo` 的来源。

关键方法：

- `description()`
- `tab_no()`

## `BillItemStatus`

用于把文本状态规约到可判断的枚举表示。

关键方法：

- `description()`
- `from_text(text)`

## parser 模块

### 账单页解析

主要入口：

- `parse_bill_page(html)`
- `parse_bill_list(html)`
- `get_total_pages(html)`

其中：

- `parse_bill_page` 返回账单列表加总页数
- `parse_bill_list` 只关心账单项本身
- `get_total_pages` 只抽取分页信息

### 单行账单解析

`parse_bill_item(row)` 负责把 HTML 行节点转成 `BillItem`。

这种拆分使得解析器可以：

- 单独测试单行规则
- 单独测试分页规则
- 在页面结构轻微变化时更容易定位问题

## `parser::export::CsvExporter`

`CsvExporter` 提供一个很小的导出能力：

- 默认表头
- 默认字段映射
- 可自定义 headers
- 可自定义 fields

设计意图是给宿主一个“可直接用，也可定制”的最低成本导出器，而不是承诺一个大而全的报表系统。

## `classifier`

`classifier` 里有两个不同性质的组件：

### `BillClassifier`

用于按规则将账单分类成语义类别。

支持：

- `from_json`
- `from_toml`
- `from_file`
- `classify(name, target)`

### `PositionTranslator`

用于把 `target_user` 翻译成更友好的 `position + room`。

支持：

- `from_json`
- `from_toml`
- `from_file`
- `translate`
- `translate_or_raw`

这两个组件都属于“解释层”，能增强可读性，但不应该篡改原始账单真值。
