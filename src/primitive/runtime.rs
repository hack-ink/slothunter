mod constant;
pub use constant::*;

mod event;
pub use event::*;

mod storage;
pub use storage::*;

// crates.io
use serde::Deserialize;
use subxt::dynamic::Value;
// slothunter
use crate::prelude::*;

pub type DispatchResult = StdResult<(), String>;

#[derive(Debug, Deserialize)]
pub struct UnnamedWrapper<T> {
	pub r#type: T,
}

#[derive(Debug, Deserialize)]
pub enum DispatchError {
	CannotLookup,
	BadOrigin,
	Module(UnnamedWrapper<ModuleError>),
	ConsumerRemaining,
	NoProviders,
	TooManyConsumers,
	Token(Value<()>),
	Arithmetic(Value<()>),
	Transactional(Value<()>),
	Exhausted,
	Corruption,
	Unavailable,
	RootNotAllowed,
}
#[derive(Debug, Deserialize)]
pub struct ModuleError {
	pub index: u8,
	pub error: [u8; 4],
}
