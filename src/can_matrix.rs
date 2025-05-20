use std::collections::HashMap;
use std::fs;
use std::process::Command;
use crate::arxml_bean::arxml_bean::{Frame, Signal};

#[derive(Debug, Clone)]
pub struct CanMatrix {
    pub clusters: HashMap<String, Vec<Frame>>,
    pub frames_by_id: HashMap<i32, Frame>,
    pub frames_by_name: HashMap<String, Frame>,
    pub signals: HashMap<String, Signal>,
}

impl CanMatrix {
    pub fn new() -> Self {
        Self {
            clusters: HashMap::new(),
            frames_by_id: HashMap::new(),
            frames_by_name: HashMap::new(),
            signals: HashMap::new(),
        }
    }
    
    
    pub fn get_signals_by_message(&self, frame_id: i32, data: &[u8]) -> Option<HashMap<String, f32>>{
        let frame = self.frames_by_id.get(&frame_id).unwrap();
        frame.decode(data)
    }
    
    pub fn load_from_arxml(&mut self, arxml_file: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 解析 ARXML 文件，获取帧、信号等数据
        // ...
        
        // 将解析后的数据存储到 CanMatrix 中
        // ...
        let data = fs::read_to_string(arxml_file)?;
        let v: HashMap<String,Vec<Frame>> = serde_json::from_str(&data)?;
        self.clusters = v;
        for (cluster_name, frames) in self.clusters.iter_mut() {
            for frame in frames.iter_mut() {
                if let Some(id) = frame.id {
                    self.frames_by_id.insert(id, frame.clone());
                }
                if let Some(name) = frame.name.clone() {
                    self.frames_by_name.insert(name, frame.clone());
                }
                let mut signals = frame.signals.clone().unwrap();
                if signals.is_empty()  {
                    continue;
                } else { 
                    for signal in signals.iter_mut() {
                        self.signals.insert(signal.name.clone().unwrap(), signal.clone());
                    }
                }
            }
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