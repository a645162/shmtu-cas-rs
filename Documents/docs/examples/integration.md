# 接入示例

本页给出宿主如何组合这些 crate 的完整思路，从最简示例到 Tauri 生产级实现。

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

## 示例 1：最简内存实现

适合快速验证和单元测试。

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

## 示例 2：最简同步组合

```rust
use anyhow::Result;
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::captcha::OcrHttpCaptchaResolver;
use shmtu_cas::sync::{incremental_sync, SyncOptions};

async fn sync_once() -> Result<()> {
    let mut epay = EpayAuth::new()?;
    let mut store = MemoryStore::default();

    match epay.probe_login().await? {
        LoginProbe::AlreadyLoggedIn => {}
        LoginProbe::NeedLogin { .. } => {
            let challenge = epay.prepare_challenge().await?;
            let resolver = OcrHttpCaptchaResolver::new("http://127.0.0.1:5000");
            let answer = resolver.resolve(&challenge.captcha_image).await?.into_final_answer();
            epay.submit_login("student_id", "password", &answer, &challenge.execution).await?;
        }
    }

    let options = SyncOptions {
        max_pages: 20,
        early_stop_threshold: 5,
        ..SyncOptions::default()
    };

    let result = incremental_sync(&epay, &mut store, &options).await?;
    println!("new_count={}, pages={}, early_stopped={}",
        result.new_count, result.pages_fetched, result.early_stopped);
    Ok(())
}
```

## 示例 3：带进度回调的同步

```rust
use shmtu_cas::sync::{incremental_sync_with_progress, SyncPageProgress};

async fn sync_with_progress() -> Result<()> {
    let callback = |progress: SyncPageProgress| {
        println!("页 {}/{} | 新增 {} 条",
            progress.page, progress.total_pages, progress.new_count);
    };

    let result = incremental_sync_with_progress(
        &epay, &mut store, &options, Some(&callback)
    ).await?;
    Ok(())
}
```

## 示例 4：会话持久化与恢复

```rust
async fn sync_with_session_persistence(account_id: &str) -> Result<()> {
    let mut epay = EpayAuth::new()?;

    // 尝试恢复上次的会话
    if let Ok(cookies) = std::fs::read_to_string(format!("sessions/{}.json", account_id)) {
        epay.restore_session(&cookies)?;
    }

    match epay.probe_login().await? {
        LoginProbe::AlreadyLoggedIn => {
            println!("会话有效，跳过登录");
        }
        LoginProbe::NeedLogin { .. } => {
            let challenge = epay.prepare_challenge().await?;
            let answer = resolver.resolve(&challenge.captcha_image).await?.into_final_answer();
            epay.submit_login(account_id, "password", &answer, &challenge.execution).await?;
        }
    }

    // 保存会话供下次使用
    let cookies = epay.extract_session()?;
    std::fs::write(format!("sessions/{}.json", account_id), cookies)?;

    // 同步...
    Ok(())
}
```

## 示例 5：Tauri 生产级集成

以下展示 Tauri 桌面应用中的完整集成架构。

### 后端：AppState 初始化

```rust
// 来自 shmtu-terminal-tauri/src/state.rs
pub struct AppState {
    pub db_manager: Arc<RwLock<DatabaseManager>>,
    pub crypto: Arc<RwLock<CryptoService>>,
    pub sync_service: Arc<RwLock<BillSyncService>>,
    pub classifier: Arc<RwLock<Option<BillClassifier>>>,
    pub db_file_manager: Arc<DatabaseFileManager>,
    pub local_ocr: Arc<std::sync::Mutex<Option<CasOnnxBackend>>>,
}

impl AppState {
    pub async fn init(data_dir: &str) -> AppResult<Self> {
        let db_manager = DatabaseManager::connect(data_dir).await?;
        let crypto = CryptoService::from_device_id("shmtu-terminal-device-key");

        // 加载位置翻译器（从本地数据库文件或 GitHub 下载）
        let db_file_manager = DatabaseFileManager::new(&db_local_dir);
        db_file_manager.ensure_local_files().await?;
        let position_translator = db_file_manager.create_position_translator();

        // 初始化同步服务
        let sync_service = BillSyncService::new(
            db_manager.clone_ref(),
            crypto.clone(),
            position_translator.clone(),
        );

        // 加载分类器
        let classifier = BillClassifier::from_file(rules_path)?;

        Ok(Self { /* ... */ })
    }
}
```

### 后端：Tauri Commands

```rust
// 来自 shmtu-terminal-tauri/src/commands/sync.rs

#[tauri::command]
pub async fn incremental_sync(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    sync_range: SyncRangePreset,
) -> Result<SyncProgressFrontend, String> {
    let sync_service = state.sync_service.read().await;
    let progress_callback = create_progress_callback(app.clone());
    sync_service.sync_identity(identity_id, sync_range, Some(&progress_callback)).await
        .map(|r| SyncProgressFrontend::success(r.total_new_count))
        .map_err(|e| {
            if e.to_string().starts_with("MANUAL_CAPTCHA_REQUIRED|") {
                return SyncProgressFrontend::captcha_required(image, execution, "请输入验证码");
            }
            e.to_string()
        })
}

#[tauri::command]
pub async fn sync_with_captcha(
    state: State<'_, AppState>,
    app: AppHandle,
    identity_id: i64,
    captcha_code: String,
    execution: String,
) -> Result<SyncProgressFrontend, String> {
    let sync_service = state.sync_service.read().await;
    sync_service.sync_with_captcha(identity_id, &captcha_code, &execution, Some(&callback)).await
}

#[tauri::command]
pub async fn query_bills(
    state: State<'_, AppState>,
    params: BillQueryParams,
) -> Result<BillQueryResult, String> { /* ... */ }
```

### 前端：React + TypeScript

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// 1. 发起同步
async function startSync(identityId: number) {
    const result = await invoke('incremental_sync', {
        identityId,
        syncRange: 'month',
    });
}

// 2. 监听进度和验证码需求
listen('sync-progress', (event) => {
    const p = event.payload as SyncProgress;

    if (p.captcha_required) {
        setCaptchaDialog({
            visible: true,
            image: `data:image/png;base64,${p.captcha_image}`,
            execution: p.execution,
        });
    } else if (p.is_running) {
        setProgress({
            page: p.current_page,
            total: p.total_pages,
            newItems: p.new_items,
            message: p.message,
        });
    } else if (p.status === 'completed') {
        setProgress({ done: true, newItems: p.new_items });
    }
});

// 3. 提交验证码答案
async function submitCaptcha(captchaCode: string, execution: string) {
    const result = await invoke('sync_with_captcha', {
        identityId: currentIdentityId,
        captchaCode,
        execution,
    });
    if (result.status === 'captcha_required') {
        setCaptchaDialog({ image: result.captcha_image, execution: result.execution });
    }
}

// 4. 查询账单
async function loadBills(page: number) {
    const result = await invoke('query_bills', {
        params: {
            identityId: currentIdentityId,
            billType: 'all',
            page,
            pageSize: 20,
        },
    });
    setBills(result.items);
    setTotalCount(result.total);
}
```

### 后端：验证码模式选择

```rust
// 来自 shmtu-terminal-tauri/src/sync/mod.rs
async fn login_auto(&self, username: &str, password: &str) -> AppResult<EpayAuth> {
    let cfg = ConfigAccess::new(&self.db_manager);
    let max_attempts = cfg.ocr_retry_count().max(1);

    for attempt in 1..=max_attempts {
        let mut epay = EpayAuth::new()?;
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;

        let captcha_code = match cfg.captcha_mode() {
            CaptchaMode::RemoteOcr => {
                OcrCaptchaResolver::new(&cfg.remote_ocr_host(), cfg.remote_ocr_port())
                    .with_retries(max_attempts)
                    .resolve(&challenge.captcha_image).await?.into_final_answer()
            }
            CaptchaMode::RemoteOcrHttp => {
                OcrHttpCaptchaResolver::new(&cfg.remote_ocr_http_url())
                    .with_retries(max_attempts)
                    .resolve(&challenge.captcha_image).await?.into_final_answer()
            }
            CaptchaMode::Manual => {
                return Err(AppError::Sync("当前验证码模式不支持自动登录".into()));
            }
            CaptchaMode::LocalOnnx => {
                let expr = local_ocr_backend.predict_bytes(&challenge.captcha_image)?;
                get_expr_result(&expr)
            }
        };

        match epay.submit_login(username, password, &captcha_code, &challenge.execution).await? {
            LoginSubmitResult::Success => return Ok(epay),
            LoginSubmitResult::ValidateCodeError if attempt < max_attempts => continue,
            LoginSubmitResult::PasswordError => return Err("密码错误".into()),
            LoginSubmitResult::Failure(msg) => return Err(msg.into()),
        }
    }
    Err("验证码识别多次失败".into())
}
```

## 真正落地时通常还要补什么

- cookies 恢复与更新
- 验证码失败重试
- 账号密码错误处理
- 进度上报
- 原始 HTML 保存
- 原始账单与聚合账单分层存储
- 多账号串行同步与错误隔离
- 会话过期自动检测与刷新
- 已毕业账号自动跳过

这部分不属于 core lib，而应该由宿主承担。Tauri 应用的 `BillSyncService` 已经完整覆盖了以上所有场景。
