---
layout: home

hero:
  name: shmtu-cas-rs
  text: 开发者文档
  tagline: 面向维护者与集成方的架构说明、crate 划分、同步设计与 lib API 设计文档
  actions:
    - theme: brand
      text: 快速开始
      link: /getting-started
    - theme: alt
      text: API 总览
      link: /shmtu-cas/api-overview
    - theme: alt
      text: 同步设计
      link: /shmtu-cas/sync

features:
  - title: 先讲边界，再讲实现
    details: 把网络抓取、验证码求解、HTML 解析、同步算法、OCR 推理拆成清晰层次，便于维护和二次封装。
  - title: 面向集成方
    details: 重点解释 shmtu-cas 如何通过 BillStore、SyncOptions、CaptchaResolver 与宿主程序解耦。
  - title: 覆盖 lib API 设计
    details: 文档不只列模块名，也解释为什么 API 长这样、扩展点在哪里、适合宿主在哪一层二次封装。
  - title: 预留技术截图
    details: 已为架构图、时序图、OCR 示例、服务接口截图预留统一静态资源目录。
---

## 文档范围

`shmtu-cas-rs` 是围绕上海海事大学校园认证、账单抓取与验证码识别建立的一组 Rust 组件。它不是单一 crate，而是一个工作区，包含：

- `shmtu-cas`：CAS 登录、账单抓取、HTML 解析、分类与同步抽象
- `shmtu-ocr`：本地 ONNX 验证码识别库
- `shmtu-ocr-server`：OCR HTTP/TCP 服务
- `shmtu-cas-cli` / `shmtu-ocr-cli` / `shmtu-ocr-gui`：调试与测试入口

这套文档面向开发者，重点说明：

- 工作区如何划分
- 各 crate 的职责边界
- `lib` 的 API 设计思路
- 同步流程如何和宿主程序对接
- OCR 组件如何独立部署或嵌入

## 阅读顺序

- 想先把文档站跑起来：看 [快速开始](/getting-started)
- 想理解模块关系：看 [整体架构](/architecture)
- 想对接 `shmtu-cas`：看 [API 总览](/shmtu-cas/api-overview)
- 想理解增量同步：看 [同步设计](/shmtu-cas/sync)
- 想集成本地 OCR：看 [shmtu-ocr](/shmtu-ocr/onnx)

## 截图占位目录

开发者文档截图占位已放到：

- `src-tauri/vendor/shmtu-cas-rs/Documents/docs/public/images/screenshots/`

当前已预留：

- `architecture/`
- `api/`
- `sync/`
- `ocr/`
- `server/`
- `integration/`

## 非目标

本文档不负责解释 Tauri 桌面程序如何使用，那部分内容已经拆分到主仓库的用户文档站中。
