# CAS 与登录流程

## 低层接口：`cas` 模块

`cas` 模块提供了一组偏底层的函数：

- `create_client()`
- `get_execution(client, url)`
- `cas_login(client, url, username, password, validate_code, execution)`
- `cas_redirect(client, url)`

这些函数关注的是 HTTP 交互细节，不关心你把状态放在哪里。

## 高层封装：`EpayAuth`

在实际账单抓取场景中，更常用的是 `cas::epay::EpayAuth`。

它提供以下核心能力：

- `new()`
- `restore_session(cookies_json)`
- `extract_session()`
- `probe_login()`
- `prepare_challenge()`
- `submit_login(...)`
- `test_login_status()`
- `get_bill(page_no, tab_no)`

## `EpayAuth` 的职责边界

它负责：

- 管理 HTTP client
- 管理 CookieJar
- 探测是否已登录
- 抓取登录 challenge
- 执行登录提交
- 获取账单页 HTML

它不负责：

- 持久化 cookies
- 解验证码
- 存储账单
- 控制同步策略

## 登录探测设计

`probe_login()` 返回：

- `AlreadyLoggedIn`
- `NeedLogin { login_url }`

这样做的意义是，宿主不需要预先知道当前会话是否有效，可以统一走探测流程。

## challenge 设计

`prepare_challenge()` 返回 `LoginChallenge`：

- `execution`
- `captcha_image`

这相当于把验证码识别决策权交给宿主。宿主可以：

- 手动输入
- 调用远端 OCR
- 调用本地 OCR

## 登录提交结果

`submit_login()` 返回 `LoginSubmitResult`：

- `Success`
- `ValidateCodeError`
- `PasswordError`
- `Failure(String)`

这样的枚举化结果比只抛一个字符串更适合宿主做 UI 分支和重试逻辑。

## 会话设计

`restore_session` 与 `extract_session` 基于 JSON 字符串工作。

优点：

- 易存储
- 易跨进程传递
- 易和数据库或配置文件集成

代价：

- 会话语义仍由宿主自己保证
- JSON 格式只是传递介质，不是安全边界

## 账单访问

`get_bill(page_no, tab_no)` 直接返回 HTML 字符串，而不是返回 `BillItem`。

这样做是合理的，因为：

- 抓取与解析保持解耦
- parser 可以独立测试
- 宿主也可以在必要时保存原始 HTML 做调试

## `WechatAuth`

工作区还提供了 `cas::wechat::WechatAuth`，用于另一类认证/查询场景。它与 `EpayAuth` 的设计风格一致：

- 先 probe
- 再 prepare challenge
- 最后 submit login

如果未来还要接别的校园入口，这种封装方式是可复用的。
