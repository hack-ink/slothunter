// crates.io
use scale_decode::DecodeAsType;
use serde::Deserialize;
use serde_json::Value;
use subxt::{dynamic::DecodedValue, events::StaticEvent};
// slothunter
use crate::hunter::*;

#[derive(Debug, Deserialize)]
pub struct EResponse<E> {
	pub data: Events<E>,
}

#[derive(Debug, Deserialize)]
pub struct Events<E> {
	pub events: Vec<E>,
}

#[derive(Debug, Deserialize)]
pub struct RawEvent {
	pub args: Value,
	pub block: Value,
}

#[derive(Debug, Deserialize)]
pub struct Event<E> {
	pub args: E,
	pub block: BlockResponse,
}

#[derive(Debug, Deserialize)]
pub struct BlockResponse {
	pub height: BlockNumber,
}

#[derive(Debug, Deserialize)]
pub struct EBidAccepted {
	pub amount: String,
	#[serde(rename(deserialize = "firstSlot"))]
	pub first_slot: u32,
	#[serde(rename(deserialize = "lastSlot"))]
	pub last_slot: u32,
}

#[derive(Debug, DecodeAsType)]
pub struct EProxyExecuted {
	pub result: StdResult<(), DecodedValue>,
}
impl EProxyExecuted {
	pub fn into_dispatch_result(self) -> DispatchResult {
		match self.result {
			Ok(()) => Ok(()),
			Err(v) => Err(format!("{v:?}")),
		}
	}
}
impl StaticEvent for EProxyExecuted {
	const EVENT: &'static str = "ProxyExecuted";
	const PALLET: &'static str = "Proxy";
}
