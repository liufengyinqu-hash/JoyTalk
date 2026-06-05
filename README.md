# JoyTalk

[![WeChat](https://img.shields.io/badge/%E5%BE%AE%E4%BF%A1-%E5%8A%A0%E5%85%A5%E4%BA%A4%E6%B5%81%E7%BE%A4-07C160?style=for-the-badge&logo=wechat&logoColor=white)](#%E5%BE%AE%E4%BF%A1%E4%BA%A4%E6%B5%81%E7%BE%A4)

<details>
<summary>📱 扫码或添加微信进群</summary>

**微信号：`Klaus-hao0306`（备注 JoyTalk 进群）**

![微信群二维码](wechat-qrcode.png)

</details>

**免费、开源、可扩展的离线语音转文字桌面应用，支持 Joy-Con 手柄控制。**

JoyTalk 是一款跨平台桌面应用，提供简单、注重隐私的语音转录功能。按下快捷键，说话，你的文字就会出现在任何文本框中。这一切都在你自己的电脑上完成，不会将任何信息发送到云端。

本版本基于 Handy 项目深度定制，新增 Nintendo Switch Joy-Con 手柄控制支持，让你可以通过手柄按键触发各种操作。

## 为什么选择 JoyTalk？

- **免费**：无障碍工具应该掌握在每个人手中，而不是付费墙后面
- **开源**：共同构建更好的工具。为自己扩展 JoyTalk，为更大的事业做出贡献
- **隐私**：你的声音留在你的电脑上。无需将音频发送到云端即可获得转录结果
- **简单**：一个工具，一个任务。转录你说的话并放入文本框
- **Joy-Con 控制**：通过 Joy-Con 手柄按键触发转录、键盘快捷键、Shell 命令、AppleScript 等

## 工作原理

1. **按下**可配置的键盘快捷键或 Joy-Con 按键开始/停止录音
2. **说话**，在录音激活时说出你的内容
3. **释放**后 JoyTalk 使用 Whisper 模型处理你的语音
4. **获取**转录文本直接粘贴到你正在使用的应用中

整个过程完全本地化：

- 使用 Silero VAD（语音活动检测）过滤静音
- 转录支持多种模型选择：
  - **Whisper 模型**（Small/Medium/Turbo/Large），支持 GPU 加速
  - **Parakeet V3** - CPU 优化模型，性能出色，支持自动语言检测
- 支持 Windows、macOS 和 Linux

## Joy-Con 手柄功能

JoyTalk 深度集成了 Nintendo Switch Joy-Con 手柄支持：

- **按键映射**：将 Joy-Con 按键映射到内置操作、键盘组合键、文本输入、打开应用、Shell 命令、AppleScript
- **触发模式**：支持按住、点击、双击、长按、连发五种触发方式
- **体感手势**：摇动、翻转、倾斜等手势检测
- **IR 红外摄像头**：右 Joy-Con 红外摄像头支持接近检测
- **NFC 读取**：右 Joy-Con NFC 标签读取支持
- **多手柄支持**：同时连接多个 Joy-Con，独立配置映射
- **应用感知**：根据当前前台应用自动切换按键映射配置

## 快速开始

### 下载安装

**macOS（Apple Silicon）：**

从 [GitHub Releases](https://github.com/liufengyinqu-hash/JoyTalk/releases) 下载最新版 DMG 文件（`JoyTalk_1.0.4_aarch64.dmg`），打开后拖入 Applications 文件夹即可。

本地构建的 DMG 文件位于：`target/release/bundle/dmg/JoyTalk_1.0.4_aarch64.dmg`

**从源码构建：** 参见 [BUILD.md](BUILD.md)

### 开发环境搭建

```bash
# 安装依赖
bun install

# 开发模式运行
bun run tauri dev

# macOS 上如遇 cmake 错误：
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev

# 生产构建
bun run tauri build
```

**模型下载（开发必需）：**

```bash
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

### 使用步骤

1. 下载并安装应用
2. 启动 JoyTalk 并授予必要的系统权限（麦克风、辅助功能）
3. 在设置中配置你的首选快捷键
4. 开始转录！

## 技术架构

JoyTalk 基于 Tauri 2.x 构建：

- **前端**：React + TypeScript + Tailwind CSS
- **后端**：Rust 系统集成、音频处理和 ML 推理
- **核心库**：
  - `whisper-rs`：本地 Whisper 模型语音识别
  - `transcribe-rs`：CPU 优化的 Parakeet 语音识别
  - `joycon-rs`：Joy-Con HID 通信与控制
  - `cpal`：跨平台音频 I/O
  - `vad-rs`：语音活动检测
  - `rdev`：全局键盘快捷键
  - `rubato`：音频重采样

## 调试模式

JoyTalk 包含高级调试模式，按以下组合键开启：

- **macOS**：`Cmd+Shift+D`
- **Windows/Linux**：`Ctrl+Shift+D`

## CLI 参数

通过命令行控制运行中的 JoyTalk 实例：

```bash
joytalk --toggle-transcription    # 切换录音开关
joytalk --toggle-post-process     # 切换带后处理的录音
joytalk --cancel                  # 取消当前操作
joytalk --start-hidden            # 启动时不显示主窗口
joytalk --no-tray                 # 启动时不显示托盘图标
joytalk --debug                   # 启用调试模式
```

## 系统要求

### Whisper 模型

- **macOS**：M 系列 Mac、Intel Mac
- **Windows**：Intel、AMD 或 NVIDIA GPU
- **Linux**：Intel、AMD 或 NVIDIA GPU（Ubuntu 22.04/24.04）

### Parakeet V3 模型

- **纯 CPU 运行**，兼容广泛硬件
- **最低要求**：Intel Skylake（第 6 代）或同等 AMD 处理器
- **性能**：中端硬件约 5 倍实时速度

## 已知问题

- Whisper 模型在某些系统配置下可能崩溃（Windows/Linux）
- Wayland 支持有限，需要安装 `wtype` 或 `dotool`
- Linux 录制覆盖层可能干扰文本粘贴

## 贡献指南

1. 查看 [Issues](https://github.com/liufengyinqu-hash/JoyTalk/issues)
2. Fork 仓库并创建功能分支
3. 在目标平台上充分测试
4. 提交 Pull Request 并附上清晰的变更说明
5. 加入微信群交流讨论

## 相关项目

- **[原版 Handy](https://github.com/cjpais/handy)** - JoyTalk 基于此项目开发
- **[handy.computer](https://handy.computer)** - 项目官网

## 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件

## 致谢

- **Whisper** by OpenAI - 语音识别模型
- **whisper.cpp & ggml** - 跨平台 Whisper 推理加速
- **Silero** - 轻量级 VAD
- **Tauri** - Rust 应用框架
- **Handy** - 本项目的基础项目
- **所有社区贡献者**
