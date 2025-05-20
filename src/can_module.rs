use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use socketcan::{CanSocket, CanFrame, EmbeddedFrame, StandardId, Socket};
use tokio::sync::{Mutex, mpsc};
use crate::arxml_bean::arxml_bean::{Frame, Pdu};

/// CAN消息结构体
#[derive(Debug, Clone)]
pub struct CanMessage {
    /// CAN消息ID（11位标准帧）
    pub id: u32,
    /// 数据载荷（最多8字节）
    pub data: Vec<u8>,
    /// 发送周期（毫秒），-1表示单次发送
    pub period_ms: i64,
    /// 下次发送时间（内部维护）
    next_send_time: Option<Instant>,
}

/// CAN总线管理模块
pub struct CanModule {
    /// 周期发送消息表（线程安全访问）
    periodic_messages: Arc<Mutex<HashMap<u32, CanMessage>>>,
    /// 单次消息发送通道
    single_shot_tx: mpsc::Sender<CanMessage>,
    /// 停止信号（原子操作保证线程安全）
    stop_signal: Arc<AtomicBool>,
}

impl CanModule {
    /// 创建CAN模块实例
    /// # 参数
    /// - interface: 接口名，如"can0"
    pub fn new(interface: &str) -> Result<Self, socketcan::Error> {
        // 初始化CAN套接字（非阻塞模式）
        let socket = CanSocket::open(interface)?;
        socket.set_nonblocking(true)?;
        let socket = Arc::new(Mutex::new(socket));

        // 共享数据结构初始化
        let periodic_messages = Arc::new(Mutex::new(HashMap::new()));
        let (single_shot_tx, single_shot_rx) = mpsc::channel(100);
        let stop_signal = Arc::new(AtomicBool::new(false));

        // 启动发送任务
        let tx_socket = Arc::clone(&socket);
        let tx_messages = periodic_messages.clone();
        let tx_stop = stop_signal.clone();
        tokio::spawn(Self::send_task(
            tx_socket,
            tx_messages,
            single_shot_rx,
            tx_stop,
        ));

        // 启动接收任务
        let rx_socket = Arc::clone(&socket);
        let rx_stop = stop_signal.clone();
        tokio::spawn(Self::receive_task(rx_socket, rx_stop));

        Ok(Self {
            periodic_messages,
            single_shot_tx,
            stop_signal,
        })
    }

    /// 异步发送任务主循环
    async fn send_task(
        socket: Arc<Mutex<CanSocket>>,
        periodic: Arc<Mutex<HashMap<u32, CanMessage>>>,
        mut single_shot_rx: mpsc::Receiver<CanMessage>,
        stop_signal: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(1));

        loop {
            tokio::select! {
                // 定时处理周期消息
                _ = interval.tick() => {
                    let now = Instant::now();
                    let mut messages = periodic.lock().await;

                    // 遍历所有周期消息
                    for msg in messages.values_mut() {
                        if msg.period_ms > 0 && now >= msg.next_send_time.unwrap_or(now) {
                            // 构造CAN帧
                            let id = StandardId::new(msg.id as u16)
                                .expect("Invalid CAN ID");
                            let frame = CanFrame::new(id, &msg.data)
                                .expect("Frame creation failed");

                            // 异步发送（使用spawn_blocking避免阻塞）
                            // let s = Arc::clone(&socket);
                            tokio::task::spawn_blocking(move || {
                                // s.blocking_lock().write_frame(&frame)
                                //     .expect("Frame write failed");
                                println!("Sent:{:?} {:?}", Instant::now(),frame)
                            }).await.unwrap();

                            // 更新下次发送时间
                            msg.next_send_time = Some(now + Duration::from_millis(msg.period_ms as u64));
                        }
                    }
                }

                // 处理单次消息
                msg = single_shot_rx.recv() => {
                    if let Some(msg) = msg {
                        let id = StandardId::new(msg.id as u16)
                            .expect("Invalid CAN ID");
                        let frame = CanFrame::new(id, &msg.data)
                            .expect("Frame creation failed");

                        // let s = Arc::clone(&socket);
                        tokio::task::spawn_blocking(move || {
                            // s.blocking_lock().write_frame(&frame)
                            //     .expect("Frame write failed");
                            println!("Sent:{:?} {:?}", Instant::now(),frame)
                        }).await.unwrap();
                    }
                }

                // 检查停止信号
                _ = async {
                    while !stop_signal.load(Ordering::Relaxed) {
                        tokio::task::yield_now().await;
                    }
                } => break
            }
        }
    }

    /// 异步接收任务主循环
    async fn receive_task(
        socket: Arc<Mutex<CanSocket>>,
        stop_signal: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(10));

        loop {
            if stop_signal.load(Ordering::Relaxed) {
                break;
            }

            // 非阻塞接收
            let socket_clone = Arc::clone(&socket);
            let frame = tokio::task::spawn_blocking(move || {
                // 现在使用克隆后的socket
                socket_clone.blocking_lock().read_frame()
            }).await.unwrap();

            match frame {
                Ok(f) => {
                    println!("Received: {:?}", f);
                    // 这里可以添加消息处理逻辑
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => eprintln!("Receive error: {}", e),
            }

            interval.tick().await;
        }
    }

    /// 添加周期消息
    /// # 参数
    /// - msg: 需要周期性发送的消息
    pub async fn add_periodic_message(&self, mut msg: CanMessage) {
        if msg.period_ms > 0 {
            msg.next_send_time = Some(Instant::now());
            let mut messages = self.periodic_messages.lock().await;
            messages.insert(msg.id, msg);
        }
    }

    /// 发送单次消息
    /// # 参数
    /// - msg: 需要单次发送的消息
    pub async fn send_once(&self, msg: CanMessage) {
        self.single_shot_tx.send(msg).await
            .expect("Message channel closed");
    }
}

impl Drop for CanModule {
    fn drop(&mut self) {
        // 设置停止信号
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

impl Frame {
    pub fn decode(&self, data: &[u8]) -> Option<HashMap<String, f32>>{
        let mut values: HashMap<String, f32> = HashMap::new();
        if let Some(is_pdu_container) =  self.is_pdu_container{
            if is_pdu_container {
                let mut index = 0;
                while index + 4 <= data.len() {
                    // 读取 3 字节 PDU ID（需处理不足 3 字节的情况）
                    let id_bytes: [u8; 3] = data[index..index+3]
                        .try_into()
                        .map_err(|_| "Invalid ID bytes").unwrap();
                    let pdu_id = u32::from_be_bytes([0u8,id_bytes[0],id_bytes[1],id_bytes[2]]);

                    // 读取 1 字节大小
                    let pdu_size = data[index + 3];

                    // 更新索引
                    index += 4;

                    // 检查数据是否完整
                    if index + pdu_size as usize > data.len() {
                        return Some(HashMap::new());
                    }

                    // 获取 PDU 数据
                    let pdu_data = &data[index..index + pdu_size as usize];
                    let pdu_data_str = bytearray_to_binary_string(pdu_data);

                    // 解析并合并结果
                    let massgae = self.unpack_pdu(&pdu_id, &pdu_data_str).unwrap();
                    values.extend(massgae);

                    // 移动索引
                    index += pdu_size as usize;
                }
            } else {
                if let Some(signals) = self.signals.as_ref() {
                    let binary_data = bytearray_to_binary_string(data);
                    let mut decoded: HashMap<String, f32> = HashMap::new();
                    for signal in signals {
                        // 检查边界条件
                        // 修改unpack_pdu方法中的索引逻辑
                        let start_bit = signal.start_bit.unwrap() as usize;
                        let size = signal.size.unwrap() as usize;

                        // 检查边界条件（使用转换后的usize类型）
                        if start_bit + size > binary_data.len() {
                            return None;
                        }

                        // 使用转换后的usize类型进行索引
                        let bits = &binary_data[start_bit..start_bit + size];

                        // 验证二进制格式
                        if bits.chars().any(|c| c != '0' && c != '1') {
                            return None;
                        }

                        // 转换为整数
                        let value = i32::from_str_radix(bits, 2).map_err(|e| {
                            PduError::InvalidBinaryString(e.to_string())
                        }).unwrap();

                        decoded.insert(signal.name.clone().unwrap(), value as f32);

                    }
                    values.extend(decoded);
                }
            }
        }
        Some(values)
    }

    pub fn unpack_pdu(&self, pdu_id: &u32, data: &String) -> Option<HashMap<String, f32>> {
        let mut decoded: HashMap<String, f32> = HashMap::new();
        if let Some(ref pdus) = self.pdus{
            let pdu = pdus.iter().find(|p| p.id == Some(*pdu_id as i32));
            if let Some(ref signals) = pdu?.signals {
                for signal in signals {
                    // 检查边界条件
                    // 修改unpack_pdu方法中的索引逻辑
                    let start_bit = signal.start_bit.unwrap() as usize;
                    let size = signal.size.unwrap() as usize;

                    // 检查边界条件（使用转换后的usize类型）
                    if start_bit + size > data.len() {
                        return None;
                    }

                    // 使用转换后的usize类型进行索引
                    let bits = &data[start_bit..start_bit + size];

                    // 验证二进制格式
                    if bits.chars().any(|c| c != '0' && c != '1') {
                        return None;
                    }

                    // 转换为整数
                    let value = i32::from_str_radix(bits, 2).map_err(|e| {
                        PduError::InvalidBinaryString(e.to_string())
                    }).unwrap();

                    decoded.insert(signal.name.clone().unwrap(), value as f32);
                }

            }

        }


        Some(decoded)
    }
    
    pub fn encode(&self, data: HashMap<String, f32>) -> Option<Vec<u8>>{
        if self.is_pdu_container? {
            let mut new_data = Vec::new();
            let mut target_pdus: HashMap<i32, Vec<(String, f32)>> = HashMap::new();

            // 遍历输入信号组，判断每个信号应该被分配到哪个PDU中
            for (signal_name, value) in data {
                for pdu in self.pdus.as_ref().unwrap() {
                    for pdu_signal in pdu.signals.as_ref().unwrap() {
                        if pdu_signal.name.as_deref().unwrap() == signal_name.as_str() {
                            target_pdus
                                .entry(pdu.id.unwrap())
                                .or_insert_with(Vec::new)
                                .push((signal_name.clone(), value));
                        }
                    }
                }
            }

            // 打印target_pdus，调试用
            println!("{:?}", target_pdus);

            for pdu_id in target_pdus.keys() {
                if let Some(pdu) = self.pdu_by_id(*pdu_id) {
                    let pdu_data = self.encode_pdu_signals(pdu, &target_pdus[pdu_id]);
                    new_data.extend(pdu_data);
                }
            }

            Some(new_data)
        } else {
            // 初始化数据缓冲区 [报文长度]
            let mut bit_buffer = vec!['0'; self.length.unwrap() as usize * 8];
            for signal in self.signals.as_ref().unwrap(){
                if !data.contains_key(signal.name.as_ref().unwrap().as_str()){
                    continue
                }
                let start_bit = signal.start_bit.unwrap() as usize;
                let bit_size = signal.size.unwrap() as usize;

                // 2. 转换为二进制字符串（左补零）
                // 注意：实际应根据信号类型处理浮点数到整数的转换
                let value_int = data[signal.name.as_ref().unwrap().as_str()] as u64; // 浮点数四舍五入转换
                let bin_str = format!("{:0>1$b}", value_int, bit_size);

                // 3. 验证边界条件
                if start_bit + bit_size > self.length.unwrap() as usize {
                    eprintln!("Signal {} out of bounds", signal.name.as_ref().unwrap());
                    continue;
                }

                // 4. 将二进制字符串写入位缓冲区
                for (i, ch) in bin_str.chars().enumerate() {
                    if start_bit + i < self.length.unwrap() as usize {
                        bit_buffer[start_bit + i] = ch;
                    }
                }
            }
            // 将位缓冲区转换为字节数组（大端格式）
            let data_buffer: Vec<u8> = (0..self.length.unwrap() as usize)
                .map(|i| {
                    let start = i * 8;
                    let end = start + 8;
                    if end > self.length.unwrap() as usize * 8 {
                        return 0;
                    }

                    let mut byte = 0u8;
                    for (j, &bit) in bit_buffer[start..end].iter().enumerate() {
                        if bit == '1' {
                            byte |= 1 << (7 - j); // 从高位开始填充
                        }
                    }
                    byte
                })
                .collect();
            Some(data_buffer)
        }
    }
    
    pub fn encode_pdu_signals(&self, pdu: &Pdu, signals: &Vec<(String, f32)>) -> Vec<u8> {
        // 获取PDU数据长度（字节）
        let pdu_size = pdu.size.unwrap() as usize;

        // 初始化数据缓冲区 [ID(3字节)][Size(1字节)][Payload(pdu_size字节)]
        let mut data = vec![0u8; 4 + pdu_size];

        // 填充PDU ID（3字节大端格式）
        let id_bytes = pdu.id.unwrap().to_be_bytes(); // u32转为大端字节序
        data[0] = id_bytes[1]; // 取后3字节
        data[1] = id_bytes[2];
        data[2] = id_bytes[3];

        // 填充PDU长度（单字节）
        data[3] = pdu.size.unwrap() as u8;

        // 构建原始位字符串（初始化为全0）
        let bit_length = pdu_size * 8;
        let mut bit_buffer = vec!['0'; bit_length];

        // 处理每个待编码信号
        for (signal_name, value) in signals {
            // 1. 查找信号定义
            if let Some(signal) = pdu.signals.as_ref().and_then(|signals| {
                signals.iter().find(|s| s.name.as_deref() == Some(signal_name.as_str()))
            }) {
                let start_bit = signal.start_bit.unwrap() as usize;
                let bit_size = signal.size.unwrap() as usize;

                // 2. 转换为二进制字符串（左补零）
                // 注意：实际应根据信号类型处理浮点数到整数的转换
                let value_int = *value as u64; // 浮点数四舍五入转换
                let bin_str = format!("{:0>1$b}", value_int, bit_size);

                // 3. 验证边界条件
                if start_bit + bit_size > bit_length {
                    eprintln!("Signal {} out of bounds", signal_name);
                    continue;
                }

                // 4. 将二进制字符串写入位缓冲区
                for (i, ch) in bin_str.chars().enumerate() {
                    if start_bit + i < bit_length {
                        bit_buffer[start_bit + i] = ch;
                    }
                }
            }
        }

        // 将位缓冲区转换为字节数组（大端格式）
        let byte_data: Vec<u8> = (0..bit_length/8)
            .map(|i| {
                let start = i * 8;
                let end = start + 8;
                if end > bit_length {
                    return 0;
                }

                let mut byte = 0u8;
                for (j, &bit) in bit_buffer[start..end].iter().enumerate() {
                    if bit == '1' {
                        byte |= 1 << (7 - j); // 从高位开始填充
                    }
                }
                byte
            })
            .collect();

        // 将有效载荷复制到输出缓冲区
        data[4..].copy_from_slice(&byte_data);
        data
    }
}

// 新增错误类型定义
#[derive(Debug)]
enum PduError {
    BitOutOfRange,
    InvalidBinaryString(String),
}
fn bytearray_to_binary_string(bytes: &[u8]) -> String {
    //byte数组转换为二进制字符串
    bytes.iter().map(|byte| format!("{:08b}", byte)).collect::<Vec<_>>().join("")
}
pub(crate) async fn start() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化CAN模块
    let can = CanModule::new("can0")?;

    // 添加周期消息（100ms周期）
    can.add_periodic_message(CanMessage {
        id: 0x100,
        data: vec![0x01, 0x02, 0x03],
        period_ms: 100,
        next_send_time: None,
    }).await;

    // 发送单次消息
    can.send_once(CanMessage {
        id: 0x201,
        data: vec![0xFF],
        period_ms: -1,
        next_send_time: None,
    }).await;

    // 运行10秒后自动停止
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}