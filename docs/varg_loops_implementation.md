# Varg 语言循环和控制流实现

## 概述

成功为 Varg 脚本语言实现了完整的循环和控制流特性，包括：
- `for` 循环（三种语法）
- `while` 循环
- `break` / `continue` / `return` 语句
- 嵌套循环支持

## 实现的功能

### 1. For 循环

**范围语法（Range）**：
```varg
for i in 0..5 {
    // i: 0, 1, 2, 3, 4
}
```

**包含范围语法（Inclusive Range）**：
```varg
for i in 1..=5 {
    // i: 1, 2, 3, 4, 5
}
```

**计数语法（Count）**：
```varg
for i in count(10) {
    // i: 0, 1, 2, ..., 9
}
```

### 2. While 循环

```varg
var counter: Int = 10
while counter > 0 {
    counter -= 1
}
```

**安全性**：实现了最大迭代限制（10,000 次）防止无限循环。

### 3. 控制流语句

**Break** - 跳出当前循环：
```varg
for i in 0..100 {
    if i >= 10 {
        break
    }
}
```

**Continue** - 跳过当前迭代：
```varg
for i in 0..10 {
    if i == 5 {
        continue
    }
    state.sum += i
}
```

**Return** - 提前退出函数：
```varg
func update(_ dt: Float) {
    if state.gameOver == 1 {
        return
    }
    // 继续游戏逻辑
}
```

### 4. 嵌套循环

完全支持任意深度的嵌套循环：
```varg
for i in 0..3 {
    for j in 0..2 {
        state.product += i * j
    }
}
```

## 技术实现细节

### 修改的文件
- `crates/engine-script-varg/src/lib.rs` (主要实现文件)

### 核心变更

1. **解析器增强** (`parse_runtime_statements`):
   - 添加 `parse_for_loop()` 识别 for 循环
   - 添加 `parse_while_loop()` 识别 while 循环
   - 处理循环体的递归解析

2. **语句解析** (`parse_runtime_statement`):
   - 添加 `break` / `continue` / `return` 关键字识别
   - 支持带返回值的 return 语句

3. **运行时环境** (`RuntimeEnvironment`):
   - 将 `should_exit` 拆分为三个独立标志：
     - `should_return`: 函数级返回
     - `should_break`: 循环级跳出
     - `should_continue`: 迭代级跳过
   - 实现精确的控制流语义

4. **执行逻辑** (`execute` 方法):
   - `ForLoop`: 支持三种范围表达式，自动清理循环变量
   - `WhileLoop`: 带迭代限制的条件循环
   - 正确处理嵌套循环中的控制流传播

### 辅助函数

```rust
fn parse_for_loop(line: &str) -> Option<(String, RangeExpression)>
fn parse_while_loop(line: &str) -> Option<ConditionExpression>
```

## 测试覆盖

添加了 **11 个新测试**，覆盖：
- ✅ 基本 for 循环（范围、包含范围、计数）
- ✅ while 循环
- ✅ break 语句
- ✅ continue 语句  
- ✅ return 语句
- ✅ 嵌套循环
- ✅ 控制流在嵌套结构中的正确传播

**测试结果**：22/22 通过 (100%)

```bash
cargo test -p engine-script-varg --lib
```

## 示例脚本

创建了三个实际应用示例：

1. **`examples/scripts/loop_demo.varg`**
   - 演示所有循环语法和控制流特性

2. **`examples/scripts/particle_system.varg`**
   - 使用循环实现粒子系统发射器

3. **`examples/scripts/wave_spawner.varg`**
   - 复杂的游戏逻辑：敌人波次生成系统
   - 展示嵌套循环、早期退出、条件跳过等

## 语义和安全性

### 控制流语义
- `break` 只影响最内层循环
- `continue` 跳到最内层循环的下一次迭代
- `return` 立即退出整个函数
- 控制流标志在退出循环后自动重置

### 安全措施
- While 循环限制最多 10,000 次迭代
- 循环变量作用域限定在循环内
- 状态变量保持持久性
- 局部变量在循环结束后清理

## 性能考虑

- 解析时间复杂度：O(n) 其中 n 是源代码行数
- 运行时开销：最小化 - 使用原生 Rust 循环
- 内存：循环变量存储在 HashMap 中，退出时自动清理

## 兼容性

- ✅ 与现有 Varg 特性完全兼容
- ✅ 不破坏任何现有测试
- ✅ 遵循项目编码约定（无 unsafe 代码）
- ✅ 通过所有格式化检查

## 未来改进方向

1. **优化建议**：
   - 考虑为小范围循环展开优化
   - 添加循环不变量提升

2. **语言特性**：
   - `for-in` 遍历数组（需要先实现数组类型）
   - 标签化 break/continue（跳出多层循环）
   - 循环表达式（返回值）

3. **工具支持**：
   - LSP 中的循环复杂度分析
   - 死循环检测警告
   - 循环性能分析器

## 总结

成功为 Varg 语言添加了生产级的循环和控制流支持，使其从一个受限的脚本语言演变为功能完整的游戏脚本语言。实现遵循了项目的高质量标准：

- 完整的测试覆盖
- 清晰的错误处理
- 安全的运行时语义
- 优秀的代码可读性

这些特性为游戏开发者提供了编写复杂游戏逻辑所需的基础工具。
