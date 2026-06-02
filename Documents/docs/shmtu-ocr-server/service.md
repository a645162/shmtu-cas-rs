# `shmtu-ocr-server`

`shmtu-ocr-server` 的作用是把 `shmtu-ocr` 包装成可远程访问的识别服务。

## 服务入口

主程序通过 CLI 参数启动，关键参数包括：

- `--ip`
- `--port`
- `--tcp-port`
- `--enable-tcp`
- `--model-dir`
- `--workers`
- `--gpu`
- `--queue-capacity`
- `--server-name`

## HTTP 路由

当前暴露的 HTTP 接口有：

- `GET /api/health`
- `POST /api/ocr`
- `POST /api/ocr/upload`
- `GET /api/status`

这已经覆盖：

- 健康检查
- base64 OCR
- 文件上传 OCR
- 运行状态查询

## TCP 支持

如果开启 `--enable-tcp`，程序还会启动 TCP server。

适合：

- 兼容旧调用方
- 局域网内轻量协议接入

## 线程池与队列

服务内部通过 `OcrPool` 管理：

- worker 数量
- 排队容量
- 模型共享或复用策略

这说明它不是“一请求一临时加载模型”的设计，而是偏常驻服务模型。

## 部署建议

- 单机桌面环境：通常直接用本地 `shmtu-ocr` 更简单
- 多客户端共享：适合部署 `shmtu-ocr-server`
- 容器化环境：可直接参考仓库中的 `Dockerfile`

## 何时选 HTTP，何时选 TCP

- 新接入优先选 HTTP
- 已有旧协议兼容需求再选 TCP

原因很简单：HTTP 更容易调试、观测和接入。

## 配图占位

### HTTP 接口调试示例

![HTTP 接口调试示例占位](/images/screenshots/server/http-endpoints-debug.png)

### OCR 服务部署示意

![OCR 服务部署示意占位](/images/screenshots/server/ocr-server-deployment.png)
