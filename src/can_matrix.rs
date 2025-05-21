use std::collections::HashMap;
use std::fs;
use std::process::Command;
use crate::arxml_bean::arxml_bean::{Cluster, Frame, Signal};
use crate::can_module::CanMessage;

#[derive(Debug, Clone)]
pub struct CanMatrix {
    pub clusters: HashMap<String, Cluster>,
    pub signals: Vec<Signal>,
}

impl CanMatrix {
    pub fn new() -> Self {
        Self {
            clusters: HashMap::new(),
            signals: Vec::new(),
        }
    }
    
    pub fn get_message_by_signals(&self,cluster_name:String, frame_id: i32, signals: HashMap<String, f32>) -> Option<CanMessage> {
        let cluster = self.clusters.get(&cluster_name).unwrap();
        let frame = cluster.frame_by_id.get(&frame_id).unwrap();
        let mut data = frame.encode(signals).unwrap();
        //如果data长度小于frame的长度，则填充0
        if data.len() < frame.length.unwrap() as usize {
            let mut new_data = data.to_vec();
            new_data.resize(frame.length.unwrap() as usize, 0);
            data = new_data;
        }
        Some(CanMessage {
            id: frame_id as u32,
            data,
            period_ms: frame.cycle_time.unwrap() as i64,
            is_fd: frame.is_fd?,
            next_send_time: None,
        })
    }
    
    pub fn get_signals_by_message(&self,cluster_name:String, frame_id: i32, data: &[u8]) -> Option<HashMap<String, f32>>{
        let cluster = self.clusters.get(&cluster_name).unwrap();
        let frame = cluster.frame_by_id.get(&frame_id).unwrap();
        frame.decode(data)
    }
    
    pub fn load_from_arxml(&mut self, arxml_file: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 解析 ARXML 文件，获取帧、信号等数据
        // ...
        
        // 将解析后的数据存储到 CanMatrix 中
        // ...
        let data = fs::read_to_string(arxml_file)?;
        let v: HashMap<String,Vec<Frame>> = serde_json::from_str(&data)?;

        self.clusters = HashMap::new();
        for x in v {
            self.clusters.insert(x.0.clone(), Cluster::new(x.0.clone(), x.1));
        }
        Ok(())
    }
    
    
    
    
}

pub fn parse_arxml(arxml_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 执行系统命令（如 Linux 的 ls 或 Windows 的 dir）
    let output = Command::new("arxmlparse")
        .arg("-l")      // 添加参数
        .arg("-a")
        .output()       // 等待命令执行并获取输出
        .expect("执行失败");

    // 打印命令输出
    println!("状态码: {}", output.status);
    println!("标准输出:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("错误输出:\n{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}