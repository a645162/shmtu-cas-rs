# 接入示例

本页给出一个“宿主如何组合这些 crate”的思路，而不是完整生产代码。

## 目标

宿主要完成以下事情：

1. 维护账号与会话
2. 获取验证码
3. 调用某种 resolver
4. 登录校园系统
5. 增量同步账单
6. 把新账单落到自己的存储层

## 建议的宿主层结构

可以按下面方式组织：

- `session_service`：管理 cookies 的保存与恢复
- `captcha_service`：根据配置选择手动 / HTTP / TCP / ONNX
- `sync_service`：封装多账号、多身份同步
- `repository`：实现 `BillStore`

## 一个极简的 `BillStore`

```rust
use shmtu_cas::datatype::bill::BillItem;
use shmtu_cas::sync::BillStore;

#[derive(Default)]
struct MemoryStore {
    numbers: std::collections::HashSet<String>,
    items: Vec<BillItem>,
}

impl BillStore for MemoryStore {
    fn contains(&self, number: &str) -> bool {
        self.numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        for bill in new_bills {
            self.numbers.insert(bill.number.clone());
            self.items.push(bill);
        }
    }
}
```

## 一个极简的同步组合

```rust
use anyhow::Result;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::sync::{incremental_sync, SyncOptions};

async fn sync_once() -> Result<()> {
    let mut epay = EpayAuth::new()?;
    let mut store = MemoryStore::default();

    match epay.probe_login().await? {
        LoginProbe::AlreadyLoggedIn => {}
        LoginProbe::NeedLogin { .. } => {
            let challenge = epay.prepare_challenge().await?;
            let captcha_code = "1234";
            let result = epay
                .submit_login("student_id", "password", captcha_code, &challenge.execution)
                .await?;
            match result {
                LoginSubmitResult::Success => {}
                _ => anyhow::bail!("login failed"),
            }
        }
    }

    let options = SyncOptions {
        max_pages: 20,
        early_stop_threshold: 5,
        ..SyncOptions::default()
    };

    let result = incremental_sync(&epay, &mut store, &options).await?;
    println!("new_count={}", result.new_count);
    Ok(())
}
```

## 真正落地时通常还要补什么

- cookies 恢复与更新
- 验证码失败重试
- 账号密码错误处理
- 进度上报
- 原始 HTML 保存
- 原始账单与聚合账单分层存储

这部分不属于 core lib，而应该由宿主承担。

## 配图占位

### 宿主集成流程图

![宿主集成流程图占位](/images/screenshots/integration/tauri-adapter-flow.png)
