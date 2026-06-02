# 验证码抽象

验证码处理被抽象为 `CaptchaResolver` trait，宿主可自由选择识别方式，无需修改登录主流程。

## 核心类型

### `CaptchaAnswerKind`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptchaAnswerKind {
    /// 一个完整算式（如 "12+34="），调用方还需要计算
    Expression,
    /// 已经是最终答案
    Answer,
}
```

| 变体 | 含义 | 使用场景 |
|------|------|---------|
| `Expression` | OCR 返回原始算式字符串 | 远程 OCR 服务通常返回 `"3+5=8"` |
| `Answer` | 已计算出最终答案 | 手动输入或已解析完毕 |

### `CaptchaAnswer`

```rust
#[derive(Debug, Clone)]
pub struct CaptchaAnswer {
    pub value: String,
    pub kind: CaptchaAnswerKind,
}
```

**构造方法：**

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `(value: impl Into<String>, kind: CaptchaAnswerKind) -> Self` | 通用构造 |
| `answer` | `(value: impl Into<String>) -> Self` | 直接标记为最终答案 |
| `expression` | `(value: impl Into<String>) -> Self` | 标记为算式表达式 |

**关键方法：**

| 方法 | 签名 | 说明 |
|------|------|------|
| `into_final_answer` | `(self) -> String` | 不论是答案还是算式，都规约为最终答案字符串 |

`into_final_answer` 内部逻辑：
- `Answer` -> 直接返回 `value`
- `Expression` -> 调用 `get_expr_result(&value)` 提取 `=` 右侧数字

**示例：**

```rust
use shmtu_cas::captcha::{CaptchaAnswer, CaptchaAnswerKind};

// 从算式自动提取答案
let a = CaptchaAnswer::expression("3+5=8");
assert_eq!(a.into_final_answer(), "8");

// 直接给答案
let b = CaptchaAnswer::answer("8");
assert_eq!(b.into_final_answer(), "8");

// 算式无等号，返回 trim 后的原值
let c = CaptchaAnswer::expression("42");
assert_eq!(c.into_final_answer(), "42");
```

### `ResolveFuture`

```rust
pub type ResolveFuture<'a> = Pin<Box<dyn Future<Output = Result<CaptchaAnswer>> + Send + 'a>>;
```

异步 Future 类型别名，所有 resolver 的 `resolve` 方法返回此类型。

## `CaptchaResolver` trait

```rust
pub trait CaptchaResolver: Send + Sync {
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a>;
}
```

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `image_data` | `&[u8]` | 验证码图片的原始字节（通常是 PNG/JPEG） |

**返回值：** `ResolveFuture<'a>`，解析后得到 `Result<CaptchaAnswer>`

**设计要点：**
- 输入只是一段图片字节，不关心图片来源
- 输出是 `CaptchaAnswer`，保留算式或答案两种语义
- 以异步 future 表达，支持远程网络调用
- `Send + Sync` 约束确保可跨线程使用

## 已有实现

### `ManualCaptchaResolver`

把验证码图片交给用户/外部回调拿到答案。

```rust
pub struct ManualCaptchaResolver { /* ... */ }
```

**构造：**

```rust
pub fn new(
    handler: Box<
        dyn for<'a> Fn(&'a [u8]) -> Pin<Box<dyn Future<Output = Result<CaptchaAnswer>> + Send + 'a>>
            + Send + Sync
    >
) -> Self
```

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `handler` | 闭包 | 接收图片字节，返回异步 Future 产出 `CaptchaAnswer` |

**示例 -- Tauri 中的手动验证码：**

```rust
use shmtu_cas::captcha::{ManualCaptchaResolver, CaptchaAnswer};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

// 在 Tauri 应用中，弹窗让用户输入验证码
let resolver = ManualCaptchaResolver::new(Box::new(|image_data| {
    Box::pin(async move {
        let base64_image = BASE64.encode(image_data);
        // 将 base64_image 发送到前端展示
        // 等待用户从 UI 输入答案
        let user_input = wait_for_user_input().await;
        Ok(CaptchaAnswer::answer(user_input))
    })
}));
```

**适合场景：** UI 程序、首次调试、OCR 不稳定时的回退路径

### `ExprCaptchaResolver`

调用方传入算式字符串，自动解析为答案。

```rust
pub struct ExprCaptchaResolver<F>
where
    F: Fn(&[u8]) -> String + Send + Sync,
{ /* ... */ }
```

**构造：**

```rust
pub fn new(expr_provider: F) -> Self
```

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `expr_provider` | `F: Fn(&[u8]) -> String + Send + Sync` | 接收图片字节，返回识别出的算式字符串 |

**工作流程：**
1. 调用 `expr_provider(image_data)` 获取算式字符串（如 `"3+5=8"`）
2. 调用 `get_expr_result()` 提取最终答案
3. 返回 `CaptchaAnswer::answer(答案)`

**示例：**

```rust
use shmtu_cas::captcha::ExprCaptchaResolver;

let resolver = ExprCaptchaResolver::new(|_image_data| {
    // 外部已识别好的算式
    "3+5=8".to_string()
});
```

**适合场景：** 已有外部表达式提供器，只需规约到统一接口

### `OcrCaptchaResolver`

通过远端 TCP OCR 服务识别验证码。

```rust
pub struct OcrCaptchaResolver { /* ... */ }
```

**构造：**

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `(host: impl Into<String>, port: u16) -> Self` | 指定服务地址和端口 |
| `with_retries` | `(mut self, retries: usize) -> Self` | 设置最大重试次数（默认 3） |
| `from_ocr` | `(ocr: CaptchaOcr) -> Self` | 从已有的 `CaptchaOcr` 实例构造 |

**TCP 协议：**
1. 连接到 `host:port`（超时 5s）
2. 发送 `image_data` + `<END>` 标记
3. 读取响应直到连接关闭（超时 10s）
4. 返回响应文本（算式字符串）

**示例：**

```rust
use shmtu_cas::captcha::OcrCaptchaResolver;

let resolver = OcrCaptchaResolver::new("127.0.0.1", 5001)
    .with_retries(5); // 最多重试 5 次

let answer = resolver.resolve(&image_data).await?;
println!("表达式: {}, 答案: {}", answer.value, answer.into_final_answer());
```

**适合场景：** 已有 TCP OCR 服务部署的局域网环境

### `OcrHttpCaptchaResolver`

通过 RESTful HTTP OCR 服务识别验证码。

```rust
pub struct OcrHttpCaptchaResolver { /* ... */ }
```

**构造：**

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `(base_url: &str) -> Self` | 指定 OCR 服务基地址 |
| `with_retries` | `(mut self, retries: usize) -> Self` | 设置最大重试次数（默认 3） |
| `from_ocr` | `(ocr: CaptchaOcrHttp) -> Self` | 从已有的 `CaptchaOcrHttp` 实例构造 |

**HTTP 请求格式：**

```
POST {base_url}/api/ocr
Content-Type: application/json

{
    "imageBase64": "<base64 编码的图片数据>"
}
```

**HTTP 响应格式：**

```json
{
    "success": true,
    "expression": "3+5=8",
    "result": 8,
    "error": null
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `success` | `bool` | 是否识别成功 |
| `expression` | `Option<String>` | 识别出的算式 |
| `result` | `Option<i32>` | 计算结果 |
| `error` | `Option<String>` | 错误信息 |

**示例：**

```rust
use shmtu_cas::captcha::OcrHttpCaptchaResolver;

let resolver = OcrHttpCaptchaResolver::new("http://127.0.0.1:5000")
    .with_retries(3);

let answer = resolver.resolve(&image_data).await?;
// answer.kind == CaptchaAnswerKind::Expression
// answer.value == "3+5=8"
let final_answer = answer.into_final_answer(); // "8"
```

**适合场景：** 服务化与容器化部署、Docker 环境

### 底层 OCR 客户端

#### `CaptchaOcr`（TCP）

```rust
pub struct CaptchaOcr { /* host: String, port: u16 */ }

impl CaptchaOcr {
    pub fn new(host: &str, port: u16) -> Self;
    /// 单次识别，失败直接返回错误
    pub fn ocr_by_remote_tcp(&self, image_data: &[u8]) -> Result<String>;
    /// 自动重试，最多 max_retries 次，每次间隔 1s
    pub fn ocr_auto_retry(&self, image_data: &[u8], max_retries: usize) -> Result<String>;
}
```

#### `CaptchaOcrHttp`（HTTP）

```rust
pub struct CaptchaOcrHttp { /* base_url, async_client */ }

impl CaptchaOcrHttp {
    pub fn new(base_url: &str) -> Self;

    // 阻塞方法
    pub fn ocr_by_http(&self, image_data: &[u8]) -> Result<String>;
    pub fn ocr_auto_retry(&self, image_data: &[u8], max_retries: usize) -> Result<String>;
    pub fn health_check(&self) -> Result<bool>;

    // 异步方法
    pub async fn ocr_by_http_async(&self, image_data: &[u8]) -> Result<String>;
    pub async fn ocr_auto_retry_async(&self, image_data: &[u8], max_retries: usize) -> Result<String>;
    pub async fn health_check_async(&self) -> Result<bool>;
}
```

## `fetch_captcha`

```rust
pub async fn fetch_captcha(client: &Client) -> Result<Vec<u8>>
```

从 CAS 服务器下载验证码图片字节。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `client` | `&reqwest::Client` | 已配置的 HTTP 客户端 |

**返回值：** `Result<Vec<u8>>` -- 图片的原始字节（PNG 格式）

**请求地址：** `https://cas.shmtu.edu.cn/cas/captcha`

**注意：** 此函数通常由 `EpayAuth::prepare_challenge()` 内部调用，宿主无需直接使用。

## `get_expr_result`

```rust
pub fn get_expr_result(expr: &str) -> String
```

从算式字符串中提取最终答案。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `expr` | `&str` | 算式字符串，如 `"3+5=8"` 或 `"42"` |

**返回值：** `String` -- `=` 右侧的数字，无 `=` 则返回 trim 后的原值

**示例：**

```rust
use shmtu_cas::captcha::get_expr_result;

assert_eq!(get_expr_result("12+34=46"), "46");
assert_eq!(get_expr_result("3+5=8"), "8");
assert_eq!(get_expr_result("10-3=7"), "7");
assert_eq!(get_expr_result("42"), "42");
```

## 设计优点

- 宿主切换验证码实现时，不需要重写登录主流程
- OCR 服务失败时，可以直接回退到手动模式
- 单元测试时可以提供假 resolver，绕过真实 OCR 依赖
- 所有 resolver 统一返回 `CaptchaAnswer`，宿主只需调用 `into_final_answer()`

## Tauri 中的实际使用

Tauri 应用根据用户配置动态选择 resolver：

```rust
// 来自 shmtu-terminal-tauri/src/sync/mod.rs 的 login_auto 方法
let captcha_code = match cfg.captcha_mode() {
    CaptchaMode::RemoteOcr => {
        shmtu_cas::captcha::OcrCaptchaResolver::new(&host, port)
            .with_retries(max_attempts)
            .resolve(&challenge.captcha_image)
            .await?
            .into_final_answer()
    }
    CaptchaMode::RemoteOcrHttp => {
        shmtu_cas::captcha::OcrHttpCaptchaResolver::new(&http_url)
            .with_retries(max_attempts)
            .resolve(&challenge.captcha_image)
            .await?
            .into_final_answer()
    }
    CaptchaMode::Manual => {
        // 将验证码图片 base64 编码发送到前端
        let image = BASE64.encode(&challenge.captcha_image);
        // 通过 Tauri 事件通知前端显示验证码弹窗
        return Err("MANUAL_CAPTCHA_REQUIRED|...".into());
    }
    CaptchaMode::LocalOnnx => {
        // 使用 shmtu-ocr 本地 ONNX 推理
        let expr = local_ocr_backend.predict_bytes(&challenge.captcha_image)?;
        shmtu_cas::captcha::get_expr_result(&expr)
    }
};
```

Tauri 前端通过事件监听验证码请求：

```typescript
// 前端 TypeScript 代码
import { listen } from '@tauri-apps/api/event';

listen('sync-progress', (event) => {
    const progress = event.payload;
    if (progress.captcha_required) {
        // 显示验证码弹窗，展示 progress.captcha_image (base64)
        // 用户输入后调用 sync_with_captcha 命令
        invoke('sync_with_captcha', {
            identityId: currentIdentityId,
            captchaCode: userInput,
            execution: progress.execution,
        });
    }
});
```
