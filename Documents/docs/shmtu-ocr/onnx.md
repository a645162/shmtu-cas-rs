# `shmtu-ocr`

`shmtu-ocr` 是本地 ONNX 验证码识别库，职责集中在“模型加载 + 图像预处理 + 推理 + 算式还原”。

## 暴露内容

顶层主要导出：

- `backend`
- `image`
- `const_value`
- `EqualSymbol`
- `ExprOperator`
- `OcrResult`

## 模型常量

`const_value` 中定义了：

- 等号模型文件名
- 运算符模型文件名
- 数字模型文件名
- 默认模型下载基地址

这让宿主在下载、检查、恢复模型时可以复用统一名称。

## 核心后端：`CasOnnxBackend`

常用方法：

- `check_model_exists(dir)`
- `missing_model_files(dir)`
- `load(dir)`
- `predict_file(path)`
- `predict_bytes(data)`

## 推理流程

`predict_validate_code` 的流程大致如下：

1. 二值化图片
2. 颜色反转
3. 先识别等号样式
4. 根据等号样式决定裁切关键点
5. 分别裁切 `digit1 / operator / digit2`
6. 用不同模型推理
7. 组装表达式并计算最终结果

## `ExprOperator`

提供两个重要方法：

- `as_str()`
- `calculate(digit1, digit2)`

因此 OCR 输出不仅能给最终值，还能给可读表达式，例如 `3 + 5 = 8`。

## `OcrResult`

包含：

- `result`
- `expr`
- `equal_symbol`
- `operator`
- `digit1`
- `digit2`

这比只返回一个字符串更适合调试、测试和误判分析。

## 适合的接入方式

- 桌面端本地 OCR
- 无公网依赖的离线场景
- 作为 `shmtu-ocr-server` 的底层推理引擎

## 接入注意点

- 模型必须齐全
- ONNX Runtime 必须可正常加载
- 错误通常出现在模型缺失、模型损坏或输入图片解析失败

## 配图占位

### ONNX 推理流程

![ONNX 推理流程占位](/images/screenshots/ocr/onnx-pipeline-overview.png)

### OCR 结果示例

![OCR 结果示例占位](/images/screenshots/ocr/ocr-result-sample.png)
