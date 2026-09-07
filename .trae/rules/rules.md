# egui 开发规则

1. **遵循 egui Immediate Mode 架构**
   - UI 每一帧重新构建，不要把 UI 当作传统 retained-mode Widget Tree。
   - 不要依赖长期持有的 `Ui`、`Response` 或临时 UI 状态。
   - 持久化状态应放在明确的 State / struct 中，并通过 `&mut self` 管理。

2. **优先使用 egui 原生布局和 Widget**
   - 优先使用 `horizontal`、`vertical`、`columns`、`Grid`、`ScrollArea`、`TopBottomPanel`、`SidePanel`、`CentralPanel` 等。
   - 不要为了简单布局大量使用绝对坐标。
   - 不要使用 `allocate_ui_at_rect`、`Rect` 等进行复杂手工定位，除非确实需要自定义绘制。

3. **自定义绘制统一使用 `Painter`**
   - 自定义图形、背景、动画优先使用 `ui.painter()` / `Painter`。
   - 绘制前必须明确坐标空间和 `Rect`，避免混用屏幕坐标、局部坐标。
   - 不要通过大量 `Shape` 创建临时 Widget 来模拟普通 UI。

4. **避免每帧产生不必要的资源**
   - 不要在 `update()` / UI 绘制函数中反复创建字体、图片、纹理、复杂数据结构或昂贵对象。
   - 图片、纹理、字体等资源应该缓存。
   - 动画只更新必要的状态，不要每帧重新初始化整个组件。

5. **正确处理 egui ID**
   - `Id` 必须稳定且唯一。
   - 循环生成控件时使用 `id_salt` / `push_id` 等方式确保 ID 不冲突。
   - 不要使用容易变化的文本作为唯一 ID，尤其是文本可能动态变化时。

6. **状态与 UI 分离**
   - UI 负责展示和产生用户事件。
   - 业务逻辑、数据状态、动画状态不要全部堆在 `update()` 中。
   - 复杂组件应该封装成独立 struct / 方法，例如 `MyPanel::ui(&mut self, ui: &mut egui::Ui)`。

7. **修改代码前先理解现有架构**
   - 不要为了修复一个 UI 问题重写整个模块。
   - 修改前检查现有 State、Context、Panel、Painter、Texture 和事件处理方式。
   - 优先做最小修改，保持现有 API 和架构兼容。

8. **注意 egui 的借用规则**
   - 避免在持有 `&mut Ui` / `&mut Context` 时再次可变借用相关 State。
   - 遇到 Rust borrow checker 错误时，优先调整数据访问范围和作用域，不要通过 `clone`、`unsafe` 或全局变量绕过问题。

9. **动画必须基于时间而不是帧数**
   - 使用 `ctx.input(|i| i.time)` 或合适的时间机制驱动动画。
   - 不要假设 FPS 固定。
   - 动画运行时使用 `ctx.request_repaint()`，没有动画时不要无意义地持续刷新。

10. **编译优先**
    - 每次修改后必须确保代码能够通过 `cargo check` / `cargo clippy`。
    - 不要留下未使用的 import、变量、dead code 或明显的 warning。
    - 不确定 egui API 时，先检查当前项目使用的 egui 版本和对应 API，不要凭记忆使用旧版本 API。
