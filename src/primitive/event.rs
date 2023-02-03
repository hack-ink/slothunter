// crates.io
use serde::Deserialize;
use serde_json::Value;

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
	pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EAuctionStarted {
	pub auction_index: u16,
	pub ending: u32,
	pub lease_period: u8,
}
