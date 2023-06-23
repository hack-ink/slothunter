// crates.io
use scale_value::Value;
use serde::Deserialize;
// slothunter
use crate::hunter::*;

// https://github.com/paritytech/polkadot/blob/b1cc6fa14330261a305d56be36c04e9c99518993/runtime/common/src/slots/mod.rs#L117
pub type SLeases = Vec<Option<(AccountId, Balance)>>;
// https://github.com/paritytech/polkadot/blob/b1cc6fa14330261a305d56be36c04e9c99518993/runtime/common/src/auctions.rs#L73
pub type SWinning = [Option<SWinner>; 36];
pub type SWinner = (AccountId, ParaId, Balance);

// https://github.com/paritytech/substrate/blob/51b2f0ed6af8dd4facb18f1a489e192fd0673f7b/frame/proxy/src/lib.rs#LL76C32-L76C32
#[derive(Debug, Deserialize)]
pub struct ProxyDefinition {
	pub delegate: UnnamedWrapper<AccountId>,
	pub proxy_type: Value<()>,
	pub delay: BlockNumber,
}
