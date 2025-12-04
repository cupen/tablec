use clap::Args;
use std::error::Error;
use rand::Rng;
use rust_xlsxwriter::{Workbook, Format, Color, FormatBorder};

#[derive(Args, Debug)]
pub struct ExampleCommand {
    #[arg(short, long, default_value = "example.xlsx", help = "output example Excel file path")]
    pub output: String,

    #[arg(short, long, default_value_t = 10, help = "number of data rows to generate")]
    pub rows: usize,

    #[arg(short, long, default_value_t = false, help = "whether to overwrite existing files")]
    pub force: bool,
}

impl ExampleCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let fpath = std::path::Path::new(&self.output);
        if fpath.exists() && !self.force {
            return Err(format!("File '{}' already exists. Use --force to overwrite.", self.output).into());
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

        // 表头定义 - 符合 tablec 格式
        let field_names = vec![
            "id", "name", "age", "score", "active", "tags", "created_at"
        ];

        let field_types = vec![
            "int32", "string", "int32", "float64", "bool", "string[]", "string"
        ];

        let field_comments = vec![
            "用户ID", "用户名", "年龄", "分数", "是否激活", "标签列表", "创建时间"
        ];

        let constraints = vec![
            "@unique", "", "", "", "", "", ""
        ];

        let reserved = vec![
            "", "", "", "", "", "", ""
        ];

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
            worksheet.write_string_with_format(2, col as u16, *field_comment, &field_comment_format)?;
        }

        // 第4行：约束
        for (col, constraint) in constraints.iter().enumerate() {
            worksheet.write_string_with_format(3, col as u16, *constraint, &constraint_format)?;
        }

        // 第5行：保留行
        for (col, reserved_text) in reserved.iter().enumerate() {
            worksheet.write_string_with_format(4, col as u16, *reserved_text, &reserved_format)?;
        }

        // 生成随机数据 - 从第6行开始
        let mut rnd = rand::rng();

        for row in 0..self.rows {
            let row_idx = row as u32 + 5; // 从第6行开始

            // id
            worksheet.write_number(row_idx, 0, (row + 1) as f64)?;

            // name
            worksheet.write_string(row_idx, 1, &format!("User {}", row + 1))?;

            // age
            worksheet.write_number(row_idx, 2, rnd.random_range(18..65) as f64)?;

            // score
            worksheet.write_number(row_idx, 3, rnd.random_range(60.0..100.0))?;

            // active
            worksheet.write_string(row_idx, 4, if rnd.random_bool(0.5) { "true" } else { "false" })?;

            // tags - 数组格式需要用方括号
            worksheet.write_string(row_idx, 5, &format!("[tag{},tag{}]", rnd.random_range(1..4), rnd.random_range(1..4)))?;

            // created_at
            worksheet.write_string(row_idx, 6, &format!("2024-{:02}-{:02}", rnd.random_range(1..13), rnd.random_range(1..29)))?;
        }

        // 保存 Excel 文件
        workbook.save(&self.output)?;

        println!("Created example Excel file: {}", self.output);
        println!("Generated {} rows with basic data types", self.rows);
        println!("Table header format: field_name | field_type | field_comment | constraint | reserved");

        Ok(())
    }
}


