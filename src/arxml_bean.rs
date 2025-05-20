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
		pub fn pdu_by_id(&self, id: i32) -> Option<&Pdu> {
			for pdu in self.pdus.as_ref().unwrap() {
				if pdu.id.unwrap() == id {
					return Some(pdu);
				}
			}
			None
		}	
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
