

mod arxml_bean;
mod can_module;
mod can_matrix;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    
    let mut can_matrix = can_matrix::CanMatrix::new();
    can_matrix.load_from_arxml("./resource/output.json")?;
    //读取json文件
    // let data = fs::read_to_string("./resource/output.json")?;
    // // println!("Parsed JSON:\n{:#?}", data);
    // let v: HashMap<String,Vec<Frame>> = serde_json::from_str(&data)?;
    // println!("{:?}", can_matrix);
    can_module::start(can_matrix).await.expect("发送消息报错");
    Ok(())
}


