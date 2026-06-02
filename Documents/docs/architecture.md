# 整体架构

`shmtu-cas-rs` 的设计重点不是“做一个完整应用”，而是“把容易变的外层和可复用的内层拆开”。

## 分层视角

可以把工作区分成四层：

1. 协议与抓取层
2. 解析与数据模型层
3. 同步与扩展层
4. OCR 推理与服务层

## 1. 协议与抓取层

对应 `shmtu-cas::cas` 与 `shmtu-cas::captcha`。

职责：

- 创建 HTTP client
- 处理 CAS 登录页 `execution`
- 提交账号、密码、验证码
- 跟随重定向
- 获取验证码图片
- 获取消费账单页面 HTML

边界：

- 不做业务持久化
- 不直接做 UI 提示
- 不关心宿主如何保存 cookies

## 2. 解析与数据模型层

对应 `shmtu-cas::datatype` 与 `shmtu-cas::parser`。

职责：

- 定义 `BillItem`、`BillType`、`BillItemStatus`
- 解析账单列表 HTML
- 解析总页数
- 导出 CSV
- 提供金额聚合等小工具

边界：

- 不处理网络
- 不处理登录状态

## 3. 同步与扩展层

对应 `shmtu-cas::sync`。

这是整个库最关键的适配层。

核心思想：

- 库负责“如何遍历远端账单”
- 宿主负责“如何判断本地是否存在”和“如何保存新增数据”

因此暴露了：

- `BillStore` trait
- `SyncOptions`
- `incremental_sync`
- `incremental_sync_with_progress`

这样任何宿主都能接入：

- Tauri 应用
- CLI
- Web 后端
- 测试内存实现

## 4. OCR 推理与服务层

对应：

- `shmtu-ocr`
- `shmtu-ocr-server`

职责：

- 加载 ONNX 模型
- 将验证码切割为数字、运算符、等号区域
- 输出表达式与最终结果
- 通过 HTTP/TCP 暴露远程识别服务

## 数据流总览

一次典型同步的数据流如下：

1. 宿主创建 `EpayAuth`
2. 通过 `probe_login` 判断是否需要重新登录
3. 通过 `prepare_challenge` 获取 `execution + captcha_image`
4. 宿主调用某个 `CaptchaResolver`
5. 通过 `submit_login` 完成登录
6. 调用 `incremental_sync`
7. 同步层拉取 HTML 页面并交给 parser
8. parser 生成 `BillItem`
9. 同步层通过 `BillStore` 把新账单交还宿主

## 设计取舍

### 为什么同步只提供增量同步

因为“全量同步”本质上只是不同参数组合，并不需要在库里重复一套主流程。宿主完全可以通过调整：

- `max_pages`
- `since_timestamp`
- `early_stop_threshold`

来构造自己的全量或半全量策略。

### 为什么 cookies 用 JSON 提取/恢复

因为宿主可能：

- 保存到数据库
- 保存到文件
- 保存在密文存储中

字符串 JSON 是最容易跨层传递的格式。

### 为什么 OCR 没直接耦合进 `EpayAuth`

因为验证码求解手段在部署现场差异极大。解耦后，库不会强迫宿主接受某种固定部署方式。

## 配图占位

### 工作区结构总览

![工作区结构总览占位](/images/screenshots/architecture/workspace-overview.png)

### 分层架构图

![分层架构图占位](/images/screenshots/architecture/shmtu-cas-layered-design.png)
