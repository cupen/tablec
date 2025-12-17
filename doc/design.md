# 设计文档
## 简介
tablec 意为 **Table** **C**ompiler, 用于编译 Excel 等表格数据到程序易读的数据格式, 如 json, msgpack 或 protobuf

## 名词
| 名词            | 含义  | 用途                         |
| -------------- | ----- | --------------------------- |
| **Table**      | 数据表 | 描述二维数据数据的基本结构。     |
| **Row**        | 行    | 数据表(Table)的横向数据集，包含多个列的数据。 |
| **Col**        | 列    | 数据表(Table)的竖向数据集，所有行的该列数据共享同一字段定义。 |
| **Schema**     | 模式  | 数据表的整体结构，包括字段信息(Field)和约束(Constraint)。 |
| **Field**      | 字段  | 用于声明某一列数据的元信息，包括字段名、类型和注释。 |
| **Constraint** | 约束  | 对一个或多个字段施加限制，如唯一、有序等等，以确保数据完整。 |
| **Tag**        | 标签 | 用于标注列，方便进行分组、筛选和分类处理。 |
| **Type**       | 类型 | 用于声明字段中数据的类型（如int、string、日期等等）。 |
| **Value**      | 值   | 表示数据表中具体的数值或内容。 |


## 设计
 Excel Sheet 即 `Table`, 前 5 行作为保留行，用于声明 Schema, 其中前 3 行是字段信息，分别是（字段名，字段类型，注释），第 4 行为 Constraint, 第 5 行暂时保留

| 第1行     | 字段名    |
| -------- | -------- |
| 第2行     | 字段类型  |
| 第3行     | 字段注释  |
| 第4行     | Constrint |
| 第5行     | 保留使用 |



## 字段类型
### 1. 基础类型
`string`, `int`, `uint`, `float`, 按精度细分为：
1. int => int8, int16, int32, int64  
2. uint => uint8, uint16, uint32, uint64  
3. float => float32, float64  
且 string 可简写为 str

### 2. 数组类型
**`type[]`** 或 **`array<type>`**  

表示 type 类型的数组，推荐简写为 `type[]`,比如 int[] 表示 int 类型的数组
其中 type 可以是任意`基础类型`，`数组类型`，或`结构体类型`

例子  

| 类型       | array\<int\>    | int[][]           | string[]          |
| ---------- | ------------- | ----------------- | ----------------- |
| 数值       | 1, 2, 3       | [1,2,3],[1,2,3]   | [hello, world]    |
| 导出(Json) | [1, 2, 3]     | [[1,2,3],[1,2,3]] | ["hello","world"] |


| 类型       | array<struct{a:int, b:str}>           | struct{a:int, b:str}[]                |
| ---------- | ------------------------------------- | ------------------------------------- |
| 数值       | {1,abc}, {2,def}                      | {1,abc}, {2,def}                      |
| 导出(Json) | [{"a":1,"b":"abc"},{"a":2,"b":"def"}] | [{"a":1,"b":"abc"},{"a":2,"b":"def"}] |


### 3. Map类型
**`Map<keyType, type>`**  

表示键值对. 其中 `keyType` 只能是 `int` 或 `string` 类型, 而 `type` 则可以是 `基础类型`, `数组类型`, `结构体类型`.

例子  
| 类型       | map<int, string>  | map<string, int[]>      |  map<string, {a:int, b:str}>                   |
| ---------- | :---------------- | ---------------------- | ---------------------------------------------- |
| 数值       | 1:2, 2:3          | k1:[1,2], k2:[2, 3]     | k1:{1,2},  k2:{2,3}                            |
| 导出(Json) | {"1":"2","2":"3"} | {"k1":[1,2],"k2":[2,3]} | <code class="json-data"> {"k1":{"a":1, "b":"2"}, "k2":{"a":2, "b":"3"}}</code> |

### 4. 结构体类型
**`{name1:type, name2:type ... }`**  

or  

**`struct{name1:type, name2:type ... }`**  


表示结构体，它包含名为 name1, name2 的多个字段，最多支持 32 个。 类型分别由 `type` 声明， 如省略 `type`, 则默认为 `string` 类型

例子  
| 类型       | {a:int, b:str}  | struct{foo:str, bar: int[]} | {hello:str, world: {a:int, b:float}}[]                             |
| ---------- | --------------- | --------------------------- | ------------------------------------------------------------------ |
| 数值       | {1, 2}          | {yes, [2,3]}                | {yes, {1, 1.0} },{no, {2, 2.0}}                                    |
| 导出(Json) | {"a":1,"b":"2"} | {"foo":"yes", "bar": [2,3]} | `[{"hello":"yes", "world":{"a":1, "b":1.0}},{"hello":"no","world":{"a":2, "b":2.0}} ]` |


## Constraint 
是对字段类型的额外约束，文法为 **`@func(arg ...)`**.  

1. @unique(field...) 表示唯一
    * @unique  当前字段唯一(默认)
    * @unique(name1, name2) 复合字段唯一
2. @seq(step)  表示序列
    * @seq     有序递增(默认) 1,2,3,4,5,6,7...
    * @seq(2)  有序递增 1,3,5,7,9,11,13...
    * @seq(-1) 有序递减  
3. @order(asc|desc) 表示趋势有序
    * @order       从小到大(默认)
    * @order(asc)  从小到大
    * @order(desc) 从大到小


## 代码工程
1. tablec 核心模块由 rust 实现，对外提供不同语言的 binding。
2. binding 用于实现其他语言 API. 
    * bingding-python 基于 pyo3 和 maturin 实现, 其使用 python 的管理工具 uv

## 数据结构
1. project
    ```rust
    #[derive(Debug, Serialize)]
    pub struct Project {
        pub name: String,
        pub version: String,
        pub hash: int64,
        pub buildAt: int64,
        pub tables: Map<name, Table>,
    }
    ```
2. table 
    ```rust
    #[derive(Debug, Serialize)]
    pub struct Table {
        pub name: String,
        pub fields: Vec<field::Field>,
        pub data: Vec<Row>,
        pub constraints: Vec<constraint::Constraint>,
    }
    ```
