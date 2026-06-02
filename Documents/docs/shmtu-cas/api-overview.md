# `shmtu-cas` API 总览

`src/lib.rs` 当前导出以下模块：

```rust
pub mod captcha;
pub mod cas;
pub mod datatype;
pub mod classifier;
pub mod parser;
pub mod sync;
```

这说明它的 API 设计是“按职责分模块暴露”，而不是只给一个大而杂的 facade。

## 模块职责

## `cas`

职责：

- 创建 HTTP client
- 获取 `execution`
- 执行 CAS 登录
- 跟随重定向
- 封装 `EpayAuth` / `WechatAuth`

适合宿主在“访问远端系统”这一层使用。

## `captcha`

职责：

- 下载验证码图片
- 提供 `CaptchaResolver` 抽象
- 定义 `CaptchaAnswer` 与 `CaptchaAnswerKind`
- 封装远程 TCP/HTTP OCR 与手动解析器

适合宿主在“验证码求解策略”这一层使用。

## `datatype`

职责：

- 定义账单数据结构与类型枚举
- 提供合并、金额求和、字段导出能力

适合在持久化层和展示层之间做统一数据交换。

## `parser`

职责：

- 解析 HTML 页面
- 提取页数
- 提取账单项
- 导出 CSV

适合在“原始 HTML -> 结构化数据”的边界使用。

## `classifier`

职责：

- `BillClassifier`：交易类别判定
- `PositionTranslator`：对方账户到楼栋/房间的映射

适合做补充标签，不适合作为核心真值来源。

## `sync`

职责：

- 定义 `BillStore`
- 定义 `SyncOptions`
- 提供带进度回调的增量同步

这是宿主二次封装时最重要的入口。

## 推荐接入方式

```rust
use anyhow::Result;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::sync::{incremental_sync, BillStore, SyncOptions};

struct MyStore;

impl BillStore for MyStore {
    fn contains(&self, number: &str) -> bool {
        let _ = number;
        false
    }

    fn merge(&mut self, new_bills: Vec<shmtu_cas::datatype::bill::BillItem>) {
        let _ = new_bills;
    }
}

async fn run_sync() -> Result<()> {
    let mut epay = EpayAuth::new()?;
    match epay.probe_login().await? {
        LoginProbe::AlreadyLoggedIn => {}
        LoginProbe::NeedLogin { .. } => {
            let challenge = epay.prepare_challenge().await?;
            let captcha = String::from_utf8_lossy(&challenge.captcha_image);
            let _ = captcha;
            let result = epay
                .submit_login("username", "password", "1234", &challenge.execution)
                .await?;
            match result {
                LoginSubmitResult::Success => {}
                _ => anyhow::bail!("login failed"),
            }
        }
    }

    let mut store = MyStore;
    incremental_sync(&epay, &mut store, &SyncOptions::default()).await?;
    Ok(())
}
```

上面示例只表达接入顺序，不表达真实验证码求解方式。

## API 设计特点

- 以 trait 隔离宿主依赖
- 以结构体参数承载同步策略
- 以模块边界区分网络、解析、分类、同步
- 以 `Result<T>` 暴露错误，而不是吞掉失败原因

## 配图占位

### API 模块关系图

![API 模块关系图占位](/images/screenshots/api/api-module-map.png)
