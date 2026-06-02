# CAS 与登录流程

## 低层接口：`cas` 模块

`cas` 模块提供一组偏底层的 HTTP 交互函数，关注协议细节，不关心业务状态。

### `create_client`

```rust
pub fn create_client() -> Result<Client>
```

创建用于 CAS 交互的 HTTP 客户端。

**返回值：** `Result<reqwest::Client>`

**客户端配置：**
- 禁止自动重定向（`redirect::Policy::none()`）
- 启用 Cookie 存储（`cookie_store(true)`）
- User-Agent: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36`

**错误：** 若 `Client::builder().build()` 失败，返回 `anyhow::Error`

**示例：**

```rust
use shmtu_cas::cas::create_client;

let client = create_client()?;
// 后续所有 cas 函数都需要这个 client
```

### `get_execution`

```rust
pub async fn get_execution(client: &Client, url: &str) -> Result<String>
```

从 CAS 登录页 HTML 中提取 `execution` 隐藏表单字段的值。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `client` | `&Client` | HTTP 客户端 |
| `url` | `&str` | CAS 登录页 URL |

**返回值：** `Result<String>` -- `execution` 字段的 value

**内部流程：**
1. GET 请求 `url`
2. 检查状态码是否 200
3. 解析 HTML，查找 `<input name="execution">`
4. 提取 `value` 属性

**错误情况：**
- 状态码非 200 -> `"获取登录页面失败，状态码: XXX"`
- HTML 中找不到 execution 元素 -> `"未找到execution元素"`

**示例：**

```rust
let execution = get_execution(&client, "https://cas.shmtu.edu.cn/cas/login?service=...").await?;
// execution 类似 "e1s1-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
```

### `cas_login`

```rust
pub async fn cas_login(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    validate_code: &str,
    execution: &str,
) -> Result<CasAuthResult>
```

提交 CAS 登录表单。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `client` | `&Client` | HTTP 客户端 |
| `url` | `&str` | CAS 登录提交 URL |
| `username` | `&str` | 学号/用户名（会 trim） |
| `password` | `&str` | 密码（会 trim） |
| `validate_code` | `&str` | 验证码答案（会 trim） |
| `execution` | `&str` | 从 `get_execution` 获取的令牌（会 trim） |

**返回值：** `Result<CasAuthResult>`

**提交的表单字段：**

| 字段名 | 值 |
|--------|-----|
| `username` | username.trim() |
| `password` | password.trim() |
| `validateCode` | validate_code.trim() |
| `execution` | execution.trim() |
| `_eventId` | `"submit"` |
| `geolocation` | `""` |

### `CasAuthResult`

```rust
#[derive(Debug, PartialEq)]
pub enum CasAuthResult {
    Success { location: String },
    ValidateCodeError,
    PasswordError,
    Failure(String),
}
```

| 变体 | 含义 | 判断依据 |
|------|------|---------|
| `Success { location }` | 登录成功，`location` 为重定向目标 | HTTP 302 |
| `ValidateCodeError` | 验证码错误 | 错误面板含 "reCAPTCHA" 或 "验证码" |
| `PasswordError` | 用户名或密码错误 | 错误面板含 "account is not recognized" 或 "用户名或密码" |
| `Failure(msg)` | 其他失败 | 错误面板的原始文本 |

### `cas_redirect`

```rust
pub async fn cas_redirect(client: &Client, url: &str) -> Result<()>
```

跟随 CAS 重定向链（最多 10 次）。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `client` | `&Client` | HTTP 客户端 |
| `url` | `&str` | 起始重定向 URL |

**返回值：** `Result<()>`

**内部流程：** 循环 GET 请求，遇到 302/301 则跳转到 Location 头，直到返回 200 或无 Location 头，最多 10 轮。

## 高层封装：`EpayAuth`

`cas::epay::EpayAuth` 是实际使用中最常见的入口，封装了消费侧的登录与账单访问。

### 结构定义

```rust
pub struct EpayAuth { /* 内部含 client, cookies, login_url */ }

pub enum LoginProbe {
    AlreadyLoggedIn,
    NeedLogin { login_url: String },
}

pub enum LoginSubmitResult {
    Success,
    ValidateCodeError,
    PasswordError,
    Failure(String),
}

pub struct LoginChallenge {
    pub execution: String,
    pub captcha_image: Vec<u8>,
}
```

### 方法总览

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `() -> Result<Self>` | 创建新实例 |
| `restore_session` | `(cookies_json: &str) -> Result<()>` | 从 JSON 恢复 cookies |
| `extract_session` | `() -> Result<String>` | 导出 cookies 为 JSON |
| `probe_login` | `async () -> Result<LoginProbe>` | 探测登录状态 |
| `prepare_challenge` | `async () -> Result<LoginChallenge>` | 获取 execution + 验证码图片 |
| `submit_login` | `async (username, password, validate_code, execution) -> Result<LoginSubmitResult>` | 提交登录 |
| `test_login_status` | `async () -> Result<bool>` | 测试是否已登录 |
| `get_bill` | `async (page_no: u32, tab_no: &str) -> Result<String>` | 获取账单页 HTML |

### `EpayAuth::new`

```rust
pub fn new() -> Result<Self>
```

创建新的 `EpayAuth`，内部调用 `create_client()` 初始化 HTTP 客户端和空 CookieJar。

**示例：**

```rust
let mut epay = EpayAuth::new()?;
```

### `EpayAuth::restore_session`

```rust
pub fn restore_session(&mut self, cookies_json: &str) -> Result<()>
```

从 JSON 字符串恢复 cookies，用于会话持久化。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `cookies_json` | `&str` | 之前通过 `extract_session()` 获取的 JSON 字符串 |

**JSON 格式示例：**

```json
{
    "JSESSIONID": { "value": "A1B2C3D4...", "domain": null },
    "TGC": { "value": "TGT-xxxx-...", "domain": null }
}
```

**错误：** JSON 解析失败时返回错误

**示例：**

```rust
// 从存储中恢复
let saved_cookies = db.get_session(&account_id)?;
epay.restore_session(&saved_cookies)?;
```

### `EpayAuth::extract_session`

```rust
pub fn extract_session(&self) -> Result<String>
```

将当前 CookieJar 导出为 JSON 字符串，供持久化存储。

**返回值：** `Result<String>` -- JSON 格式的 cookies

**示例：**

```rust
// 登录成功后保存
let cookies_json = epay.extract_session()?;
db.save_session(&account_id, &cookies_json)?;
```

### `EpayAuth::probe_login`

```rust
pub async fn probe_login(&mut self) -> Result<LoginProbe>
```

探测当前会话是否已登录消费系统。

**返回值：** `Result<LoginProbe>`

| 变体 | 含义 | HTTP 状态 |
|------|------|----------|
| `AlreadyLoggedIn` | 已登录，可直接访问账单 | 200 |
| `NeedLogin { login_url }` | 未登录，`login_url` 为 CAS 登录入口 | 302 |

**内部流程：**
1. 请求 `https://ecard.shmtu.edu.cn/epay/consume/query?pageNo=1&tabNo=1`
2. 提取响应中的 `Set-Cookie` 头更新 CookieJar
3. 200 -> `AlreadyLoggedIn`，302 -> `NeedLogin`，其他 -> 错误

**示例：**

```rust
match epay.probe_login().await? {
    LoginProbe::AlreadyLoggedIn => {
        println!("已登录，直接同步");
    }
    LoginProbe::NeedLogin { login_url } => {
        println!("需要登录，URL: {}", login_url);
    }
}
```

### `EpayAuth::prepare_challenge`

```rust
pub async fn prepare_challenge(&self) -> Result<LoginChallenge>
```

获取登录所需的 `execution` 令牌和验证码图片。

**前提：** 必须先调用 `probe_login()` 且返回 `NeedLogin`

**返回值：** `Result<LoginChallenge>`

```rust
pub struct LoginChallenge {
    pub execution: String,       // CAS 表单令牌
    pub captcha_image: Vec<u8>,  // 验证码图片字节 (PNG)
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `execution` | `String` | CAS 登录表单必需的隐藏字段值 |
| `captcha_image` | `Vec<u8>` | 验证码图片原始字节 |

**内部流程：**
1. 从 `probe_login` 保存的 `login_url` 获取 `execution`
2. 从 `https://cas.shmtu.edu.cn/cas/captcha` 下载验证码图片

**示例：**

```rust
let challenge = epay.prepare_challenge().await?;
// challenge.execution 类似 "e1s1-xxxx..."
// challenge.captcha_image 是 PNG 图片字节
```

### `EpayAuth::submit_login`

```rust
pub async fn submit_login(
    &self,
    username: &str,
    password: &str,
    validate_code: &str,
    execution: &str,
) -> Result<LoginSubmitResult>
```

提交登录凭证，完成 CAS 认证。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `username` | `&str` | 学号 |
| `password` | `&str` | 密码 |
| `validate_code` | `&str` | 验证码答案（如 `"8"`） |
| `execution` | `&str` | 从 `prepare_challenge()` 获取的令牌 |

**返回值：** `Result<LoginSubmitResult>`

| 变体 | 含义 |
|------|------|
| `Success` | 登录成功 |
| `ValidateCodeError` | 验证码错误，可重试 |
| `PasswordError` | 用户名或密码错误 |
| `Failure(msg)` | 其他错误 |

**示例：**

```rust
let result = epay.submit_login("2024001", "mypassword", "8", &challenge.execution).await?;
match result {
    LoginSubmitResult::Success => println!("登录成功"),
    LoginSubmitResult::ValidateCodeError => println!("验证码错误，需重新获取"),
    LoginSubmitResult::PasswordError => println!("密码错误"),
    LoginSubmitResult::Failure(msg) => println!("其他错误: {}", msg),
}
```

### `EpayAuth::test_login_status`

```rust
pub async fn test_login_status(&self) -> Result<bool>
```

简单测试是否已登录。返回 `true` 表示已登录。

### `EpayAuth::get_bill`

```rust
pub async fn get_bill(&self, page_no: u32, tab_no: &str) -> Result<String>
```

获取消费账单页面的 HTML。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `page_no` | `u32` | 页码（从 1 开始） |
| `tab_no` | `&str` | 标签页编号，由 `BillType::tab_no()` 生成 |

**tab_no 对照：**

| `BillType` | `tab_no` | 含义 |
|------------|----------|------|
| `All` | `"1"` | 全部 |
| `Success` | `"2"` | 成功 |
| `NotPaid` | `"3"` | 未付款 |
| `Failure` | `"4"` | 失败 |

**返回值：** `Result<String>` -- 账单页 HTML 字符串

**错误情况：**
- 302 重定向 -> `"未登录，需要重新登录"`
- 其他非 200 -> `"获取账单失败，状态码: XXX"`

**请求地址：** `https://ecard.shmtu.edu.cn/epay/consume/query?pageNo={page_no}&tabNo={tab_no}`

**示例：**

```rust
let html = epay.get_bill(1, "1").await?; // 第1页，全部类型
```

## `WechatAuth`

`cas::wechat::WechatAuth` 用于热水系统认证，设计风格与 `EpayAuth` 一致。

### 结构定义

```rust
pub struct WechatAuth { /* ... */ }

pub struct LoginChallenge {
    pub execution: String,
    pub captcha_image: Vec<u8>,
    pub login_url: String,  // 注意：比 EpayAuth 多了此字段
}
```

### 方法总览

| 方法 | 签名 | 说明 |
|------|------|------|
| `new` | `() -> Result<Self>` | 创建新实例 |
| `probe_login` | `async () -> Result<LoginProbe>` | 探测热水系统登录状态 |
| `prepare_challenge` | `async (ticket_url: &str) -> Result<LoginChallenge>` | 获取验证码和 execution |
| `submit_login` | `async (username, password, validate_code, execution) -> Result<LoginSubmitResult>` | 提交登录 |
| `test_login_status` | `async () -> Result<bool>` | 测试是否已登录 |
| `get_hot_water` | `async () -> Result<String>` | 获取热水信息 HTML |

### 与 `EpayAuth` 的差异

| 特性 | EpayAuth | WechatAuth |
|------|----------|------------|
| 目标系统 | 消费账单 | 热水信息 |
| `probe_login` | 自动获取 login_url | 返回 ticket_url |
| `prepare_challenge` | 无参数 | 需要 `ticket_url` 参数 |
| `LoginChallenge` | 无 login_url 字段 | 包含 login_url 字段 |
| 数据获取 | `get_bill(page_no, tab_no)` | `get_hot_water()` |
| 会话管理 | `restore_session` / `extract_session` | 无（需宿主自行管理） |

### `WechatAuth::prepare_challenge`

```rust
pub async fn prepare_challenge(&mut self, ticket_url: &str) -> Result<LoginChallenge>
```

与 `EpayAuth` 不同，需要传入 `probe_login` 返回的 `ticket_url`，先跟随一次 `wengine_new_ticket` 跳转获取真正的 CAS 登录 URL。

**参数：**

| 参数 | 类型 | 说明 |
|------|------|------|
| `ticket_url` | `&str` | `probe_login` 返回的跳转 URL |

### `WechatAuth::get_hot_water`

```rust
pub async fn get_hot_water(&self) -> Result<String>
```

获取热水信息 HTML，可用 `parser::hot_water::parse_hot_water_list()` 解析。

**请求地址：** `http://hqzx.shmtu.edu.cn/cellphone/getHotWater`

**示例：**

```rust
let mut wechat = WechatAuth::new()?;
match wechat.probe_login().await? {
    LoginProbe::AlreadyLoggedIn => {}
    LoginProbe::NeedLogin { ticket_url } => {
        let challenge = wechat.prepare_challenge(&ticket_url).await?;
        let answer = resolver.resolve(&challenge.captcha_image).await?.into_final_answer();
        wechat.submit_login("student_id", "password", &answer, &challenge.execution).await?;
    }
}
let html = wechat.get_hot_water().await?;
let info_list = shmtu_cas::parser::hot_water::parse_hot_water_list(&html)?;
```

## 完整登录流程示例

### 手动验证码模式（Tauri GUI）

```rust
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

async fn login_manual() -> Result<EpayAuth> {
    let mut epay = EpayAuth::new()?;

    // 1. 探测登录状态
    if let LoginProbe::NeedLogin { .. } = epay.probe_login().await? {
        // 2. 获取验证码
        let challenge = epay.prepare_challenge().await?;

        // 3. 在 Tauri UI 中展示验证码图片（base64 编码）
        let base64_image = BASE64.encode(&challenge.captcha_image);
        // 通过 Tauri 事件发送到前端
        app.emit("captcha-required", CaptchaPayload {
            image: base64_image,
            execution: challenge.execution.clone(),
        })?;

        // 4. 等待前端回调返回用户输入
        let captcha_code = wait_for_user_input().await;

        // 5. 提交登录
        match epay.submit_login("2024001", "password", &captcha_code, &challenge.execution).await? {
            LoginSubmitResult::Success => {}
            LoginSubmitResult::ValidateCodeError => {
                // 验证码错误 -> 重新获取
                let new_challenge = epay.prepare_challenge().await?;
                // ... 重试
            }
            LoginSubmitResult::PasswordError => bail!("密码错误"),
            LoginSubmitResult::Failure(msg) => bail!("登录失败: {}", msg),
        }
    }

    // 6. 保存会话供下次复用
    let cookies = epay.extract_session()?;
    save_to_storage(&cookies);

    Ok(epay)
}
```

### 自动 OCR 模式（带重试）

```rust
use shmtu_cas::captcha::OcrHttpCaptchaResolver;

async fn login_auto(username: &str, password: &str) -> Result<EpayAuth> {
    let max_attempts = 3;

    for attempt in 1..=max_attempts {
        let mut epay = EpayAuth::new()?;
        epay.probe_login().await?;
        let challenge = epay.prepare_challenge().await?;

        let resolver = OcrHttpCaptchaResolver::new("http://127.0.0.1:5000");
        let captcha_code = resolver.resolve(&challenge.captcha_image).await?.into_final_answer();

        match epay.submit_login(username, password, &captcha_code, &challenge.execution).await? {
            LoginSubmitResult::Success => {
                // 二次确认登录状态
                if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
                    return Ok(epay);
                }
            }
            LoginSubmitResult::ValidateCodeError if attempt < max_attempts => continue,
            LoginSubmitResult::PasswordError => bail!("密码错误"),
            LoginSubmitResult::Failure(msg) => bail!("登录失败: {}", msg),
            _ => {}
        }
    }

    bail!("验证码识别多次失败")
}
```

### 会话恢复模式（Tauri 典型用法）

Tauri 应用中，会话以加密 JSON 存储在数据库，下次启动时恢复：

```rust
async fn login_with_saved_session(account_id: &str) -> Result<EpayAuth> {
    let mut epay = EpayAuth::new()?;

    // 尝试恢复已保存的会话
    if let Some(session) = db.get_session(account_id, &crypto).await? {
        epay.restore_session(&session.cookies)?;
        if let Ok(LoginProbe::AlreadyLoggedIn) = epay.probe_login().await {
            return Ok(epay); // 会话有效，直接使用
        }
    }

    // 会话过期或不存在 -> 需要重新登录
    // ... 走正常登录流程
}
```
