use std::error::Error;
use prost::Message;
use crate::core::table::table::Table as OurTable;

include!(concat!(env!("OUT_DIR"), "/tablec.rs"));

pub fn export(tables: &Vec<OurTable>, output: &str) -> Result<(), Box<dyn Error>> {
    let proto_tables: Vec<Table> = tables.iter().map(|t| {
        let proto_fields: Vec<Field> = t.fields.iter().map(|f| {
            Field {
                name: f.name.clone(),
                r#type: format!("{:?}", f.t),
                desc: f.desc.clone(),
                rules: f.constraint.as_ref().map_or(vec![], |c| vec![format!("@{}({:?})", c.func, c.args)]),
                tags: f.tags.clone(),
            }
        }).collect();

        let proto_rows: Vec<Row> = t.data.iter().map(|r| {
            let mut fields_map = std::collections::HashMap::new();
            for (key, value) in r.fields.iter() {
                fields_map.insert(key.clone(), format!("{:?}", value));
            }
            Row { fields: fields_map }
        }).collect();

        Table {
            name: t.name.clone(),
            fields: proto_fields,
            data: proto_rows,
            constraints: vec![],
        }
    }).collect();

    let tables_message = Tables { tables: proto_tables };

    let mut buf = Vec::new();
    tables_message.encode(&mut buf)?;
    std::fs::write(output, buf)?;
    println!("Exported data to {}", output);
    Ok(())
}
