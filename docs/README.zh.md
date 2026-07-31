[English](../README.md) | 简体中文

# Tablec — 表格编译器

一个面向游戏开发的**表格编译器** —— 把 Excel/CSV/JSON 表格数据编译成程序可直接读取的结构化格式（JSON、MessagePack）。

## 主要特性

- **表格 Schema**：类数据库的字段类型与约束定义
- **极致性能**：基于 Rust 实现，性能优异
- **丰富的类型系统**：支持基本类型、数组、映射以及结构体（自定义类型）
- **多种导出格式**：JSON、MessagePack
- **CLI 工具**：提供 `build`、`check`、`example` 三个命令
- **Python 绑定**：通过 PyO3 提供原生 Python API

## 安装

需要 Rust 1.80+：

```bash
cargo install --path .
tablec --version
```

## 快速开始

### 编译

```bash
tablec build -i input.xlsx -o output.json

# 编译 ./tablec.toml，或当前目录下所有 *.xlsx
tablec build
# 同上，但针对 ./data 目录
tablec build ./data
```

### 检查错误

```bash
tablec check path/to/files
```

### 生成示例 Excel

```bash
tablec example -o example.xlsx -r 10
```

如果输入目录中包含 `tablec.toml`（或 `.tablec.toml`），它会控制 include 模式、输出文件名、格式等配置项。可通过 `--config path/to/other.toml` 覆盖自动发现行为。

## 数据格式

Excel 工作表结构（前 5 行预留给 Schema）：

| 行号 | 内容 |
|------|------|
| 1 | 字段名（Field names） |
| 2 | 字段类型（Field types） |
| 3 | 字段注释（Field comments） |
| 4 | 约束条件（Constraints） |
| 5 | 保留行（Reserved） |

## 类型系统

### 基本类型

- **整数**：`int8`、`int16`、`int32`、`int64`
- **无符号整数**：`uint8`、`uint16`、`uint32`、`uint64`
- **浮点数**：`float32`、`float64`
- **字符串**：`string` 或 `str`
- **布尔**：`bool`

### 数组类型

```rust
int[]          // 一维数组
int[][]        // 二维数组
string[]       // 字符串数组
```

### 映射类型

```rust
map<int, string>          // 键为 int，值为 string
map<string, int[]>        // 键为 string，值为 int 数组
```

### 结构体类型

```rust
{x:int, y:float}                  // Point 结构体（匿名）
struct{name:str, value:int[]}     // 命名的结构体
```

## 约束

- `@unique` — 字段值必须唯一
- `@unique(field1, field2)` — 组合唯一约束
- `@seq` — 序列值（1, 2, 3, ...）
- `@seq(2)` — 带步长的序列（1, 3, 5, ...）
- `@order` — 升序
- `@order(desc)` — 降序

## 测试

### 运行全部测试

```bash
cargo test --package tablec-core
```

### 运行规模测试

```bash
# 小规模（10 张表 × 10 行）
cargo test --package tablec-core test_small_scale

# 中规模（100 张表 × 1000 行）
cargo test --package tablec-core test_medium_scale -- --ignored

# 大规模（1000 张表 × 10000 行）
cargo test --package tablec-core test_large_scale -- --ignored
```

### 运行性能基准

```bash
cargo bench --package tablec-core
```

### 测试覆盖

| 类别 | 说明 |
|------|------|
| 类型覆盖 | 基本类型、数组、映射、结构体 |
| 约束 | `@unique`、`@seq`、`@order` |
| 规模测试 | 10/100/1000 张表，10/1000/10000 行 |
| 性能 | 解析耗时、导出速度、内存占用 |

## Python 绑定

详见 [Python 绑定说明](../binding-python/README.md)。

## 开发说明

```bash
git config core.hooksPath .githooks
```

导入已有历史数据时，如需绕过钩子进行单次提交，可执行：

```bash
git commit --no-verify
```

CI 还会跑 `cargo fmt --all --check` 作为兜底检查。

## 项目结构

```
tablec/
├── tablec-core/        # 核心库（Rust）
│   ├── src/            # 源代码
│   ├── tests/          # 集成测试
│   └── benches/        # 性能基准
├── tablec-cli/         # CLI 应用
├── binding-python/     # Python 绑定（PyO3）
└── doc/                # 文档
```

## 许可证

```
Copyright (c) 2023-2026 cupen<xcupen@gmail.com>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```