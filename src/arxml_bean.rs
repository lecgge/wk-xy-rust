use std::collections::HashMap;
use serde::{Serialize, Deserialize};

pub mod arxml_bean{
	use std::collections::HashMap;
	use serde::{Deserialize, Serialize};

	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct Frame {
		pub name: Option<String>,

		pub id: Option<i32>,

		pub extended: Option<bool>,

		pub length: Option<i32>,

		pub cycle_time: Option<i32>,

		pub is_fd: Option<bool>,

		pub is_multiplexed: Option<bool>,

		pub is_pdu_container: Option<bool>,

		pub is_j1939: Option<bool>,

		pub pdu_name: Option<String>,

		pub pdus: Option<Vec<Pdu>>,

		pub signals: Option<Vec<Signal>>,
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
				let buffer_len = (self.length.unwrap() * 8i32) as usize;
				let mut bit_buffer = vec!['0'; buffer_len];
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
					if start_bit + bit_size > buffer_len {
						eprintln!("Signal {} out of bounds", signal.name.as_ref().unwrap());
						continue;
					}

					// 4. 将二进制字符串写入位缓冲区
					for (i, ch) in bin_str.chars().enumerate() {
						if start_bit + i < buffer_len {
							bit_buffer[start_bit + i] = ch;
						}
					}
				}
				// 将位缓冲区转换为字节数组（大端格式）
				let data_buffer: Vec<u8> = (0..buffer_len / 8)
					.map(|i| {
						let start = i * 8;
						let end = start + 8;
						if end > buffer_len {
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

		pub fn pdu_by_id(&self, id: i32) -> Option<&Pdu> {
			for pdu in self.pdus.as_ref().unwrap() {
				if pdu.id.unwrap() == id {
					return Some(pdu);
				}
			}
			None
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
	


	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct Signal {
		pub name: Option<String>,

		pub start_bit: Option<i32>,

		pub size: Option<i32>,

		pub is_little_endian: Option<bool>,

		pub is_ascii: Option<bool>,

		pub is_float: Option<bool>,

		pub is_multiplexer: Option<bool>,

		pub max: Option<i128>,

		pub min: Option<i128>,

		pub initial_value: Option<i32>,

		pub values: Option<HashMap<String,String>>,

		pub cycle_time: Option<i32>,

		pub comment: Option<String>,

		pub comments: Option<HashMap<String,String>>,

		pub offset: Option<i32>,

		pub factor: Option<i32>,

		pub unit: Option<String>,
	}


	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct Pdu {
		pub name: Option<String>,

		pub cycle_time: Option<i32>,

		pub id: Option<i32>,

		pub size: Option<i32>,

		pub triggering_name: Option<String>,

		pub pdu_type: Option<String>,

		pub port_type: Option<String>,

		pub signals: Option<Vec<Signal>>,
	}


	#[derive(Serialize, Deserialize, Debug, Clone)]
	pub struct Root {
		pub clusters: Option<HashMap<String, Vec<Frame>>>
	}
}
