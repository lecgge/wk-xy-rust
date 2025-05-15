use std::collections::HashMap;
use std::fs;
use arxml_bean::arxml_bean::{Frame, Root};

mod arxml_bean;
mod can_module;
mod can_matrix;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    //读取json文件
    let data = fs::read_to_string("./resource/output.json")?;
    // println!("Parsed JSON:\n{:#?}", data);
    let v: HashMap<String,Vec<Frame>> = serde_json::from_str(&data)?;
    println!("{:?}", v);
    Ok(())
}


