# Personal AI Companion Runtime Platform 技术设计文档（第一版）

## 1. 项目目标

本项目并非传统意义上的聊天应用，而是一个面向个人 AI companion / 桌宠系统的应用层运行时平台（runtime orchestration platform）。

系统的核心目标不是单纯提供一个 LLM 对话界面，而是：

* 管理多个 AI 模型之间的协作与调度
* 管理本地 GPU / VRAM 资源
* 管理人格（personality）与行为（behavior）
* 管理多模态输入输出
* 提供桌宠化 UI 与长期陪伴体验
* 支持本地模型与远程 API 的统一抽象
* 为未来扩展 Agent、插件、多模型协同预留架构空间

系统需要兼顾：

* 长期运行稳定性
* 人格一致性
* 高自由度
* 本地化与隐私
* 可配置性
* 可扩展性
* Windows 桌面完整体验
* Linux（优先 Arch Linux + KDE Plasma）兼容

---

# 2. 系统总体架构

系统采用：

```text
Application Layer AI Architecture
```

而不是：

```text
Single Monolithic LLM Application
```

系统中：

* 模型不直接控制系统
* 模型不直接调用其他模型
* 模型不直接操作资源

所有能力均通过：

```text
Scheduler / Runtime Orchestrator
```

统一管理。

---

## 2.1 核心设计原则

### 2.1.1 模型仅负责“输入 -> 输出”

模型自身不拥有：

* 系统状态
* 生命周期
* 调度权
* 资源控制权

模型只负责：

```text
Input -> Inference -> Output
```

---

### 2.1.2 人格属于应用层而不是模型层

人格（Personality）不应绑定于某个具体模型。

人格必须：

* 本地定义
* 可视化编辑
* 可导入导出
* 可切换
* 可版本化

模型只负责“扮演人格”。

人格配置由：

```text
Prompt Assembler
```

动态转换为模型输入。

---

### 2.1.3 所有能力统一抽象为 Capability

系统不应硬编码：

```text
某模型 = 某功能
```

而应抽象为：

```text
Capability
```

例如：

```text
chat
vision
image_generation
image_editing
tts
memory_summary
ocr
embedding
rerank
```

Scheduler 根据：

* capability
* 当前资源
* 用户配置
* 优先级
* 平台兼容性

动态路由任务。

---

### 2.1.4 所有推理行为统一抽象为 Job

系统内部不允许：

```text
模型直接调用模型
```

所有行为均应转化为：

```text
Job
```

例如：

```text
ChatJob
VisionJob
ImageGenerationJob
ImageEditJob
SummaryJob
MemoryExtractJob
TTSJob
```

所有 Job 统一进入：

```text
Task Queue
```

由 Scheduler 统一调度。

---

# 3. 模块划分

系统建议拆分为以下核心模块。

---

# 3.1 Runtime Scheduler（核心调度器）

这是整个系统的核心。

负责：

* Job Queue
* GPU 资源调度
* VRAM 仲裁
* 模型加载/卸载
* 推理排队
* 中断与取消
* 超时管理
* Worker 生命周期

Scheduler 是系统唯一允许直接管理模型资源的模块。

---

## 3.1.1 禁止模型直接互调

错误设计：

```text
LLM -> 调 VL -> VL 调 OCR -> OCR 调总结
```

正确设计：

```text
LLM -> 提议任务
Scheduler -> 创建 Job
Worker -> 执行
```

---

## 3.1.2 Job 生命周期

每个 Job 应至少包含：

```json
{
  "id": "...",
  "type": "...",
  "priority": 0,
  "status": "...",
  "capability": "...",
  "created_at": "...",
  "timeout": 30000,
  "cancelable": true
}
```

状态：

```text
queued
waiting_resource
running
completed
failed
cancelled
timeout
```

---

# 3.2 GPU Resource Manager

负责：

* VRAM 使用监控
* 模型独占控制
* 推理互斥
* CUDA 清理
* Fragmentation 管理

---

## 3.2.1 不允许动态 VRAM 挤占

本项目默认采用：

```text
粗粒度模型独占
```

而不是：

```text
多模型同时抢占显存
```

原因：

* CUDA fragmentation
* 显存碎片
* allocator 不稳定
* 长时间运行后崩溃概率上升

因此：

不同大模型默认不同时运行。

---

## 3.2.2 模型切换流程

切换模型时：

```text
Unload Current Model
-> torch.cuda.empty_cache()
-> gc.collect()
-> Load Next Model
```

---

# 3.3 Model Worker Layer

所有模型必须封装为：

```text
Worker
```

统一接口。

---

## 3.3.1 Worker 接口

```python
class Worker:
    def load()
    def unload()
    def infer(input)
    def interrupt()
    def health_check()
```

---

## 3.3.2 Worker 分类

### Local LLM Worker

兼容：

* llama.cpp
* Ollama（可选）
* vLLM（未来）

---

### Remote API Worker

兼容：

* OpenAI API 格式
* DeepSeek API
* Claude API（未来）
* Gemini API（未来）

---

### Image Worker

兼容：

* ComfyUI API（优先）
* Stable Diffusion
* FLUX
* SDXL

---

### Vision Worker

兼容：

* Qwen2.5-VL
* LLaVA
* InternVL

---

### TTS Worker

兼容：

* GPT-SoVITS
* CosyVoice
* Fish Speech（未来）

---

# 4. 默认模型方案（第一版默认配置）

默认方案采用：

```text
云端 cognition + 本地 embodiment
```

---

## 4.1 主语言模型

默认：

```text
DeepSeek API
```

负责：

* 主对话
* 高级推理
* 长文本理解
* 情绪理解

但：

DeepSeek 不直接控制系统。

---

## 4.2 本地人格层

本地小模型负责：

* 人格一致化
* 语气重写
* 风格约束
* 输出过滤

推荐：

```text
Qwen 7B / 14B
```

通过 llama.cpp 运行。

---

## 4.3 本地图像生成

推荐默认：

```text
ComfyUI + FLUX/SDXL
```

系统直接调用 API。

用户不需要操作 ComfyUI 网页工作流。

---

## 4.4 本地视觉分析

默认：

```text
Qwen2.5-VL
```

负责：

* 屏幕理解
* OCR
* 图像内容分析

---

# 5. Personality System（人格系统）

人格必须完全独立于模型。

---

## 5.1 人格配置文件

建议：

```text
/personality/*.json
```

示例：

```json
{
  "name": "default",
  "tone": "冷淡克制",
  "initiative": 0.4,
  "humor": 0.2,
  "verbosity": "medium",
  "style_rules": [
    "避免过度情绪化",
    "不频繁使用感叹号",
    "不主动心理安慰"
  ]
}
```

---

## 5.2 Prompt Assembler

负责：

* 拼接 system prompt
* 插入人格规则
* 插入 few-shot
* 插入上下文摘要

---

## 5.3 Persona Rewriter

DeepSeek 输出后：

由本地人格层再次重写：

* 语气
* 风格
* 人格一致性

防止：

* API 风格漂移
* 官方腔
* 人格崩坏

---

# 6. Context & Memory System

上下文必须分层。

禁止把所有内容塞进同一个聊天上下文。

---

## 6.1 Conversational Context

仅保存：

* 用户聊天
* AI 回复

用于实时对话。

---

## 6.2 Episodic Memory

长期摘要记忆。

例如：

```text
用户最近在写毕业论文
最近熬夜较多
最近经常讨论数学与科研
```

---

## 6.3 Semantic Memory

结构化长期事实：

```json
{
  "favorite_games": [],
  "research_topic": "...",
  "speech_style": "..."
}
```

---

## 6.4 Cognitive Scratchpad

AI 内部推理层。

不直接进入聊天上下文。

---

# 7. 多模态系统

---

# 7.1 屏幕截图系统

支持：

* 截图间隔
* 截图目录
* 保留数量
* 自动清理
* 手动分析

均需提供 GUI 配置。

---

## 7.1.1 两级分析机制

### Level 1（轻量）

执行：

* OCR
* active window
* hash diff

不调用 VL。

---

### Level 2（重量）

仅在：

* 屏幕明显变化
* 用户允许
* AI 主动触发

时：

调用 VL 模型。

---

# 7.2 AI 主动性系统

AI 主动行为不允许简单随机触发。

必须综合：

* 用户 idle 时间
* 当前窗口
* 最近对话密度
* 最近情绪状态
* 冷却时间
* 用户设置的活跃度

生成：

```text
initiative score
```

超过阈值后才允许主动发言。

---

# 7.3 Prompt Builder（图像）

聊天 prompt 与生图 prompt 必须分离。

---

## 图像生成流程

```text
LLM
-> Scene Description
-> Prompt Builder
-> ComfyUI Workflow
-> Image Worker
```

---

# 8. UI 系统

---

# 8.1 技术栈建议

推荐：

```text
Tauri + Rust Backend
```

原因：

* 更轻量
* 更适合常驻应用
* 更适合桌宠 overlay
* 更适合系统托盘
* 更适合跨平台

不建议 Electron。

---

# 8.2 平台支持

## Windows

必须完整支持：

* 桌宠 overlay
* 透明窗口
* 系统托盘
* 截图
* 全部多模态功能

---

## Arch Linux + KDE Plasma

尽量支持：

* Wayland
* X11
* tray
* overlay

若部分桌宠功能实现困难，可降级。

但：

非必要不放弃 Linux 支持。

---

# 8.3 桌宠系统

桌宠需要支持：

* 可自定义形象
* Live2D（未来）
* 透明背景
* always-on-top
* click-through（可选）

---

## 桌宠交互

桌宠需要包含：

* 对话按钮
* 悬浮菜单
* 快速功能入口

---

# 9. 配置系统

所有重要行为必须可视化配置。

禁止强依赖手改 JSON。

---

## 必须提供 GUI 配置：

* 模型配置
* API 配置
* 人格配置
* Prompt 配置
* GPU 调度策略
* 截图配置
* AI 主动性
* Memory 策略
* 图像模型参数
* Worker 启停

---

## 配置文件

同时保留：

```text
json/yaml/toml
```

方便：

* 导入导出
* 社区分享
* Agent 自动生成

---

# 10. 可观测性（Observability）

必须从第一版开始实现。

否则后期无法调试。

---

## 至少需要：

### Job Timeline

记录：

* 创建
* 排队
* 开始
* 完成

---

### VRAM Usage

记录：

* 峰值
* 当前占用
* 模型切换

---

### Prompt Logs

记录：

* 输入 prompt
* Prompt Assembler 输出
* 最终发送内容

---

### Token Usage

统计：

* API token
* 本地 token

---

### Memory Logs

记录：

* 命中哪些记忆
* 使用哪些 summary

---

# 11. 插件系统（预留）

第一版不强制实现。

但架构必须预留：

```text
/plugins
```

未来用于：

* VSCode
* Steam
* Discord
* 日历
* 音乐播放器
* 游戏状态

---

# 12. 内部消息协议

禁止：

* 字符串乱传
* prompt 拼接式系统

必须统一协议。

---

## 推荐结构

```json
{
  "type": "...",
  "source": "...",
  "target": "...",
  "payload": {},
  "timestamp": "..."
}
```

---

# 13. 非目标（当前版本不考虑）

当前版本暂不重点考虑：

* 生视频
* 多 GPU 集群
* 分布式推理
* Kubernetes
* 云端部署
* 多用户系统

但架构需预留扩展空间。

---

# 14. 第一版核心目标

第一版应优先实现：

## 核心闭环

```text
桌宠 UI
-> DeepSeek 对话
-> 本地人格重写
-> 截图分析
-> AI 主动发言
-> 图像生成
-> GPU 调度
-> 配置系统
```

优先保证：

* 稳定性
* 长时间运行
* 人格一致性
* 可扩展性

而不是：

* 功能数量
* 模型数量
* 炫技式 Agent 行为。
