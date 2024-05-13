# 设计文档
tablec 用于编译 Excel 等表格数据到其它数据格式, 如 json, msgpack 或 protobuf

# 名词解释
* **Table** 二维数据表
* **Row** 二维数据表里的一行。
* **Col** 二维数据表里的一列。
* **Field** 二维数据表里的字段，用于声明某一列数据的字段信息,包括字段名，类型和注释。
* **Schema** 类似数据库Schema，用于描述字段信息(Field)和约束(Constraint) 
* **Constraint** 表示对一个或多个 Field 的约束，比如字段值唯一，字段值有序等等。
* **Tag** 表示标签，用于标注列,方便分组。
* **Type** 表示数据类型，用于声明字段类型。
* **Value** 表示数据值。

# 表格设计
 Excel 表格就是 `Table`, 前 5 行作为保留行，用于声明 Schema, 其中前三行是字段信息，分别是（字段名，字段类型，注释），第 4 行为 Constraint


# 字段类型
## 基础类型
string, int, uint, float, 其中按精度细分为：
1. int => int8, int16, int32, int64  
2. uint32 => uint8, uint16, uint32, uint64  
3. float => float32, float64  
并且 string 可以简写为 str

## 数组类型
array\<type\>  

表示一个 type 类型的数组，也可以简写为 `type[]`,比如 int[] 表示 int 类型的数组
其中 type 可以是任意`基础类型`，`数组类型`，或`结构体类型`

### 例子
| 类型       | array<int>    | int[][]           | string[]          |
| ---------- | ------------- | ----------------- | ----------------- |
| 数值       | 1, 2, 3       | [1,2,3],[1,2,3]   | [hello, world]    |
| 导出(Json) | [1, 2, 3]     | [[1,2,3],[1,2,3]] | ["hello","world"] |
| ---------  | ------------- | ----------------- | ----------------- |
| 数值       | [1, 2, 3]     | [[1,2,3],[1,2,3]] | [hello, world]    |
| 导出(Json) | [1, 2, 3]     | [[1,2,3],[1,2,3]] | ["hello","world"] |


## Map类型
Map\<keyType, type\>  

表示键值对. 其中 keyType 只能是 int 或 string 类型, 而 type 则可以是`基础类型`, `数组类型`, `结构体类型`.

### 例子
| 类型       | map<int, string>     | map<string, int[]>    |
| ---------- | :--------------- | --------------------- |
| 数值       | 1:2, 2:3        | a:[1,2], b:[2, 3]     |
| 导出(Json) | {"1":"2","2":"3"} | {"1":[1,2],"2":[2,3]} |

## 结构体类型
{name1: type, name2:type ... }  

or  

struct{name1: type, name2:type ... }  


表示一个结构体，它包含名为 name1, name2 的多个字段，最多支持 32 个。 类型分别由 type 声明， 如果省略 type, 则默认为 string 类型

### 例子
| 类型       | {a:int, b:str}  | struct{foo:str, bar: int[]} | struct{hello:str, world: int[]}[]                               |
| ---------- | --------------- | --------------------------- | --------------------------------------------------------------- |
| 数值       | {1, 2}          | {yes, [2,3]}                | {yes,[1,2,3]},{nonono, [4,5,6]}                                 |
| 导出(Json) | {"a":1,"b":"2"} | {"foo":"yes", "bar": [2,3]} | [{"hello":"yes", "world":[1,2,3]},{"hello":"no","world":[4,5,6]} ] |


# Constraint 
是对字段类型的额外约束，文法为 @func(arg ...).  

1. @unique(field...) 表示唯一
    * @unique  当前字段唯一(默认)
    * @unique(name1, name2) 复合字段唯一
2. @seq(step) 表示序列
    * @seq    数据符合 1,2,3,4,5,6,7... (默认)
    * @seq(2) 数据符合 1,3,5,7,9,11,13...
3. @order(asc|desc) 表示趋势有序
    * @order 从小到大(默认)
    * @order(asc) 从小到大
    * @order(desc) 从大到小


# 代码工程
1. tablec 核心模块由 rust 实现，对外提供不同语言的 binding。
2. pybinding 用于实现 python binding, 其使用了 python 的管理工具 uv，可以自动管理 py 依赖。
    * pybingding 基于 pyo3 和 maturin 实现,用于链接 rust 代码

## 数据结构
1. tables

