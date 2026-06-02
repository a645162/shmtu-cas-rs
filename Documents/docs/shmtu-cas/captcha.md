# 验证码抽象

## 为什么单独抽象验证码

校园验证码的部署条件变化很大：

- 有的现场允许人工输入
- 有的现场已有 OCR 服务
- 有的现场只能离线本地推理

如果把验证码逻辑写死在登录流程里，宿主会很难适配。

## 核心类型

### `CaptchaAnswerKind`

表示解析器返回的是：

- `Expression`
- `Answer`

### `CaptchaAnswer`

字段：

- `value`
- `kind`

并提供：

- `answer(...)`
- `expression(...)`
- `into_final_answer()`

这意味着解析器既可以直接给最终数字，也可以只给算式文本。

## `CaptchaResolver` trait

```rust
pub trait CaptchaResolver: Send + Sync {
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a>;
}
```

这是验证码设计的核心扩展点。

要求很克制：

- 输入只是一段图片字节
- 输出是 `CaptchaAnswer`
- 以异步 future 表达

这让不同实现能共享同一宿主逻辑。

## 已有实现

### `ManualCaptchaResolver`

适合：

- UI 程序
- 首次调试
- OCR 不稳定时的回退路径

### `ExprCaptchaResolver`

适合：

- 已有外部表达式提供器
- 只需要把“表达式 -> 答案”规约到统一接口

### `OcrCaptchaResolver`

走远程 TCP OCR 服务。

适合已有 socket 服务部署的场景。

### `OcrHttpCaptchaResolver`

走远程 HTTP OCR 服务。

适合服务化与容器化部署。

## `fetch_captcha`

`fetch_captcha(client)` 用于直接下载验证码图片字节。

这是一个故意保持低层的函数。它不隐含任何识别策略，只负责把远端 challenge 取回来。

## 设计优点

- 宿主切换验证码实现时，不需要重写登录主流程
- OCR 服务失败时，可以直接回退到手动模式
- 单元测试时可以提供假 resolver，绕过真实 OCR 依赖

## 宿主集成建议

推荐把验证码策略放在宿主配置中，而不是把某个 resolver 写死在业务代码里。

一个常见模式是：

1. 宿主根据配置选择 resolver
2. `prepare_challenge()` 获取图片
3. `resolver.resolve(image_data)` 得到答案
4. 把答案提交给 `submit_login()`
