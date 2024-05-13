
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use super::fields::Type;


pub fn parse_array<T>(text: &str) -> Result<Vec<T>, Box<dyn Error>>
where 
    T: FromStr + std::fmt::Debug,
    <T as FromStr>::Err: std::fmt::Display,
{
    let trimmed = text.trim_matches(|c| c == '[' || c == ']');
    let items: Vec<&str> = trimmed.split(", ").collect();
    let mut result = vec![];
    for item in items {
        let parsed_value = match item.trim().parse::<T>() {
            Ok(value) => value,
            Err(e) => return Err(format!("failed to parsing array item: '{}'. type:'{}', {}", item, e, T).into()),
        };
        result.push(parsed_value);
    }
    Ok(result)
}


pub fn parse_map<K, V>(text: &str, key_type: &Type, value_type: &Type) -> Result<HashMap<K, V>, Box<dyn Error>>
where K: FromStr,
      V: FromStr,
      <K as FromStr>::Err: std::fmt::Debug,
      <V as FromStr>::Err: std::fmt::Debug,
{

    let trimmed = text.trim_matches(|c| c == '{' || c == '}');
    let items: Vec<&str> = trimmed.split(", ").collect();
    let mut result = HashMap::new();
    for item in items {
        let kv: Vec<&str> = item.split(":").map(|s| s.trim()).collect();
        if kv.len() != 2 {
            return Err(format!("Invalid map entry: '{}'", item).into());
        }
        let key = kv[0].parse::<K>()?;
        let value = kv[1].parse::<V>()?;
        result.insert(key, value);
    }
    Ok(result)
}