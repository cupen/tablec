use std::str::FromStr;
use tablec::core::table::constraint::Constraint;
use tablec::core::table::field::Field;
use tablec::core::table::field::FieldType;
use tablec::core::table::row::Row;
use tablec::core::table::value::Value;

fn main() {
    println!("=== 约束验证功能演示 ===\n");

    // 演示 @unique 约束
    println!("1. @unique 约束验证:");
    let unique_constraint = Constraint::from_str("@unique").unwrap();
    let fields = vec![Field {
        name: "id".to_string(),
        t: FieldType::Int32,
        desc: "唯一ID".to_string(),
        constraint: Some(unique_constraint.clone()),
        tags: vec![],
    }];

    // 有效数据 - 所有ID唯一
    let valid_rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("id".to_string(), Value::Int(2))]),
        Row::from_vec(vec![("id".to_string(), Value::Int(3))]),
    ];

    match unique_constraint.validate(&fields, &valid_rows) {
        Ok(()) => println!("   ✓ 唯一性验证通过"),
        Err(e) => println!("   ✗ 验证失败: {}", e),
    }

    // 无效数据 - 有重复ID
    let invalid_rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("id".to_string(), Value::Int(1))]),
    ];

    match unique_constraint.validate(&fields, &invalid_rows) {
        Ok(()) => println!("   ✗ 应该检测到重复值"),
        Err(e) => println!("   ✓ 正确检测到重复: {}", e),
    }

    // 演示 @seq 约束
    println!("\n2. @seq 序列约束验证:");
    let seq_constraint = Constraint::from_str("@seq").unwrap();
    let seq_fields = vec![Field {
        name: "seq".to_string(),
        t: FieldType::Int32,
        desc: "序列号".to_string(),
        constraint: Some(seq_constraint.clone()),
        tags: vec![],
    }];

    // 有效序列
    let seq_rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int(2))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int(3))]),
    ];

    match seq_constraint.validate(&seq_fields, &seq_rows) {
        Ok(()) => println!("   ✓ 序列验证通过"),
        Err(e) => println!("   ✗ 验证失败: {}", e),
    }

    // 无效序列
    let broken_seq = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int(3))]),
    ];

    match seq_constraint.validate(&seq_fields, &broken_seq) {
        Ok(()) => println!("   ✗ 应该检测到序列中断"),
        Err(e) => println!("   ✓ 正确检测到序列中断: {}", e),
    }

    // 演示 @order 约束
    println!("\n3. @order 排序约束验证:");
    let order_constraint = Constraint::from_str("@order").unwrap();
    let order_fields = vec![Field {
        name: "value".to_string(),
        t: FieldType::Int32,
        desc: "排序值".to_string(),
        constraint: Some(order_constraint.clone()),
        tags: vec![],
    }];

    // 有效排序（升序）
    let ordered_rows = vec![
        Row::from_vec(vec![("value".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("value".to_string(), Value::Int(2))]),
        Row::from_vec(vec![("value".to_string(), Value::Int(3))]),
    ];

    match order_constraint.validate(&order_fields, &ordered_rows) {
        Ok(()) => println!("   ✓ 排序验证通过"),
        Err(e) => println!("   ✗ 验证失败: {}", e),
    }

    // 无效排序
    let unordered_rows = vec![
        Row::from_vec(vec![("value".to_string(), Value::Int(1))]),
        Row::from_vec(vec![("value".to_string(), Value::Int(3))]),
        Row::from_vec(vec![("value".to_string(), Value::Int(2))]),
    ];

    match order_constraint.validate(&order_fields, &unordered_rows) {
        Ok(()) => println!("   ✗ 应该检测到排序错误"),
        Err(e) => println!("   ✓ 正确检测到排序错误: {}", e),
    }

    println!("\n=== 演示完成 ===");
}