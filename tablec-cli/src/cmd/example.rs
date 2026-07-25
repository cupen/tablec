use clap::Args;
use rand::Rng;
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook};
use std::error::Error;

#[derive(Args, Debug)]
pub struct ExampleCommand {
    #[arg(
        short,
        long,
        default_value = "example.xlsx",
        help = "output example Excel file path"
    )]
    pub output: String,

    #[arg(
        short,
        long,
        default_value_t = 10,
        help = "number of data rows to generate"
    )]
    pub rows: usize,

    #[arg(
        short = 'f',
        long,
        default_value_t = false,
        help = "whether to overwrite existing files"
    )]
    pub force: bool,

    #[arg(
        long,
        default_value_t = false,
        help = "use random data instead of sequential (default is sequential)"
    )]
    pub rand: bool,
}

impl ExampleCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let fpath = std::path::Path::new(&self.output);
        if fpath.exists() && !self.force {
            return Err(format!(
                "File '{}' already exists. Use --force to overwrite.",
                self.output
            )
            .into());
        }

        // 创建新的工作簿
        let mut workbook = Workbook::new();

        // 添加工作表
        let worksheet = workbook.add_worksheet();

        // 创建表头格式 - 不同行使用不同颜色和边框
        let field_name_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xE6F3FF)) // 浅蓝色
            .set_border(FormatBorder::Thin);

        let field_type_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xE6FFE6)) // 浅绿色
            .set_border(FormatBorder::Thin);

        let field_comment_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xFFF0E6)) // 浅橙色
            .set_border(FormatBorder::Thin);

        let constraint_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xFFE6E6)) // 浅红色
            .set_border(FormatBorder::Thin);

        let reserved_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xF0F0F0)) // 浅灰色
            .set_border(FormatBorder::Thin);

        // 表头定义 - 符合 tablec 格式，覆盖所有类型
        let field_names = vec![
            "id", "name", "age", "score", "active", "tags", "meta", "numbers", "mapping", "nested",
        ];

        let field_types = vec![
            "int32",
            "string",
            "int16",
            "float64",
            "bool",
            "string[]",
            "{a:int,b:str}",
            "int[][]",
            "map<string,int>",
            "{x:int,y:float}[]",
        ];

        let field_comments = vec![
            "主键",
            "名称",
            "年龄",
            "分数",
            "是否激活",
            "标签",
            "元数据结构体",
            "二维整数数组",
            "字符串到整数的映射",
            "浮点结构体数组",
        ];

        let constraints = vec!["@unique", "", "", "", "", "", "", "", "", ""];

        let reserved = vec!["", "", "", "", "", "", "", "", "", ""];

        // 写入表头 - 前5行
        // 第1行：字段名
        for (col, field_name) in field_names.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, *field_name, &field_name_format)?;
        }

        // 第2行：字段类型
        for (col, field_type) in field_types.iter().enumerate() {
            worksheet.write_string_with_format(1, col as u16, *field_type, &field_type_format)?;
        }

        // 第3行：字段注释
        for (col, field_comment) in field_comments.iter().enumerate() {
            worksheet.write_string_with_format(
                2,
                col as u16,
                *field_comment,
                &field_comment_format,
            )?;
        }

        // 第4行：约束
        for (col, constraint) in constraints.iter().enumerate() {
            worksheet.write_string_with_format(3, col as u16, *constraint, &constraint_format)?;
        }

        // 第5行：保留行
        for (col, reserved_text) in reserved.iter().enumerate() {
            worksheet.write_string_with_format(4, col as u16, *reserved_text, &reserved_format)?;
        }

        // 生成数据 - 从第6行开始
        let mut rnd = rand::rng();

        for row in 0..self.rows {
            let row_idx = row as u32 + 5; // 从第6行开始
            let i = row + 1; // 1-based index

            // id (int32)
            worksheet.write_number(row_idx, 0, i as f64)?;

            // name (string)
            worksheet.write_string(row_idx, 1, &format!("User {}", i))?;

            // age (int16) - 固定: 18+i, 随机: 18..65
            let age: i32 = if self.rand {
                rnd.random_range(18..65)
            } else {
                18 + i as i32
            };
            worksheet.write_number(row_idx, 2, age as f64)?;

            // score (float64) - 固定: 60.0+i, 随机: 60.0..100.0
            let score: f64 = if self.rand {
                rnd.random_range(60.0..100.0)
            } else {
                60.0 + i as f64
            };
            worksheet.write_number(row_idx, 3, score)?;

            // active (bool)
            let active: bool = if self.rand {
                rnd.random_bool(0.5)
            } else {
                i % 2 == 1
            };
            worksheet.write_string(row_idx, 4, if active { "true" } else { "false" })?;

            // tags (string[]) - 固定: [tag1,tag2], 随机: [tag?,tag?]
            let tag1 = if self.rand { rnd.random_range(1..5) } else { 1 };
            let tag2 = if self.rand {
                rnd.random_range(1..5)
            } else {
                2.min(i as i32)
            };
            worksheet.write_string(row_idx, 5, &format!("[tag{},tag{}]", tag1, tag2))?;

            // meta ({a:int,b:str}) - 固定: {i,str_i}, 随机: {?,?}
            let meta_a: i32 = if self.rand {
                rnd.random_range(1..100)
            } else {
                i as i32
            };
            let meta_b = if self.rand {
                format!("str{}", rnd.random_range(1..10))
            } else {
                format!("str_{}", i)
            };
            worksheet.write_string(
                row_idx,
                6,
                &format!("{{{}}}", format!("{},{}", meta_a, meta_b)),
            )?;

            // numbers (int[][]) - 固定: [[i,i+1],[i+2,i+3]], 随机: random
            let nums = if self.rand {
                format!(
                    "[[{},{}],[{},{}]]",
                    rnd.random_range(1..10),
                    rnd.random_range(1..10),
                    rnd.random_range(1..10),
                    rnd.random_range(1..10)
                )
            } else {
                format!("[[{},{}],[{},{}]]", i, i + 1, i + 2, i + 3)
            };
            worksheet.write_string(row_idx, 7, &nums)?;

            // mapping (map<string,int>) - 格式: k1:1, k2:2
            let map_val1: i32 = if self.rand {
                rnd.random_range(1..50)
            } else {
                i as i32
            };
            let map_val2: i32 = if self.rand {
                rnd.random_range(1..50)
            } else {
                (i as i32) * 2
            };
            worksheet.write_string(row_idx, 8, &format!("k1:{},k2:{}", map_val1, map_val2))?;

            // nested ({x:int,y:float}[]) - 固定: [{i,float_i},..], 随机
            let nested = if self.rand {
                format!(
                    "[{{{},{}}},{{{},{}}}]",
                    rnd.random_range(1..20),
                    rnd.random_range(1.0..10.0),
                    rnd.random_range(1..20),
                    rnd.random_range(1.0..10.0)
                )
            } else {
                format!(
                    "[{{{},{:.1}}},{{{},{:.1}}}]",
                    i,
                    i as f64,
                    i + 10,
                    (i + 10) as f64
                )
            };
            worksheet.write_string(row_idx, 9, &nested)?;
        }

        // 保存 Excel 文件
        workbook.save(&self.output)?;

        let mode = if self.rand { "random" } else { "sequential" };
        println!("Created example Excel file: {}", self.output);
        println!("Generated {} rows with {} data", self.rows, mode);
        println!(
            "Column types: int32, string, int16, float64, bool, string[], {{a:int,b:str}}, int[][], map<string,int>, {{x:int,y:float}}[]"
        );
        println!(
            "Table header format: field_name | field_type | field_comment | constraint | reserved"
        );

        Ok(())
    }
}
