# Batch Renamer · 设计文档

> 一个**文件批量重命名器** GUI 程序，包含 Slint GUI + 多线程 + Channel 三要素。  
> **目标**：你照着这份设计自己敲出来，敲完就掌握了 Slint + 线程 + channel 的完整链路。  
> **本文只给定义不给实现**——函数签名、类型声明、算法思路都列了，花括号里的代码留给你。

---

## 目录

1. [项目结构与依赖](#1-项目结构与依赖)
2. [Slint UI 完整声明](#2-slint-ui-完整声明)
3. [Rust 架构总览与数据流](#3-rust-架构总览与数据流)
4. [业务函数签名与算法说明](#4-业务函数签名与算法说明)
5. [多线程 + Channel 设计](#5-多线程--channel-设计)
6. [Slint ↔ Rust 类型映射表](#6-slint--rust-类型映射表)
7. [分步实现指南](#7-分步实现指南)

---

## 1. 项目结构与依赖

```
batch-renamer/
├── Cargo.toml
├── build.rs
└── src/
    ├── main.rs               ← 🧭 编排：new App → init → start Timer → run
    ├── controller/
    │   ├── mod.rs             ← 重新导出各子模块
    │   ├── app_controller.rs  ← 🧩 编排者：bind_events 分发到子模块
    │   ├── choose_folder.rs   ← 📁 选择文件夹 → 扫描 → 填充列表
    │   ├── preview.rs         ← 🔍 预览重命名 → 计算新文件名
    │   ├── execute.rs         ← ⚡ 执行重命名 → 线程 + channel 调度
    │   └── check.rs           ← ✅ 勾选状态处理
    ├── model/
    │   ├── mod.rs             ← 重新导出
    │   └── rename_event.rs    ← RenameEvent 枚举（已写好）
    ├── service/
    │   ├── mod.rs             ← 重新导出
    │   └── rename_service.rs  ← 4 个纯业务函数（scan / expand / generate / execute）
    └── ui/
        └── app.slint          ← 唯一 UI 文件 (~200 行)
```

### Cargo.toml

```toml
[package]
name = "batch-renamer"
version = "0.1.0"
edition = "2024"

[dependencies]
slint = "1.16.1"
rfd = "0.15"          # 原生文件对话框
chrono = "0.4"         # 日期时间格式化

[build-dependencies]
slint-build = "1.16.1"
```

### build.rs

```rust
fn main() {
    slint_build::compile("ui/app.slint").unwrap();
}
```

---

## 2. Slint UI 完整声明

> 这个文件里只有**类型声明 + 布局骨架 + property/callback 声明**，没有实现逻辑。

### 2.1 数据模型

```slint
// === 一条文件的记录 ===
export struct FileEntry {
    old-name: string,    // 原始文件名（含扩展名）
    new-name: string,    // 预览/重命名后的文件名
    checked: bool,       // 复选框：是否参与重命名
    index: int,          // 序号（用于 {n} 占位符）
}

// === 执行结果汇总 ===
export struct RenameResult {
    total: int,          // 选中文件总数
    success: int,        // 成功数
    failed: int,         // 失败数
}
```

### 2.2 主组件

```slint
export component App inherits Window {
    preferred-width: 800px;
    preferred-height: 600px;
    title: "Batch Renamer";

    // ── Property ──

    // 文件列表
    in-out property <[FileEntry]> files;
    // 当前选择的文件夹路径
    in-out property <string> folder-path: "";
    // 用户输入的改名规则模板
    in-out property <string> rule-template: "{n}_{prefix}";
    // 用户输入的前缀文本（从 rule-template 中提取，或单独输入）
    in property <string> prefix-text: "";

    // 执行进度
    in property <bool> is-running: false;     // 是否正在重命名中
    in-out property <int> progress-current: 0;
    in-out property <int> progress-total: 0;
    in-out property <string> status-text: "就绪";

    // 执行结果
    in property <RenameResult> renamer-result;

    // ── Callback ──

    // 用户点击"选择文件夹"
    callback choose-folder-clicked();
    // 用户点击"预览"（传入当前规则模板，Rust 侧重新计算 new-name）
    callback preview-clicked(string);
    // 用户点击"执行重命名"
    callback execute-clicked();
    // 用户勾选/取消某一行（传入 index + 新状态）
    callback item-checked-changed(int, bool);

    // ── 布局 ──

    VerticalLayout {
        padding: 16px;
        spacing: 12px;

        // ① 文件夹选择行
        HorizontalLayout {
            spacing: 8px;
            Button {
                text: "📁 选择文件夹";
                clicked => { root.choose-folder-clicked(); }
            }
            Text {
                text: root.folder-path;
                vertical-alignment: center;
                color: gray;
                overflow: elide;
            }
        }

        // ② 改名规则行
        HorizontalLayout {
            spacing: 8px;
            Text {
                text: "规则:";
                vertical-alignment: center;
            }
            rule-input := LineEdit {
                text <=> root.rule-template;
                placeholder-text: "{n}_{prefix}";
                horizontal-stretch: 1;
            }
            Text {
                text: " 可用: {n} = 序号, {prefix} = 前缀, {YYYYMMDD} = 日期";
                vertical-alignment: center;
                font-size: 12px;
                color: gray;
            }
        }

        // ③ 文件列表（主力区域）
        Rectangle {
            vertical-stretch: 1;
            border-width: 1px;
            border-color: #ccc;
            border-radius: 4px;

            // 无文件时的占位
            if root.files.length == 0 : Text {
                text: "请先选择文件夹";
                horizontal-alignment: center;
                vertical-alignment: center;
                color: gray;
            }

            if root.files.length > 0 : ScrollView {
                VerticalLayout {
                    // 表头
                    HorizontalLayout {
                        padding: 4px 8px;
                        Text { width: 32px; text: ""; }
                        Text { horizontal-stretch: 1; text: "原始文件名"; font-weight: 600; }
                        Text { width: 24px; text: ""; }
                        Text { horizontal-stretch: 1; text: "新文件名  "; font-weight: 600; }
                    }

                    // 每行
                    for file[idx] in root.files : HorizontalLayout {
                        padding: 4px 8px;
                        spacing: 4px;

                        // 复选框
                        CheckBox {
                            checked: file.checked;
                            toggled => {
                                root.item-checked-changed(file.index, checked);
                            }
                        }

                        // 旧文件名
                        Text {
                            text: file.old-name;
                            horizontal-stretch: 1;
                            overflow: elide;
                            vertical-alignment: center;
                        }

                        // 箭头
                        Text {
                            text: "→";
                            vertical-alignment: center;
                            color: gray;
                        }

                        // 新文件名（预览）
                        Text {
                            text: file.new-name;
                            horizontal-stretch: 1;
                            overflow: elide;
                            vertical-alignment: center;
                            color: green;
                        }
                    }
                }
            }
        }

        // ④ 进度行
        HorizontalLayout {
            spacing: 8px;
            // 文字进度
            Text {
                text: root.status-text;
                vertical-alignment: center;
            }
            // 进度条（用 Rectangle 模拟）
            Rectangle {
                width: 200px;
                height: 16px;
                border-radius: 8px;
                background: #e0e0e0;
                Rectangle {
                    width: root.progress-total > 0
                        ? (root.progress-current * 200px / root.progress-total)
                        : 0px;
                    height: 100%;
                    border-radius: 8px;
                    background: #4CAF50;
                }
            }
            Text {
                text: root.progress-total > 0
                    ? "\{root.progress-current} / \{root.progress-total}"
                    : "";
                vertical-alignment: center;
                font-size: 12px;
            }
            // 弹性空间
            Rectangle { horizontal-stretch: 1; }
        }

        // ⑤ 操作按钮行
        HorizontalLayout {
            spacing: 12px;
            alignment: center;

            Button {
                text: "🔍 预览";
                enabled: !root.is-running && root.files.length > 0;
                clicked => {
                    root.preview-clicked(root.rule-template);
                }
            }
            Button {
                text: "⚡ 执行重命名";
                enabled: !root.is-running && root.files.length > 0;
                clicked => { root.execute-clicked(); }
            }
        }

        // ⑥ 结果摘要行（执行完毕后显示）
        if root.renamer-result.total > 0 : Text {
            text: "完成: 成功 \{root.renamer-result.success} 个, 失败 \{root.renamer-result.failed} 个";
            horizontal-alignment: center;
            font-size: 13px;
            color: root.renamer-result.failed > 0 ? red : green;
        }
    }
}
```

### 2.3 需要 import 的标准控件

```slint
import { Button, LineEdit, CheckBox, ScrollView, StandardButton } from "std-widgets.slint";
```

> 在 `app.slint` 文件最顶部加上这行 import，上面代码用到的 `Button`、`LineEdit`、`CheckBox`、`ScrollView` 才能识别。

---

## 3. Rust 架构总览与数据流

### 3.1 数据流图

```
┌── 选择文件夹 ──────────────────────────────────┐
│  ① Button click → callback choose-folder-clicked │
│  ② Rust: rfd::FileDialog → 用户选文件夹          │
│  ③ Rust: scan_directory() → Vec<String>          │
│  ④ 构造 VecModel<FileEntry> → app.set_files()    │
└─────────────────────────────────────────────────┘
                       ↓
┌── 预览重命名 ──────────────────────────────────┐
│  ① Button click → callback preview-clicked(rule) │
│  ② Rust: 遍历 files → generate_new_name()        │
│  ③ 更新 VecModel 中每个 entry 的 new-name 字段   │
│  ④ app.set_files() 刷新 UI                       │
└─────────────────────────────────────────────────┘
                       ↓
┌── 执行重命名（多线程 + Channel） ──────────────┐
│  ① Button click → callback execute-clicked()     │
│  ② Rust: app.set_is_running(true)                │
│  ③ thread::spawn(move || {                       │
│       for each checked file {                    │
│           fs::rename(old, new)                    │
│           tx.send(Progress)                       │
│       }                                          │
│       tx.send(Finished)                           │
│   })                                             │
│  ④ Timer 每 100ms 轮询 rx.try_recv()            │
│     ├ Progress → 更新 progress-current / status  │
│     └ Finished → set_is_running(false) + 刷新列表 │
└─────────────────────────────────────────────────┘
```

### 3.2 各模块职责

| 模块 | 文件 | 职责 |
|------|------|------|
| `controller/` | `app_controller.rs` | 🧩 **编排者**：`bind_events` 内只做 4 行分发 → 调子模块函数，不写业务逻辑 |
| `controller/` | `choose_folder.rs` | 📁 绑定 `on_choose_folder_clicked`：弹 rfd 对话框 → `scan_directory` → 填充 `VecModel<FileEntry>` |
| `controller/` | `preview.rs` | 🔍 绑定 `on_preview_clicked`：读当前列表 → 调 `generate_new_name` → 写回列表 |
| `controller/` | `execute.rs` | ⚡ 绑定 `on_execute_clicked`：收集 checked → `thread::spawn` → 调 `execute_rename` |
| `controller/` | `check.rs` | ✅ 绑定 `on_item_checked_changed`：更新对应项的 checked 状态 |
| `service/` | `rename_service.rs` | 纯业务逻辑，**与 Slint 无关**：scan / generate / expand / execute |
| `model/` | `rename_event.rs` | 数据定义：RenameEvent 枚举 |
| `ui/` | `app.slint` | 界面声明：组件树 + property + callback |

**数据流向规则**：

```
User click → Slint callback
    → controller::xxx::bind_xxx  (子模块，只处理本按钮逻辑)
        → service::rename_service::xxx  (纯函数，返回数据)
    → controller::xxx::app.set_xxx  (写回 Slint UI)
                              ↑
                      model::rename_event（跨线程通信）
```

### 3.3 main() 骨架结构

```rust
slint::include_modules!();

mod controller;
mod model;
mod service;

use std::sync::{Arc, Mutex, mpsc};
use slint::{Timer, TimerMode};

fn main() -> Result<(), slint::PlatformError> {
    // 1. 创建 Slint 窗口
    let app = App::new()?;

    // 2. 创建通道
    let (tx, rx) = mpsc::channel::<model::rename_event::RenameEvent>();
    let rx = Arc::new(Mutex::new(rx));

    // 3. 绑定 callback —— 传 &App，不要 move 所有权
    controller::app_controller::bind_events(&app, tx);

    // 4. 启动 Timer（轮询 rx，更新 UI）
    let timer = {
        let app_weak = app.as_weak();
        let rx_clone = rx.clone();
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
            let rx_guard = rx_clone.lock().unwrap();
            loop {
                use std::sync::mpsc::TryRecvError;
                match rx_guard.try_recv() {
                    Ok(model::rename_event::RenameEvent::Progress { current, total, .. }) => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_progress_current(current as i32);
                            app.set_progress_total(total as i32);
                            app.set_status_text("正在处理…".into());
                        }
                    }
                    Ok(model::rename_event::RenameEvent::Finished { success, failed }) => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_is_running(false);
                            app.set_status_text(
                                if failed > 0 { "部分完成（有失败）".into() }
                                else { "全部完成 ✓".into() }
                            );
                            // TODO: 刷新文件列表（重新 scan_directory + set_files）
                        }
                    }
                    Ok(model::rename_event::RenameEvent::Error(msg)) => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_status_text(format!("错误: {}", msg).into());
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        });
        timer
    };

    // 5. 运行事件循环
    app.run()
}
```

> 关键：`bind_events(&app, tx)` 传的是**引用**，`main()` 后面还能用 `app` 来初始化数据。闭包里用 `app.as_weak()` 捕获弱引用，避免循环引用。

---

## 4. 业务函数签名与算法说明

> 这些函数全部放在 **`src/service/rename_service.rs`** 中。  
> controller 层的 `bind_events` 会调用 `service::rename_service::xxx()`。  
> 每个函数都是**纯函数**（不依赖于 Slint、不持有 `AppWindow`），方便单元测试。

```rust
// src/service/rename_service.rs 的模块头
use std::path::Path;
use std::sync::mpsc;

use crate::model::rename_event::RenameEvent;
```



### 4.1 `scan_directory()`

```rust
/// 扫描文件夹，返回文件名列表（仅文件，不含目录，排序后）
///
/// # 参数
/// - `path: &str` — 文件夹路径（来自 rfd::FileDialog）
///
/// # 返回
/// - `io::Result<Vec<String>>` — 仅文件名（不含路径前缀），按字母排序
///
/// # 你需要做的事
/// 1. Path::new(path)
/// 2. fs::read_dir 遍历
/// 3. 只保留 .is_file() 的条目
/// 4. 取 .file_name() 转字符串
/// 5. .collect 成 Vec<String>
/// 6. .sort()
fn scan_directory(path: &str) -> std::io::Result<Vec<String>>
```

**调用位置**：`on_choose_folder_clicked` 回调内部，用户选完文件夹后立即调用。

### 4.2 `generate_new_name()`

```rust
/// 根据规则模板为单个文件生成新文件名
///
/// # 参数
/// - `old_name: &str` — 原始文件名（含扩展名，如 "IMG_123.jpg"）
/// - `rule: &str`     — 用户输入的规则模板（如 "{n}_{prefix}"）
/// - `index: usize`   — 序号（从 1 开始）
/// - `prefix: &str`   — 用户输入的前缀（从 rule 中提取或单独输入）
///
/// # 返回
/// - `String` — 新文件名（保留原始扩展名）
///
/// # 算法
/// 1. 从 old_name 提取扩展名（最后一个 . 之后的部分）
/// 2. 从 old_name 提取不带扩展名的部分（stem）
/// 3. 调用 expand_placeholders(rule, index, prefix, date_str) 得到主干
/// 4. 返回 主干 + "." + 扩展名
///
/// # 示例
/// generate_new_name("IMG_100.jpg", "{prefix}_{n}", 1, "旅行")
///   → stem = "IMG_100", ext = "jpg"
///   → expand("{prefix}_{n}", 1, "旅行", "20250713") = "旅行_001"
///   → 返回 "旅行_001.jpg"
fn generate_new_name(old_name: &str, rule: &str, index: usize, prefix: &str) -> String
```

### 4.3 `expand_placeholders()`

```rust
/// 替换模板中的占位符
///
/// 支持的占位符：
/// - `{n}`        → 序号，自动补零到 3 位（1 → "001", 12 → "012"）
/// - `{prefix}`   → 前缀文本（用户输入）
/// - `{YYYYMMDD}` → 当前日期（chrono::Local::now().format("%Y%m%d")）
///
/// # 算法
/// 1. let result = rule.replace("{n}", &format!("{:03}", index))
/// 2. let result = result.replace("{prefix}", prefix)
/// 3. let result = result.replace("{YYYYMMDD}", &today)
/// 4. 返回 result
///
/// # 注意
/// 替换顺序不重要，因为三个占位符互不包含
fn expand_placeholders(rule: &str, index: usize, prefix: &str) -> String
```

### 4.4 `execute_rename()`

```rust
/// 实际执行文件重命名（在多线程中调用）
///
/// # 参数
/// - `entries: Vec<FileEntry>` — 只传 checked = true 的条目
/// - `folder: &str`            — 文件夹路径
/// - `tx: Sender<RenameEvent>` — 通过 channel 发进度给主线程
///
/// # 算法
/// 遍历 entries：
///   1. 构造 old_path = Path::new(folder).join(&entry.old_name)
///   2. 构造 new_path = Path::new(folder).join(&entry.new_name)
///   3. 如果 old_path == new_path → 跳过（文件名未变）
///   4. 如果 new_path 已存在 → 可以选择跳过或覆盖（本项目建议跳过）
///   5. fs::rename(&old_path, &new_path)
///   6. tx.send(RenameEvent::Progress { current, total, file_name })
/// 遍历结束后 tx.send(RenameEvent::Finished { success, failed })
///
/// # 错误处理
/// 每个文件独立 try 捕获错误 → 失败时只增加 failed 计数，不中断后续文件
fn execute_rename(
    entries: Vec<FileEntry>,
    folder: &str,
    tx: std::sync::mpsc::Sender<RenameEvent>,
)
```

---

## 5. 多线程 + Channel 设计

### 5.1 消息枚举

> 这个枚举已经写在 `src/model/rename_event.rs` 中，直接 `use crate::model::rename_event::RenameEvent` 即可。

```rust
/// 重命名线程 → 主线程的消息
#[derive(Debug, Clone)]
pub enum RenameEvent {
    Progress {
        current: usize,
        total: usize,
        current_file: String,
    },
    Finished {
        success: usize,
        failed: usize,
    },
    Error(String),
}
```

### 5.2 线程生命周期

```
controller::app_controller::bind_events 收到 callback
┌─────────────────────────────────────────────┐
│  app.on_execute_clicked(move || {           │
│      let selected = 从 VecModel 收集 checked │
│      app.set_is_running(true);               │
│      thread::spawn(move || {                │
│          service::rename_service::           │
│              execute_rename(selected, folder, tx);
│      });                                     │
│  });                                         │
└─────────────────────────────────────────────┘
                        │ thread::spawn
                        ▼
┌─ 后台线程 ────────────────────────────────────┐
│  for each entry:                              │
│    fs::rename(old, new)                        │
│    tx.send(RenameEvent::Progress{...})         │
│  tx.send(RenameEvent::Finished{...})          │
└──────────────────────────────────────────────┘
                        │ tx.send()
                        ▼
┌─ main() 中的 Timer (每 100ms) ───────────────┐
│  loop { rx_guard.try_recv() → 匹配 3 种消息 }  │
│    Progress → set_progress_current / status   │
│    Finished → set_is_running(false) + 刷新列表 │
│    Error    → set_status_text(msg)             │
└──────────────────────────────────────────────┘
```

### 5.3 Timer 代码实现

> Timer 放置在 **`main()`** 中（见 §3.3 完整代码），不在 controller 里。  
> 闭包中用 `app.as_weak()` 捕获弱引用，调用 `rx_guard.try_recv()` 处理三种消息。  
> 收到 `Finished` 后，需调用 `service::rename_service::scan_directory()` 重新扫描文件夹 → 重建 `VecModel<FileEntry>` → `app.set_files()` 刷新列表。

### 5.4 `app_controller.rs` — 编排者（只做分发）

> `app_controller.rs` 不做任何业务操作，它只负责把每个 Slint callback 分发给对应的子模块函数。

```rust
use std::sync::mpsc::Sender;
use crate::App;
use crate::model::rename_event::RenameEvent;

/// 绑定全部 callback —— 每个 callback 只调一行子模块函数
///
/// 参数：
/// - `app: &App` — Slint 窗口引用
/// - `tx: Sender<RenameEvent>` — 执行按钮的线程发消息用
pub fn bind_events(app: &App, tx: Sender<RenameEvent>) {
    choose_folder::bind_choose_folder(app);
    preview::bind_preview(app);
    execute::bind_execute(app, tx);
    check::bind_check(app);
}
```

**每个子模块的职责**：子模块的 `bind_xxx(app)` 函数内完成全部 Slint 绑定工作（`app.on_xxx(move || { ... })`），内部使用 `app.as_weak()` 捕获弱引用。

**为什么这样分**：

| 原来（内联） | 现在（子模块） |
|------------|-------------|
| `app_controller.rs` 约 60 行，4 个 todo! | 每个子模块约 10~20 行，各管各的 |
| 改一个按钮逻辑可能在 `main.rs` 或 `app_controller.rs` | 每个功能在自己文件中，一眼找到 |
| `todo!()` 在闭包里，不能单独测试 | 子模块的 `bind_xxx` 可单独验证绑定逻辑 |
| 新增按钮 → 在 `bind_events` 续一行 | 新增按钮 → 新建子模块 + mod.rs 注册 + 在 bind_events 加一行 |

---

### 5.5 `choose_folder.rs` — 选择文件夹

> **位置**：`src/controller/choose_folder.rs`  
> **职责**：用户点"📁 选择文件夹" → 弹系统对话框 → 扫描文件 → 填充 Slint 列表。

```rust
// 函数签名
pub fn bind_choose_folder(app: &App) {
    let app_weak = app.as_weak();
    app.on_choose_folder_clicked(move || {
        // 你的代码需要做的事：
        // 1. rfd::FileDialog::new().pick_folder() → 弹出系统原生对话框
        //    如果用户取消，直接 return。
        //
        // 2. 调 service::rename_service::scan_directory(&path_str)
        //    → 返回 Result<Vec<String>, String>
        //
        // 3. 遍历文件名列表，构建 VecModel<FileEntry>
        //    → for (i, name) in files.iter().enumerate()
        //    → model.push(FileEntry { old_name: name, new_name: name, checked: true, index: i })
        //    → app.set_files(model)
        //
        // 4. app.set_folder_path(path_str)
        //
        // # 特殊
        // - rfd 对话框必须在主线程调用（它是同步阻塞的，没问题）
        // - scan_directory 是纯 IO 操作，文件少时可以同步（100 个文件 < 5ms）
        // - 如果以后有上万文件 → 用 std::thread::spawn + invoke_from_event_loop
        todo!()
    });
}
```

### 5.6 `preview.rs` — 预览重命名

> **位置**：`src/controller/preview.rs`  
> **职责**：用户点"🔍 预览" → 根据规则模板重新计算每条的新文件名。

```rust
// 函数签名
pub fn bind_preview(app: &App) {
    let app_weak = app.as_weak();
    app.on_preview_clicked(move |rule: slint::SharedString| {
        // 你的代码需要做的事：
        // 1. app.get_files() 拿到当前 ModelRc<FileEntry>
        //    转为可变模型Vec
        //
        // 2. 遍历 i ∈ 0..model.row_count()
        //    → let mut entry = model.row_data(i).unwrap()
        //    → 如果 entry.checked == true:
        //        entry.new_name = service::rename_service::generate_new_name(
        //            &entry.old_name, &rule, (i + 1) as usize, ""
        //        )
        //     否则：
        //        entry.new_name = entry.old_name（保持原样）
        //    → model.set_row_data(i, entry)
        //
        // 3. app.set_files(model.into()) 触发 UI 刷新
        //
        // # 提示
        // - VecModel::set_row_data 需要 use slint::Model  trait
        //   （是的，Model trait 上有 row_count() 和 set_row_data() 方法）
        // - generate_new_name 的第四个参数 prefix 可以从规则中提取，
        //   目前简单起见传 ""，让用户只用 rule 里的 {n} 和 {YYYYMMDD}
        todo!()
    });
}
```

### 5.7 `execute.rs` — 执行重命名

> **位置**：`src/controller/execute.rs`  
> **职责**：用户点"⚡ 执行重命名" → 收集勾选文件 → 开线程执行。

```rust
use std::sync::mpsc::Sender;
use std::thread;
use slint::{VecModel, Model};
use crate::App;
use crate::model::rename_event::RenameEvent;
use crate::service::rename_service;

pub fn bind_execute(app: &App, tx: Sender<RenameEvent>) {
    let app_weak = app.as_weak();
    app.on_execute_clicked(move || {
        // 你的代码需要做的事：
        //
        // 1. 从 app.get_files() 读取当前文件列表
        //    let model = app_weak.upgrade().unwrap().get_files();
        //    let count = model.row_count();
        //    let entries: Vec<FileEntry> = (0..count)
        //        .filter_map(|i| model.row_data(i))
        //        .collect();
        //
        // 2. 过滤 checked == true 的条目
        //    let selected: Vec<FileEntry> = entries.into_iter()
        //        .filter(|e| e.checked)
        //        .collect();
        //
        // 3. 如果没有选中文件，set_status_text("请勾选文件") 后 return
        //
        // 4. 锁定 UI：app.set_is_running(true)
        //
        // 5. 获取文件夹路径：
        //    let folder = app.get_folder_path().to_string();
        //
        // 6. 开线程：
        //    thread::spawn(move || {
        //        service::rename_service::execute_rename(selected, &folder, tx);
        //    });
        //
        // # 要点
        // - 线程内使用 tx（clone 进来的），不碰 app
        // - 线程结束时通过 tx.send(RenameEvent::Finished) 通知 Timer
        // - Timer 在 main() 中收到 Finished 后刷新列表
        todo!()
    });
}
```

### 5.8 `check.rs` — 勾选状态处理

> **位置**：`src/controller/check.rs`  
> **职责**：用户勾选/取消某一行 → 更新对应 FileEntry 的 checked 字段。

```rust
use slint::{VecModel, Model};

pub fn bind_check(app: &App) {
    app.on_item_checked_changed(move |idx: i32, checked: bool| {
        // 你的代码需要做的事：
        //
        // 1. 读当前 files 列表
        //    let model = app_weak.upgrade().map(|app| app.get_files());
        //
        // 2. 改第 idx 行的 checked 字段
        //    let mut entry = model.row_data(idx as usize).unwrap();
        //    entry.checked = checked;
        //    model.set_row_data(idx as usize, entry);
        //
        // 3. 写回（VecModel::set_row_data 直接修改内存，
        //    Slint 自动刷新 checked 状态）
        //
        // # 原理
        // FileEntry.checked 在 .slint 中绑定到 CheckBox.checked，
        // 用户在 UI 上勾选时 CheckBox 自动更新自身状态，
        // 但 FileEntry 内部的 checked 字段不会自动同步——
        // 需要你在 Rust 侧手动更新 Model。
        //
        // ⚠️ 也可以选择不处理：如果你的 execute 按钮直接
        // 从 app.get_files() 读取实时 checked 状态，
        // FileEntry.checked 可能已经被 Slint 自动同步。
        // 测试一下：如果 execute 能读到正确的 checked 值，
        // 这个子模块可以留空甚至删除。
        todo!("测试后再决定是否需要")
    });
}
```

### 5.9校验`validate_preview`与非法字符检查

``````rust
use std::collections::HashSet;

use crate::FileEntry;

/// 校验预览是否合法
///
/// 检查：
/// 1. 规则不能为空
/// 2. 至少勾选一个文件
/// 3. 文件名不能为空
/// 4. 文件名不能重复
/// 5. 文件名不能包含非法字符
pub fn validate_preview(
    files: &[FileEntry],
    rule: &str,
) -> Result<(), String> {

    // 1. 规则不能为空
    if rule.trim().is_empty() {
        return Err("请输入重命名规则".into());
    }

    // 2. 至少勾选一个文件
    if !files.iter().any(|f| f.checked) {
        return Err("请至少选择一个文件".into());
    }

    let mut names = HashSet::new();

    for file in files {

        if !file.checked {
            continue;
        }

        // 3. 文件名不能为空
        if file.new_name.trim().is_empty() {
            return Err(format!(
                "文件 [{}] 的新名称为空",
                file.old_name
            ));
        }

        // 4. 文件名不能重复
        if !names.insert(file.new_name.clone()) {
            return Err(format!(
                "存在重复文件名：{}",
                file.new_name
            ));
        }

        // 5. 文件名非法字符
        if contains_invalid_chars(&file.new_name) {
            return Err(format!(
                "文件名包含非法字符：{}",
                file.new_name
            ));
        }
    }

    Ok(())
}
``````



**非法字符检查**

``````rust
fn contains_invalid_chars(name: &str) -> bool {

    const INVALID: [char; 9] =
        ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

    name.chars()
        .any(|c| INVALID.contains(&c))
}
``````






## 6. Slint ↔ Rust 类型映射表

### 6.1 自定义类型

| Slint 声明 | Rust 自动生成类型名 | 字段映射 |
|-----------|-------------------|---------|
| `export struct FileEntry { old-name, new-name, checked, index }` | `FileEntry` | `old_name: SharedString`, `new_name: SharedString`, `checked: bool`, `index: i32` |
| `export struct RenameResult { total, success, failed }` | `RenameResult` | `total: i32`, `success: i32`, `failed: i32` |

### 6.2 Property 映射

| Slint 声明 | Rust set_ | Rust get_ |
|-----------|----------|----------|
| `in-out property <[FileEntry]> files` | `set_files(ModelRc<FileEntry>)` | `get_files() -> ModelRc<FileEntry>` |
| `in-out property <string> folder-path` | `set_folder_path(SharedString)` | `get_folder_path() -> SharedString` |
| `in-out property <string> rule-template` | `set_rule_template(SharedString)` | `get_rule_template() -> SharedString` |
| `in property <bool> is-running` | `set_is_running(bool)` | — |
| `in-out property <int> progress-current` | `set_progress_current(i32)` | `get_progress_current() -> i32` |
| `in-out property <int> progress-total` | `set_progress_total(i32)` | `get_progress_total() -> i32` |
| `in-out property <string> status-text` | `set_status_text(SharedString)` | `get_status_text() -> SharedString` |
| `in property <RenameResult> renamer-result` | `set_renamer_result(RenameResult)` | — |

### 6.3 Callback 映射

| Slint 声明 | Rust 绑定 | 闭包参数 |
|-----------|----------|---------|
| `callback choose-folder-clicked()` | `on_choose_folder_clicked(F)` | `F: Fn() + 'static` |
| `callback preview-clicked(string)` | `on_preview_clicked(F)` | `F: Fn(SharedString) + 'static` |
| `callback execute-clicked()` | `on_execute_clicked(F)` | `F: Fn() + 'static` |
| `callback item-checked-changed(int, bool)` | `on_item_checked_changed(F)` | `F: Fn(i32, bool) + 'static` |

### 6.4 VecModel 构造（Rust 侧填充列表）

```rust
use slint::{VecModel, ModelRc, SharedString};

let model: ModelRc<FileEntry> = VecModel::default().into();
let vec_model = VecModel::from(model.clone());

// 遍历 scan_directory 的结果，构建 FileEntry
for (i, name) in file_names.iter().enumerate() {
    vec_model.push(FileEntry {
        old_name: name.clone().into(),
        new_name: name.clone().into(),   // 初始 = 旧名
        checked: true,
        index: i as i32,
    });
}
app.set_files(model.into());
```

---

## 7. 分步实现指南

### Step 1 — 建项目骨架（10 分钟）

```
1. cargo new batch-renamer
2. 编辑 Cargo.toml（加 slint / rfd / chrono 依赖）
3. 创建 build.rs
4. 创建目录：src/ui/  src/controller/  src/model/  src/service/
5. 创建 src/ui/app.slint（只写空 Window + import，不写组件）
6. src/model/rename_event.rs（写入 RenameEvent 枚举）
7. src/main.rs 只写最小编排代码（不含 Timer 也 ok）
8. cargo run → 空窗口弹出
```

**验证**：黑色窗口弹出 ✅

### Step 2 — 写完 Slint UI（30 分钟）

```
1. 把 §2 的 app.slint 完整代码抄进去
2. 注意检查：
   - text: "root.folder-path"  →  去掉引号
   - text: "root.status-text"  →  去掉引号
   - 硬编码测试数据可以保留做预览，最后去掉
3. cargo run → 看到所有控件
```

**验证**：窗口出现 5 个区域：文件夹选择 → 规则输入 → 空列表 → 进度条 → 按钮 ✅

### Step 3 — 填充 model + service + controller 子模块壳（40 分钟）

```
1. model/rename_event.rs 已写好，确认 pub enum RenameEvent 可用
2. 创建 src/service/rename_service.rs，写入 4 个函数的 pub fn 签名 + 算法：
   - scan_directory / expand_placeholders / generate_new_name / execute_rename
3. 创建 src/controller/ 下 5 个文件（全写空壳，函数体留 todo!()）：
   - app_controller.rs    → pub fn bind_events(app: &App, tx: Sender<RenameEvent>)
   - choose_folder.rs     → pub fn bind_choose_folder(app: &App)
   - preview.rs           → pub fn bind_preview(app: &App)
   - execute.rs           → pub fn bind_execute(app: &App, tx: Sender<RenameEvent>)
   - check.rs             → pub fn bind_check(app: &App)
4. controller/mod.rs 声明所有子模块
5. main.rs 写 §3.3 的完整编排代码（含 Timer）
6. cargo check → 全部通过
```

**验证**：`cargo check` 通过 ✅

### Step 4 — 逐个填充 controller 子模块（按需，每个约 10 分钟）

```
顺序建议（从简单到复杂）：

1. choose_folder.rs ← 先实现这个（能看到列表出来，成就感）
   → rfd::FileDialog → scan_directory → VecModel<FileEntry> → set_files

2. check.rs ← 然后是勾选（简单，几行）
   → 从 app.get_files() 读 → 改 checked → set_row_data → 写回

3. preview.rs ← 再预览（需要 Model trait）
   → 遍历列表 → generate_new_name → set_row_data

4. execute.rs ← 最后执行（涉及线程 + channel，最复杂）
   → 收集 checked → set_is_running(true) → thread::spawn → execute_rename

5. 回到 main.rs：在 Timer 的 Finished 分支中
   → 重新 scan_directory → 刷新列表
```

**验证**：
- 点"📁 选择文件夹"→ 文件列表出现 ✅
- 勾选/取消某行 → 状态保持 ✅
- 输入规则点"🔍 预览"→ 新文件名变绿 ✅
- 点"⚡ 执行重命名"→ 进度条走完 → 文件管理器确认改名 ✅
- 列表自动刷新为新文件名 ✅

### Step 5 — Timer 完善（20 分钟）

```
1. 确认 main.rs 中的 Timer 在收到 RenameEvent::Finished 后
   重新扫描文件夹并刷新列表
2. 错误处理：执行期间禁用按钮已由 is-running 绑定自动完成
3. 如果文件很多，Timer 周期可以调大（200ms）
```

**验证**：执行完毕后列表自动刷新 ✅

### Step 6 — 打磨细节（可选，20 分钟）

```
1. 规则输入框按回车自动触发预览（LineEdit 的 accepted callback）
2. 去掉 slint 中硬编码的测试数据（让 files 默认空列表）
3. 预览时跳过 unchecked 的文件（保持原文件名）
4. 处理极端情况：文件夹为空时提示
```

**验证**：各项细节按预期工作 ✅

---

## 8. 已知问题与优化方案

> 基于当前代码审查发现的问题。每个条目标注类型、严重程度、位置及修复方向。

---

### 8.1 Bug 报告

| # | 类型 | 严重度 | 位置 | 问题 |
|---|------|--------|------|------|
| B1 | 🐛 Bug | 🔴 **高** | `execute.rs:38` | 未校验 `folder-path` 是否为空。如果用户直接点"执行"（没选文件夹），`folder` 为空字符串 → 重命名会在当前工作目录（`CWD`）执行，可能改错文件 |
| B2 | 🐛 Bug | 🔴 **高** | `main.rs:43` | `Finished` 分支有 `// TODO: 刷新文件列表` **未实现**。改名后列表仍显示旧文件名，用户以为没成功。同时 `renamer-result` property 从未被赋值 → UI 底部的结果摘要行永远不显示 |
| B3 | 🐛 Bug | 🟡 **中** | `preview.rs:40` | 预览时 `{n}` 序号用的是 `enumerate()` 索引而不是 `entry.index`。如果文件 0 取消勾选、文件 1 勾选，预览时勾选文件获得 n=1（enumerate 重新编号），但 execute 时 `entry.index` 是固定的 → **预览和执行的序号对不上** |
| B4 | 🧹 清洁 | 🟢 **低** | `rename_service.rs:44-45` | 生产代码残留了两行 `println!("rule = {}"...)` 调试输出。终端会打印乱码 |

#### B1 修复思路

```rust
// execute.rs 第 37-38 行之间插入文件夹校验：
if folder.trim().is_empty() {
    app.set_status_text("请先选择文件夹".into());
    app.set_is_running(false);
    return;
}
```

#### B2 修复思路

```rust
// main.rs 第 37-44 行，在 set_is_running(false) 后补上：
// 1. 获取当前文件夹路径
let path = app.get_folder_path().to_string();
if !path.is_empty() {
    // 2. 重新扫描目录
    if let Ok(files) = service::rename_service::scan_directory(&path) {
        // 3. 重建 FileEntry 列表
        let entries: Vec<FileEntry> = files.iter().enumerate().map(|(i, name)| {
            FileEntry { old_name: name.into(), new_name: name.into(), checked: true, index: i as i32 }
        }).collect();
        app.set_files(ModelRc::new(VecModel::from(entries)));
    }
}
// 4. 设置结果摘要
app.set_renamer_result(RenameResult {
    total: (success + failed) as i32,
    success: success as i32,
    failed: failed as i32,
});
```

#### B3 修复思路

```rust
// preview.rs 第 38-44 行，把 enumerate() 改为使用 entry.index：
for entry in files.iter_mut() {
    if entry.checked {
        entry.new_name = rename_service::generate_new_name(
            &entry.old_name, &rule,
            entry.index as usize + 1,   // ← 改成固定 index
            ""
        ).into();
    } else {
        entry.new_name = entry.old_name.clone();
    }
}
```

#### B4 修复思路

```rust
// rename_service.rs 第 44-45 行，直接删除两行 println!()
```

---

### 8.2 优化方案

| # | 类型 | 优先级 | 位置 | 建议 |
|---|------|--------|------|------|
| O1 | 🔒 安全 | **高** | `main.rs:43` | Finished 后刷列表 + 设 renamer-result（已合并入 B2） |
| O2 | 🎯 准确 | **中** | `preview.rs:38` | 序号用 `entry.index`（已合并入 B3） |
| O3 | 🔒 安全 | **中** | `execute.rs:37` | 校验文件夹路径（已合并入 B1） |
| O4 | ⚡ 性能 | **低** | `main.rs` | Timer 周期从 100ms 改为 200ms — 重命名进度不需要那么高刷新率，减少 CPU 空转 |
| O5 | ✨ 体验 | **低** | `preview.rs:45` | `validate_preview` 只返回第一个冲突文件名。可以改为收集所有冲突再一次性报错，减少用户反复修改反复预览 |
| O6 | 🧹 代码 | **低** | `model/rename_event.rs:10-15` | `Progress.current_file` 用 `SharedString`，`Error` 用 `String` → 建议统一为 `String`（减少 `.into()` 心智负担） |

#### O5 修复思路

```rust
// 把 validate_preview 的签名从返回第一个错误，改为收集所有错误：
pub fn validate_preview(files: &[FileEntry], rule: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    // ... 每个检查 pass 用 errors.push(...) 代替 return Err(...)
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
// preview.rs 侧：
if let Err(errors) = rename_service::validate_preview(&files, &rule) {
    app.set_status_text(format!("预览问题:\n{}", errors.join("\n")).into());
    return;
}
```

#### O4 修复思路

```rust
// main.rs 第 24 行：
// timer.start(slint::TimerMode::Repeated, Duration::from_millis(100),  // 改前
timer.start(slint::TimerMode::Repeated, Duration::from_millis(200),      // 改后
```

---

### 8.3 修复优先级建议

```
第一优先（立即修，不改会出问题）:
  ├─ B1 文件夹路径校验（否则可能改 CWD 文件）
  └─ B2 Finished 刷列表 + 设 renamer-result（否则改名后界面不刷新）

第二优先（功能正确性）:
  ├─ B3 预览序号改为 entry.index（否则预览和执行编号不一致）
  └─ O4 Timer 周期 200ms（减轻 CPU 负担）

第三优先（代码整洁）:
  ├─ B4 删 println! 调试输出
  ├─ O5 validate_preview 收集多个错误
  └─ O6 RenameEvent 类型统一
```

---

## 你在本项目中练到的知识点

| 知识点 | 在哪个环节出现 | 未来在音乐播放器项目哪里重现 |
|--------|-------------|-------------------------|
| `slint::VecModel` 动态填充 | Step 3 | 歌曲列表 |
| `slint::SharedString` ↔ `String` | Step 3-4 | 播放信息传递 |
| Slint `for` 循环 + `in-out property <[...]>` | Step 2-3 | 歌曲列表展示 |
| `mpsc::channel` 子线程→主线程通信 | Step 5 | 播放完成通知 |
| `Timer` + `try_recv` 轮询 | Step 5 | 播放进度轮询 |
| `thread::spawn(move || { ... })` | Step 5 | 播放线程 |
| `Arc<Mutex<Receiver>>` 共享 rx | Step 5 | 播放进度共享 |
| `invoke_from_event_loop` | Step 5（可选，也可用 Timer 代替） | 异步扫描后刷新 UI |

---

*文档生成时间：2025-07-13*
*设计版本：v1.0*
