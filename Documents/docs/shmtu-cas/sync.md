# 同步设计

`sync` 模块是 `shmtu-cas` 最值得关注的部分，因为它决定了库如何和宿主解耦。

## 核心抽象：`BillStore`

```rust
pub trait BillStore: Send + Sync {
    fn contains(&self, number: &str) -> bool;
    fn merge(&mut self, new_bills: Vec<BillItem>);
}
```

设计意图很明确：

- `contains`：告诉库某条交易号是否已存在
- `merge`：把新增账单交还宿主

库不需要知道宿主用的是：

- SQLite
- PostgreSQL
- 文件
- 内存

## `SyncOptions`

字段包括：

- `start_page`
- `max_pages`
- `bill_type`
- `early_stop_threshold`
- `since_timestamp`

这组参数描述的不是“业务动作”，而是“抓取算法约束”。

### 参数含义

`start_page`
: 从第几页开始抓。

`max_pages`
: 最多翻多少页，避免无限翻页或异常死循环。

`bill_type`
: 决定抓取哪个账单标签页。

`early_stop_threshold`
: 连续遇到多少条已知交易号后提前停止。

`since_timestamp`
: 小于该时间戳的记录直接停止继续向后抓取。

## 同步主函数

提供两个入口：

- `incremental_sync`
- `incremental_sync_with_progress`

后者允许宿主在分页级别获取进度通知。

## 增量同步算法

同步流程大致如下：

1. 根据 `bill_type` 计算 `tab_no`
2. 从 `start_page` 开始逐页请求 HTML
3. 使用 parser 解析页码与账单
4. 对每条 `BillItem` 做时间边界判断
5. 用 `BillStore::contains` 判断是否是旧记录
6. 连续命中旧记录达到阈值时提前停止
7. 收集新增账单并统一 `merge`

## 为什么要“连续已知条目早停”

因为账单列表通常按时间倒序排列，增量同步时旧数据会集中出现。连续命中阈值意味着：

- 很可能已经穿过新增数据区
- 继续翻页只会增加请求成本

这是一种偏工程实用主义的优化。

## 为什么 `merge` 在最后统一调用

当前设计会先收集 `new_bills`，再一次性 `merge`。

优点：

- 宿主可以按批处理写库
- 结果更容易做事务控制
- 回调时的新增统计更稳定

代价：

- 单次内存占用略高于边抓边写

## 页级进度回调

`SyncPageProgress` 提供：

- `page`
- `total_pages`
- `new_count`

它不携带宿主概念，比如身份或账号名。这样保证 core lib 仍然和具体 UI/业务对象解耦。

## 宿主常见二次封装

宿主通常会在库之外补充：

- 多账号轮询
- cookies 持久化
- 登录失效恢复
- 验证码弹窗或 OCR 选择
- 原始账单与合并账单两层存储
- 进度事件广播

这也是 Tauri 端 `BillSyncService` 要做的事情。

## 配图占位

### 增量同步时序图

![增量同步时序图占位](/images/screenshots/sync/incremental-sync-sequence.png)

### BillStore 适配示意

![BillStore 适配示意占位](/images/screenshots/sync/billstore-adapter-example.png)
