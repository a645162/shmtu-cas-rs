# 工作区与 Crate

## `shmtu-cas`

位置：`Core/shmtu-cas`

职责：

- CAS 登录
- 校园消费账单抓取
- 验证码解析器抽象
- 账单 HTML 解析
- 同步算法
- 分类与位置翻译

它是整个工作区的核心业务库。

## `shmtu-cas-cli`

位置：`Core/shmtu-cas-cli`

职责：

- 作为轻量命令行入口验证核心库能力
- 在没有宿主应用时快速调试接口

## `shmtu-ocr`

位置：`ocr/shmtu-ocr`

职责：

- 本地 ONNX 推理
- 验证码图像预处理
- 输出表达式与最终识别结果

它本身是一个纯库，不负责服务监听。

## `shmtu-ocr-cli`

位置：`ocr/shmtu-ocr-cli`

职责：

- 单机验证 OCR 模型和输入图片效果

## `shmtu-ocr-gui`

位置：`ocr/shmtu-ocr-gui`

职责：

- 提供 GUI 方式调试 OCR 推理

## `shmtu-ocr-server`

位置：`ocr/shmtu-ocr-server`

职责：

- 把 OCR 能力包装成 HTTP / TCP 服务
- 供远程 OCR 模式复用

## `Documents`

位置：`Documents`

职责：

- 承载 VitePress 开发者文档
- 作为 API 与架构设计的文字入口

## crate 之间的依赖关系

- `shmtu-cas` 可以独立使用
- `shmtu-ocr` 可以独立使用
- `shmtu-ocr-server` 依赖 `shmtu-ocr`
- Tauri 或其他宿主通常会同时依赖 `shmtu-cas` 与 `shmtu-ocr`
