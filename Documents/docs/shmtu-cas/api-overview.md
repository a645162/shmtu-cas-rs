# `shmtu-cas` API 总览

`shmtu-cas` 是整个工作区的核心业务库，通过 `src/lib.rs` 导出六个模块：

```rust
pub mod captcha;
pub mod cas;
pub mod datatype;
pub mod classifier;
pub mod parser;
pub mod sync;
```

每个模块的职责边界和完整 API 参考见子页面：

- [CAS 与登录流程](/shmtu-cas/cas-and-login) — `cas` 模块
- [验证码抽象](/shmtu-cas/captcha) — `captcha` 模块
- [解析器与数据模型](/shmtu-cas/parser-and-data) — `datatype` + `parser` + `classifier` 模块
- [同步设计](/shmtu-cas/sync) — `sync` 模块

## 快速索引

| 想做什么 | 用什么 | 在哪 |
|---------|--------|------|
| 创建 HTTP 客户端 | `cas::create_client()` | cas |
| 获取 CAS execution 令牌 | `cas::get_execution()` | cas |
| 提交 CAS 登录 | `cas::cas_login()` | cas |
| 跟随重定向 | `cas::cas_redirect()` | cas |
| 消费侧登录与账单 | `cas::epay::EpayAuth` | cas |
| 热水侧登录与查询 | `cas::wechat::WechatAuth` | cas |
| 下载验证码图片 | `captcha::fetch_captcha()` | captcha |
| 算式求值 | `captcha::get_expr_result()` | captcha |
| 验证码解析器 trait | `captcha::CaptchaResolver` | captcha |
| 手动验证码 | `captcha::ManualCaptchaResolver` | captcha |
| 远程 TCP OCR | `captcha::OcrCaptchaResolver` | captcha |
| 远程 HTTP OCR | `captcha::OcrHttpCaptchaResolver` | captcha |
| 账单数据结构 | `datatype::bill::BillItem` | datatype |
| 账单类型枚举 | `datatype::bill::BillType` | datatype |
| 账单状态枚举 | `datatype::bill::BillItemStatus` | datatype |
| 解析账单 HTML | `parser::parse_bill_page()` | parser |
| 导出 CSV | `parser::export::CsvExporter` | parser |
| 解析热水信息 | `parser::hot_water::parse_hot_water_list()` | parser |
| 账单分类 | `classifier::BillClassifier` | classifier |
| 位置翻译 | `classifier::PositionTranslator` | classifier |
| 增量同步 | `sync::incremental_sync()` | sync |
| 带进度的增量同步 | `sync::incremental_sync_with_progress()` | sync |

## 推荐接入方式

最简接入：创建 `EpayAuth`，选择一种 `CaptchaResolver`，调用 `incremental_sync`。

```rust
use anyhow::Result;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::captcha::OcrHttpCaptchaResolver;
use shmtu_cas::sync::{incremental_sync, BillStore, SyncOptions};
use shmtu_cas::datatype::bill::BillItem;

struct MyStore;

impl BillStore for MyStore {
    fn contains(&self, number: &str) -> bool { false }
    fn merge(&mut self, new_bills: Vec<BillItem>) { /* 保存 */ }
}

async fn run_sync() -> Result<()> {
    let mut epay = EpayAuth::new()?;

    match epay.probe_login().await? {
        LoginProbe::AlreadyLoggedIn => {}
        LoginProbe::NeedLogin { .. } => {
            let challenge = epay.prepare_challenge().await?;
            let resolver = OcrHttpCaptchaResolver::new("http://127.0.0.1:5000");
            let answer = resolver.resolve(&challenge.captcha_image).await?.into_final_answer();
            epay.submit_login("student_id", "password", &answer, &challenge.execution).await?;
        }
    }

    let mut store = MyStore;
    incremental_sync(&epay, &mut store, &SyncOptions::default()).await?;
    Ok(())
}
```

## API 设计特点

- 以 trait 隔离宿主依赖（`BillStore`、`CaptchaResolver`）
- 以结构体参数承载同步策略（`SyncOptions`）
- 以模块边界区分网络、解析、分类、同步
- 以 `Result<T>` 暴露错误，不吞掉失败原因
- 以枚举化结果（`LoginProbe`、`LoginSubmitResult`）便于宿主做 UI 分支

## 模块依赖关系

```
captcha ──┐
          ├── cas ── sync ── (BillStore)
datatype ─┤              │
parser ───┘              └── parser
classifier ──────────────── (可选，宿主层调用)
```

- `sync` 依赖 `cas`（获取 HTML）和 `parser`（解析 HTML）
- `cas` 依赖 `captcha`（获取验证码图片）
- `datatype` 被所有模块共享
- `classifier` 独立使用，不在同步主链路中
