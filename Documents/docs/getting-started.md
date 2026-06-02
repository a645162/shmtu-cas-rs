# 快速开始

本节帮助你在 `shmtu-cas-rs` 仓库内启动开发文档，并明确常用入口。

## 仓库结构

工作区主要由四部分组成：

- `Core/shmtu-cas`
- `Core/shmtu-cas-cli`
- `ocr/shmtu-ocr`
- `ocr/shmtu-ocr-server`

文档站位于：

- `Documents/docs`：VitePress 内容
- `Documents/docs/.vitepress`：站点配置

## 本地运行文档

在 `Documents` 目录下执行：

```bash
npm install
npm run docs:dev
```

构建静态站点：

```bash
npm run docs:build
```

## Rust 开发入口

### 核心库

```bash
cargo build -p shmtu-cas
```

### 命令行调试

```bash
cargo run -p shmtu-cas-cli
```

### OCR 服务

```bash
cargo run -p shmtu-ocr-server -- --model-dir ./models
```

## 你需要先理解的三个边界

### 1. `shmtu-cas` 不负责宿主持久化

同步层通过 `BillStore` trait 与外部交互。也就是说：

- 库不绑定 SQLite
- 库不绑定 JSON
- 库不绑定 Tauri

宿主程序自己决定存储策略。

### 2. OCR 是独立能力，不强耦合在同步主链路里

验证码处理被抽象为 `CaptchaResolver`，因此你可以：

- 手动输入
- 远程 TCP OCR
- 远程 HTTP OCR
- 本地 ONNX OCR

### 3. `EpayAuth` 负责校园消费侧的登录与账单页访问

它不替宿主管理业务状态，只负责：

- 探测登录态
- 取 challenge
- 提交登录
- 拉账单 HTML

## 建议阅读顺序

1. 先看 [整体架构](/architecture)
2. 再看 [API 总览](/shmtu-cas/api-overview)
3. 接着看 [同步设计](/shmtu-cas/sync)
4. 若需要 OCR，再看 [shmtu-ocr](/shmtu-ocr/onnx)
