# 同步设计

`sync` 模块是 `shmtu-cas` 最值得关注的部分，因为它决定了库如何和宿主解耦。

## 核心抽象：`BillStore`

```rust
pub trait BillStore: Send + Sync {
    fn contains(&self, number: &str) -> bool;
    fn merge(&mut self, new_bills: Vec<BillItem>);
}
```

### `BillStore::contains`

```rust
fn contains(&self, number: &str) -> bool
```

判断某条交易号是否已存在于本地。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `number` | `&str` | 交易号（纯数字字符串） |

**返回值：** `bool` -- 已存在返回 `true`

**用途：** 增量同步时用于判断是否是旧记录，是"连续已知早停"的基础。

### `BillStore::merge`

```rust
fn merge(&mut self, new_bills: Vec<BillItem>)
```

将新增条目交还宿主。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `new_bills` | `Vec<BillItem>` | 本次同步发现的新增账单 |

**注意：** 宿主自行决定持久化策略（SQLite、PostgreSQL、文件、内存）。

## `SyncOptions`

```rust
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub start_page: u32,
    pub max_pages: u32,
    pub bill_type: BillType,
    pub early_stop_threshold: u32,
    pub since_timestamp: Option<i64>,
}
```

### 字段详解

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `start_page` | `u32` | `1` | 从第几页开始抓 |
| `max_pages` | `u32` | `100` | 最多翻多少页，防止无限翻页 |
| `bill_type` | `BillType` | `BillType::All` | 抓取哪个账单标签页 |
| `early_stop_threshold` | `u32` | `5` | 连续遇到多少条已知交易号后早停 |
| `since_timestamp` | `Option<i64>` | `None` | 仅同步该时间戳之后的账单 |

### 参数含义与典型配置

**`start_page`**

从第几页开始抓取。默认 `1`（最新页）。不需要修改，除非要做断点续传。

**`max_pages`**

安全上限，防止因解析异常导致无限翻页。

| 场景 | 推荐值 |
|------|--------|
| 增量同步（最近一周） | `100` |
| 全量同步 | `1000` |

**`bill_type`**

决定请求哪个标签页（对应 `tab_no`）。

| `BillType` | `tab_no` | 含义 |
|------------|----------|------|
| `All` | `"1"` | 全部 |
| `Success` | `"2"` | 成功 |
| `NotPaid` | `"3"` | 未付款 |
| `Failure` | `"4"` | 失败 |

**`early_stop_threshold`**

连续遇到已知交易号的数量达到此值时，停止翻页。

| 场景 | 推荐值 |
|------|--------|
| 增量同步 | `5`-`10` |
| 全量同步 | `u32::MAX`（禁用早停） |

**`since_timestamp`**

Unix 时间戳（秒），小于此值的账单直接跳过。

| 场景 | 推荐值 |
|------|--------|
| 最近一周 | `now - 7 days` |
| 最近一个月 | `now - 30 days` |
| 不限时间 | `None` |

### 构造示例

```rust
use shmtu_cas::sync::SyncOptions;
use shmtu_cas::datatype::bill::BillType;

// 默认增量同步
let opts = SyncOptions::default();

// 最近一周的增量同步
let opts = SyncOptions {
    start_page: 1,
    max_pages: 100,
    bill_type: BillType::All,
    early_stop_threshold: 10,
    since_timestamp: Some(
        (chrono::Local::now() - chrono::Duration::days(7)).timestamp()
    ),
};

// 全量同步（禁用早停）
let opts = SyncOptions {
    start_page: 1,
    max_pages: 1000,
    bill_type: BillType::All,
    early_stop_threshold: u32::MAX,
    since_timestamp: None,
};
```

## 同步主函数

### `incremental_sync`

```rust
pub async fn incremental_sync(
    epay: &EpayAuth,
    store: &mut dyn BillStore,
    options: &SyncOptions,
) -> Result<SyncResult>
```

增量同步：逐页拉取账单，用交易号去重，遇到连续 N 条已知条目则早停。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `epay` | `&EpayAuth` | 已登录的 EpayAuth 实例 |
| `store` | `&mut dyn BillStore` | 宿主提供的数据存储 |
| `options` | `&SyncOptions` | 同步参数 |

**返回值：** `Result<SyncResult>`

### `incremental_sync_with_progress`

```rust
pub async fn incremental_sync_with_progress<F>(
    epay: &EpayAuth,
    store: &mut dyn BillStore,
    options: &SyncOptions,
    progress_callback: Option<&F>,
) -> Result<SyncResult>
where
    F: Fn(SyncPageProgress) + Send + Sync + ?Sized
```

带页级进度回调的增量同步。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `epay` | `&EpayAuth` | 已登录的 EpayAuth 实例 |
| `store` | `&mut dyn BillStore` | 宿主提供的数据存储 |
| `options` | `&SyncOptions` | 同步参数 |
| `progress_callback` | `Option<&F>` | 页级进度回调，每完成一页触发一次 |

## `SyncResult`

```rust
#[derive(Debug)]
pub struct SyncResult {
    pub new_count: usize,
    pub pages_fetched: u32,
    pub early_stopped: bool,
    pub new_bills: Vec<BillItem>,
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `new_count` | `usize` | 本次新增的条目数 |
| `pages_fetched` | `u32` | 翻了多少页 |
| `early_stopped` | `bool` | 是否因早停条件而终止 |
| `new_bills` | `Vec<BillItem>` | 所有新增条目 |

## `SyncPageProgress`

```rust
#[derive(Debug, Clone)]
pub struct SyncPageProgress {
    pub page: u32,
    pub total_pages: u32,
    pub new_count: usize,
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `page` | `u32` | 当前已拉取到的页面编号 |
| `total_pages` | `u32` | 账单总页数 |
| `new_count` | `usize` | 截至本页累计发现的新账单数 |

**设计要点：** 不携带宿主概念（如身份或账号名），保证 core lib 与具体 UI/业务对象解耦。

## 增量同步算法

同步流程大致如下：

1. 根据 `bill_type` 计算 `tab_no`
2. 从 `start_page` 开始逐页请求 HTML
3. 使用 parser 解析页码与账单
4. 对每条 `BillItem` 做时间边界判断（`since_timestamp`）
5. 用 `BillStore::contains` 判断是否是旧记录
6. 连续命中旧记录达到阈值时提前停止
7. 收集新增账单并统一 `merge`

### 为什么要"连续已知条目早停"

因为账单列表通常按时间倒序排列，增量同步时旧数据会集中出现。连续命中阈值意味着：

- 很可能已经穿过新增数据区
- 继续翻页只会增加请求成本

### 为什么 `merge` 在最后统一调用

当前设计会先收集 `new_bills`，再一次性 `merge`。

**优点：**
- 宿主可以按批处理写库
- 结果更容易做事务控制
- 回调时的新增统计更稳定

**代价：**
- 单次内存占用略高于边抓边写

## Tauri 中的完整同步实现

Tauri 应用中的 `BillSyncService` 展示了完整的宿主层封装：

### 多账号同步

```rust
// 来自 shmtu-terminal-tauri/src/sync/mod.rs
impl BillSyncService {
    pub async fn sync_identity(
        &self,
        identity_id: i64,
        sync_range: SyncRangePreset,
        progress_callback: Option<&SyncProgressCallback>,
    ) -> AppResult<IdentitySyncResult> {
        // 1. 获取该身份下所有启用的账号
        let accounts = self.get_enabled_accounts_for_identity(identity_id).await?;

        // 2. 根据验证码模式选择同步流程
        if matches!(cfg.captcha_mode(), CaptchaMode::Manual) {
            // 手动模式：逐账号同步，遇验证码暂停等待前端输入
            self.sync_identity_manual(identity_id, sync_options, progress_callback).await
        } else {
            // 自动模式：使用配置的 OCR 服务自动登录
            self.do_sync(identity_id, &sync_options, progress_callback).await
        }
    }
}
```

### 会话恢复与持久化

```rust
// 尝试复用已保存的会话
async fn try_sync_with_saved_session(&self, account: &Account, ...) -> AppResult<Option<...>> {
    if let Some(session) = self.db_manager.get_session(&account.account_id, &self.crypto).await? {
        let mut epay = EpayAuth::new()?;
        epay.restore_session(&session.cookies)?;
        if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
            // 会话有效，直接同步
            return Ok(Some(self.sync_logged_in_account(...).await?));
        }
    }
    Ok(None) // 需要重新登录
}

// 登录成功后保存会话
async fn save_session(&self, epay: &EpayAuth, account_id: &str) -> AppResult<()> {
    let cookies_json = epay.extract_session()?;
    self.db_manager.save_session(account_id, &cookies_json, &self.crypto).await?;
    Ok(())
}
```

### 手动验证码交互

Tauri 应用中，手动验证码流程通过错误码 + Tauri 事件实现：

```rust
// 后端：需要验证码时返回特殊错误
LoginProbe::NeedLogin { .. } => {
    let challenge = epay.prepare_challenge().await?;
    let image = BASE64.encode(&challenge.captcha_image);
    let execution = challenge.execution.clone();

    // 暂存同步状态
    self.store_pending_manual_sync(PendingManualSync { ... }).await;

    // 返回特殊错误，前端识别后弹出验证码输入框
    return Err(AppError::Sync(format!(
        "MANUAL_CAPTCHA_REQUIRED|{}|{}",
        image, execution
    )));
}

// 前端用户输入后调用
pub async fn sync_with_captcha(
    &self,
    identity_id: i64,
    captcha_code: &str,
    execution: &str,
    progress_callback: Option<&SyncProgressCallback>,
) -> AppResult<IdentitySyncResult> {
    let mut pending = self.take_pending_manual_sync().await.ok_or_else(|| ...)?;

    match pending.epay.submit_login(
        &pending.current_account.account_id,
        &password,
        captcha_code,
        &pending.execution,
    ).await? {
        LoginSubmitResult::Success => {
            // 继续同步，处理下一个账号
            self.continue_pending_manual_sync(pending, progress_callback).await
        }
        LoginSubmitResult::ValidateCodeError => {
            // 验证码错误，刷新新验证码
            let challenge = pending.epay.prepare_challenge().await?;
            Err(AppError::Sync(format!("CAPTCHA_WRONG|{}|{}", image, execution)))
        }
        // ...
    }
}
```

### 前端 TypeScript 交互

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// 监听同步进度
listen('sync-progress', (event) => {
    const progress = event.payload;

    if (progress.captcha_required) {
        // 显示验证码弹窗
        showCaptchaDialog(progress.captcha_image, progress.execution);
    } else if (progress.is_running) {
        // 更新进度条
        updateProgressBar(progress.current_page, progress.total_pages);
    } else if (progress.status === 'completed') {
        // 同步完成
        showSyncResult(progress.new_items);
    }
});

// 发起增量同步
await invoke('incremental_sync', {
    identityId: 1,
    syncRange: 'month',
});

// 提交验证码答案
await invoke('sync_with_captcha', {
    identityId: 1,
    captchaCode: '8',
    execution: 'e1s1-xxxx-...',
});
```

### `BillStoreImpl` 实现

Tauri 应用中 `BillStoreImpl` 实现了 `BillStore` trait，同时管理两层存储（原始表 + 合并表）：

```rust
// 来自 shmtu-terminal-tauri/src/db/store.rs
impl shmtu_cas::sync::BillStore for BillStoreImpl {
    fn contains(&self, number: &str) -> bool {
        // 基于 known_numbers HashSet，O(1) 查找
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        // 过滤已知交易号，暂存到 pending_bills 缓冲区
        // 后续调用 flush_pending_bills() 一次性写入
    }
}

// 同步完成后刷入数据库
async fn sync_logged_in_account(&self, ...) -> AppResult<AccountSyncResult> {
    let sync_result = shmtu_cas::sync::incremental_sync_with_progress(
        epay, &mut store, sync_options, ...
    ).await?;

    // 将缓冲区中的新账单写入原始表和合并表
    store.flush_pending_bills().await?;

    Ok(AccountSyncResult {
        new_count: sync_result.new_count,
        pages_fetched: sync_result.pages_fetched,
        early_stopped: sync_result.early_stopped,
        ..
    })
}
```
