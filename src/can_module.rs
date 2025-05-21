use crate::arxml_bean::arxml_bean::{Frame, Pdu};
use crate::can_matrix;
use crate::can_matrix::CanMatrix;
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, Socket, StandardId, CanFdSocket, CanFdFrame};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};

/// CAN消息结构体
#[derive(Debug, Clone)]
pub struct CanMessage {
    /// CAN消息ID（11位标准帧）
    pub id: u32,
    /// 数据载荷（最多8字节）
    pub data: Vec<u8>,
    /// 发送周期（毫秒），-1表示单次发送
    pub period_ms: i64,
    pub is_fd:bool,
    /// 下次发送时间（内部维护）
    pub next_send_time: Option<Instant>,
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
        let socket = CanFdSocket::open(interface)?;
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
        socket: Arc<Mutex<CanFdSocket>>,
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
                            if msg.is_fd{
                                let mut frame = CanFdFrame::new(id, &msg.data)
                                .expect("Frame creation failed");
                                frame.set_brs(true);
                                frame.set_esi(false);
                                // 异步发送（使用spawn_blocking避免阻塞）
                                let s = Arc::clone(&socket);
                                tokio::task::spawn_blocking(move || {
                                    s.blocking_lock().write_frame(&frame)
                                        .expect("Frame write failed");
                                    println!("Sent:{:?} {:?}", Instant::now(),frame)
                                }).await.unwrap();
                            } else {
                                let frame = CanFrame::new(id, &msg.data)
                                .expect("Frame creation failed");
                                // 异步发送（使用spawn_blocking避免阻塞）
                                let s = Arc::clone(&socket);
                                tokio::task::spawn_blocking(move || {
                                    s.blocking_lock().write_frame(&frame)
                                        .expect("Frame write failed");
                                    println!("Sent:{:?} {:?}", Instant::now(),frame)
                                }).await.unwrap();
                            }
                            
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

                        let s = Arc::clone(&socket);
                        tokio::task::spawn_blocking(move || {
                            s.blocking_lock().write_frame(&frame)
                                .expect("Frame write failed");
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
    async fn receive_task(socket: Arc<Mutex<CanFdSocket>>, stop_signal: Arc<AtomicBool>) {
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
            })
            .await
            .unwrap();

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
        self.single_shot_tx
            .send(msg)
            .await
            .expect("Message channel closed");
    }
}

impl Drop for CanModule {
    fn drop(&mut self) {
        // 设置停止信号
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

pub(crate) async fn start(can_matrix: CanMatrix) -> Result<(), Box<dyn std::error::Error>> {
    // 初始化CAN模块
    let can = CanModule::new("vcan0")?;
    let message = can_matrix.get_message_by_signals(
        0x4D2,
        HashMap::from([("isHADS_NM_BSMtoRMS".to_string(), 1.0f32),
            ("isHADS_NM_RSStoRMS".to_string(), 1.0f32),
            ("isHADS_NM_NOSSta".to_string(), 1.0f32),
        ]),
    );
    println!("------------{:?}", message);
    // let signals = can_matrix.get_signals_by_message(0x4D2, &*message.as_ref().unwrap().data);
    // 添加周期消息（100ms周期）
    can.add_periodic_message(message.unwrap()).await;

    // 发送单次消息
    can.send_once(CanMessage {
        id: 0x201,
        data: vec![0xFF],
        is_fd:true,
        period_ms: -1,
        next_send_time: None,
    })
    .await;

    // 运行10秒后自动停止
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}
