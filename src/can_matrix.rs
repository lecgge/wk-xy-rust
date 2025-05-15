use std::collections::HashMap;
use std::process::Command;
use crate::arxml_bean::arxml_bean::{Frame, Signal};

#[derive(Debug, Clone)]
pub struct CanMatrix {
    pub clusters: HashMap<String, Vec<Frame>>,
    pub frames: HashMap<i32, Frame>,
    pub signals: HashMap<i32, Signal>,
}

impl CanMatrix {
    pub fn new() -> Self {
        Self {
            clusters: HashMap::new(),
            frames: HashMap::new(),
            signals: HashMap::new(),
        }
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