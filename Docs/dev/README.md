# 开发说明

## Ubuntu 24.04 下的 `libonnxruntime.so`

本项目中的 OCR 模块 `ocr/shmtu-ocr` 使用 `ort` crate 调用 ONNX Runtime 完成本地推理。
当前 `shmtu-ocr` 的 `Cargo.toml` 启用了：

```toml
ort = { version = "2.0.0-rc.12", features = ["load-dynamic"] }
```

这意味着 Rust 可执行文件在运行时不会静态打包 ONNX Runtime，而是需要在启动时动态加载一个系统可访问的 ONNX Runtime 动态库。
在 Ubuntu 24.04 上，对应文件通常就是：

```bash
libonnxruntime.so
```

## 这个 so 是干什么的

`libonnxruntime.so` 是 ONNX Runtime 的 Linux 动态库实现。
本项目的本地 OCR 推理链路依赖它完成以下工作：

1. 加载三个 ONNX 模型：
   `resnet18_equal_symbol_latest.onnx`
   `resnet18_operator_latest.onnx`
   `resnet34_digit_latest.onnx`
2. 为模型创建推理 `Session`
3. 接收 Rust 侧构造的输入张量
4. 执行 ONNX 推理
5. 返回分类结果给 `shmtu-ocr`，再由 Rust 代码拼装出最终算式和答案

如果这个 so 无法被找到，`shmtu-ocr-cli` 即使已经成功编译，也无法在运行时真正执行 OCR 推理。

## 为什么要单独复制到固定位置

最初可用的 `libonnxruntime.so` 位于 .NET 产物目录，例如：

```bash
/home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/shmtu-dotnet-lib/build/Debug/net10.0/runtimes/linux-x64/native/libonnxruntime.so
```

这个路径的问题是：

1. 它属于另一个项目的构建输出目录
2. `.NET build/Debug` 路径不稳定，清理、重建或切换配置后可能变化
3. Rust OCR 不应该长期依赖另一个项目的临时构建产物路径

因此，当前开发环境已将该文件复制到一个更稳定的用户级路径：

```bash
/home/konghaomin/.local/lib/onnxruntime/libonnxruntime.so
```

这样做的目的是：

1. 让 OCR 运行时依赖有固定位置
2. 降低路径变化导致的运行失败
3. 便于在 `~/.bashrc` 中统一配置
4. 便于后续 Tauri、CLI、本地调试脚本复用同一份 ONNX Runtime

## Ubuntu 24.04 当前约定

当前 Ubuntu 24.04 开发环境约定如下：

### 1. ONNX Runtime so 固定路径

```bash
/home/konghaomin/.local/lib/onnxruntime/libonnxruntime.so
```

### 2. Shell 环境变量

在 `~/.bashrc` 中配置：

```bash
export ORT_DYLIB_PATH="/home/konghaomin/.local/lib/onnxruntime/libonnxruntime.so"
```

重新加载 shell：

```bash
source ~/.bashrc
```

### 3. 模型目录

当前 ONNX 模型目录约定为：

```bash
/home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/Models
```

其中至少应包含：

```bash
resnet18_equal_symbol_latest.onnx
resnet18_operator_latest.onnx
resnet34_digit_latest.onnx
```

## 为什么使用 `ORT_DYLIB_PATH`

因为 `ort` 使用了 `load-dynamic` 特性，它会在运行时查找 ONNX Runtime 动态库。
在 Linux 上，如果不显式指定，默认会尝试加载：

```bash
libonnxruntime.so
```

但这要求：

1. 该文件位于系统动态库搜索路径中
2. 或者位于程序能够直接找到的位置

在开发机上，这种默认搜索通常不稳定。
因此这里不依赖系统级安装，而是显式指定：

```bash
export ORT_DYLIB_PATH="/home/konghaomin/.local/lib/onnxruntime/libonnxruntime.so"
```

这样可确保 `shmtu-ocr-cli`、后续 Tauri 调用链以及其它调试命令启动时都能定位到同一份 ONNX Runtime。

## 如何复制这个 so

如果需要重新部署，可执行：

```bash
mkdir -p ~/.local/lib/onnxruntime
cp /home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/shmtu-dotnet-lib/build/Debug/net10.0/runtimes/linux-x64/native/libonnxruntime.so ~/.local/lib/onnxruntime/libonnxruntime.so
```

然后确认 `~/.bashrc` 中已存在：

```bash
export ORT_DYLIB_PATH="$HOME/.local/lib/onnxruntime/libonnxruntime.so"
```

## 在本项目中的实际用途

当前 OCR 相关代码位于：

```text
src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr
src-tauri/vendor/shmtu-cas-rs/ocr/shmtu-ocr-cli
```

主要用法如下：

### 本地图片 OCR

```bash
cargo run --manifest-path shmtu-ocr-cli/Cargo.toml --target-dir /tmp/shmtu-ocr-target -- image <图片路径> --model-dir /home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/Models
```

### 拉取 CAS 验证码并本地推理

```bash
cargo run --manifest-path shmtu-ocr-cli/Cargo.toml --target-dir /tmp/shmtu-ocr-target -- fetch --model-dir /home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/Models --rounds 5
```

### 与远端 OCR 服务对比

```bash
cargo run --manifest-path shmtu-ocr-cli/Cargo.toml --target-dir /tmp/shmtu-ocr-target -- compare --model-dir /home/konghaomin/Prj/SHMTU/Terminal/shmtu-terminal-desktop/Models --ocr-host test.329509.xyz --ocr-port 21601 --rounds 5
```

这些命令都依赖 `ORT_DYLIB_PATH` 对应的 `libonnxruntime.so` 正常可用。

## 已验证的 Ubuntu 24.04 场景

当前环境已经完成以下验证：

1. `cargo check --manifest-path shmtu-ocr/Cargo.toml`
2. `cargo check --manifest-path shmtu-ocr-cli/Cargo.toml`
3. `cargo test --manifest-path shmtu-ocr-cli/Cargo.toml --target-dir /tmp/shmtu-ocr-target`
4. 使用样本图运行 `image` 子命令，成功输出本地 ONNX 推理结果
5. 使用 `compare` 子命令成功完成本地 ONNX 与远端 OCR 服务对比

说明在 Ubuntu 24.04 下，只要：

1. `libonnxruntime.so` 存在于固定路径
2. `ORT_DYLIB_PATH` 已导出
3. 三个 ONNX 模型文件完整

则当前 Rust OCR CLI 可以正常工作。

## 常见问题

### 1. 能编译，但一运行就报找不到 ONNX Runtime

通常是 `ORT_DYLIB_PATH` 没有设置，或者指向的文件不存在。
先检查：

```bash
echo "$ORT_DYLIB_PATH"
ls -l "$ORT_DYLIB_PATH"
```

### 2. `ORT_DYLIB_PATH` 已设置，但 OCR 仍然失败

检查该 so 是否为当前系统架构可用版本。
Ubuntu 24.04 x86_64 应使用 Linux x64 的 `libonnxruntime.so`，不要误用：

1. Windows 的 `onnxruntime.dll`
2. macOS 的 `libonnxruntime.dylib`
3. ARM64 版本的 Linux so

### 3. 模型存在，但 `compare` 或 `image` 失败

优先检查：

1. `ORT_DYLIB_PATH` 是否有效
2. `--model-dir` 是否指向包含 3 个模型文件的目录
3. 当前 shell 是否执行过 `source ~/.bashrc`

### 4. 以后更换 ONNX Runtime 版本怎么办

建议直接替换固定路径下的文件：

```bash
~/.local/lib/onnxruntime/libonnxruntime.so
```

这样不需要修改 CLI 命令，也不需要再改 `~/.bashrc`。

## 建议

在 Ubuntu 24.04 开发机上，推荐长期保持以下策略：

1. `libonnxruntime.so` 固定放在 `~/.local/lib/onnxruntime/`
2. `~/.bashrc` 中固定导出 `ORT_DYLIB_PATH`
3. OCR 模型目录独立于构建输出目录
4. Rust OCR 与 .NET 构建产物解耦，不直接依赖 `build/Debug/...` 路径

这样后续无论是命令行调试、Tauri 集成，还是自动化测试，都会稳定很多。
