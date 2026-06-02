---
layout: home

hero:
  name: shmtu-cas-rs
  text: 开发者文档
  tagline: 上海海事大学校园认证与账单同步的 Rust 组件库
  actions:
    - theme: brand
      text: API 总览
      link: /shmtu-cas/api-overview
    - theme: alt
      text: 同步设计
      link: /shmtu-cas/sync
    - theme: alt
      text: 接入示例
      link: /examples/integration

features:
  - title: CAS 登录
    details: EpayAuth 封装探测、挑战、提交、会话恢复全流程，WechatAuth 支持热水侧认证
  - title: 验证码抽象
    details: CaptchaResolver trait 统一手动、远程 TCP/HTTP OCR、本地 ONNX 四种模式，切换无需改主流程
  - title: 增量同步
    details: BillStore trait 解耦存储，SyncOptions 控制策略，连续已知早停 + 时间边界 + 页级进度回调
  - title: 解析与分类
    details: HTML 解析、BillItem 数据模型、BillClassifier 分类、PositionTranslator 位置翻译、CSV 导出
---

## 这套库做什么

shmtu-cas-rs 帮你完成三件事：

1. **登录** — 探测会话状态，获取验证码，提交 CAS 认证
2. **同步** — 逐页拉取消费账单，增量去重，把新数据交还给你的存储层
3. **识别** — 本地 ONNX 或远程服务识别验证码算式

你只需要实现 `BillStore` 的两个方法（`contains` 和 `merge`），就能接入整个同步链路。

## 最快的上手路径

```rust
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe};
use shmtu_cas::captcha::OcrHttpCaptchaResolver;
use shmtu_cas::sync::{incremental_sync, BillStore, SyncOptions};

// 实现 BillStore（决定数据怎么存）
struct MyStore;
impl BillStore for MyStore {
    fn contains(&self, number: &str) -> bool { /* 查重 */ }
    fn merge(&mut self, new_bills: Vec<BillItem>) { /* 存储 */ }
}

// 登录 + 同步
let mut epay = EpayAuth::new()?;
if let LoginProbe::NeedLogin { .. } = epay.probe_login().await? {
    let challenge = epay.prepare_challenge().await?;
    let answer = OcrHttpCaptchaResolver::new("http://127.0.0.1:5000")
        .resolve(&challenge.captcha_image).await?.into_final_answer();
    epay.submit_login("学号", "密码", &answer, &challenge.execution).await?;
}
incremental_sync(&epay, &mut MyStore::default(), &SyncOptions::default()).await?;
```

## 模块一览

| 模块 | 职责 |
|------|------|
| [`cas`](/shmtu-cas/cas-and-login) | CAS 登录、EpayAuth、WechatAuth |
| [`captcha`](/shmtu-cas/captcha) | 验证码下载、CaptchaResolver、四种实现 |
| [`datatype`](/shmtu-cas/parser-and-data) | BillItem、BillType、BillItemStatus |
| [`parser`](/shmtu-cas/parser-and-data) | HTML 解析、CSV 导出、热水信息解析 |
| [`classifier`](/shmtu-cas/parser-and-data) | 账单分类、位置翻译 |
| [`sync`](/shmtu-cas/sync) | BillStore、SyncOptions、增量同步 |

## 工作区成员

| crate | 用途 |
|-------|------|
| `shmtu-cas` | 核心：登录 + 同步 + 解析 + 分类 |
| `shmtu-ocr` | 本地 ONNX 验证码识别 |
| `shmtu-ocr-server` | OCR HTTP/TCP 远程服务 |
| `shmtu-cas-cli` | 命令行调试工具 |
