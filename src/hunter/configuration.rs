// std
use std::{
	fmt::{Debug, Display, Formatter, Result as FmtResult},
	fs,
	path::PathBuf,
};
// crates.io
use app_dirs2::{AppDataType, AppInfo};
use serde::Deserialize;
use sp_core::{sr25519::Pair, Pair as _};
use subxt::tx::PairSigner;
// slothunter
use crate::hunter::*;

pub const SLOTHUNTER: AppInfo = AppInfo { name: "slothunter", author: "Xavier Lau" };

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigurationToml {
	pub network: Network,
	pub graphql_endpoint: Option<String>,
	pub node_endpoint: Option<String>,
	pub block_subscription_mode: BlockSubscriptionMode,
	pub bid: BidToml,
	pub notification: NotificationToml,
}
impl ConfigurationToml {
	pub fn load(path: Option<PathBuf>) -> Result<Self> {
		fn initialize(path: &PathBuf) -> Result<ConfigurationToml> {
			if !path.is_file() {
				let s = include_str!("../../configuration-template.toml");

				fs::write(path, s)?;

				Ok(toml::from_str(s)?)
			} else {
				Ok(toml::from_str(&fs::read_to_string(path)?)?)
			}
		}

		let path = path.unwrap_or(app_dirs2::app_root(AppDataType::UserConfig, &SLOTHUNTER)?);

		if matches!(path.extension().map(|s| s.to_str().unwrap_or_default()), Some("toml")) {
			return initialize(&path);
		}
		if !path.is_dir() {
			fs::create_dir_all(&path)?;
		}

		let path = path.join("config.toml");

		initialize(&path)
	}

	pub fn try_into_configuration(self) -> Result<Configuration> {
		let Self {
			network,
			graphql_endpoint,
			node_endpoint,
			block_subscription_mode,
			bid:
				BidToml { para_id, leases, watch_only, r#type, real, delegate, upper_limit, increment },
			notification: NotificationToml { mail, webhooks },
		} = self;
		let graphql_endpoint =
			graphql_endpoint.unwrap_or_else(|| network.graphql_endpoint().to_owned());
		let node_endpoint = node_endpoint.unwrap_or_else(|| network.node_endpoint().to_owned());

		Ok(Configuration {
			graphql_endpoint,
			node_endpoint,
			block_subscription_mode,
			token: network.token(),
			bid: Bid {
				para_id,
				leases,
				watch_only,
				r#type,
				real: array_bytes::hex2array(real)
					.map_err(|e| anyhow::anyhow!("invalid public key, {e:?}"))?,
				delegate: PairSigner::new(Pair::from_seed(
					&array_bytes::hex2array(delegate)
						.map_err(|e| anyhow::anyhow!("invalid seed, {e:?}"))?,
				)),
				upper_limit: upper_limit.parse()?,
				increment: increment.parse()?,
			},
			notification: Notification {
				mail: mail
					.map(|m| -> Result<_> {
						Ok(Mail {
							sender: Sender {
								username: m.sender.username.parse()?,
								password: m.sender.password,
								smtp: m.sender.smtp,
							},
							receivers: m
								.receivers
								.into_iter()
								.map(|r| r.parse())
								.collect::<StdResult<_, _>>()?,
						})
					})
					.transpose()?,
				webhooks,
			},
		})
	}
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BidToml {
	pub para_id: ParaId,
	pub leases: SlotRange,
	pub watch_only: bool,
	pub r#type: BidType,
	pub real: String,
	pub delegate: String,
	pub upper_limit: String,
	pub increment: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotificationToml {
	pub mail: Option<MailToml>,
	pub webhooks: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MailToml {
	pub sender: SenderToml,
	pub receivers: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SenderToml {
	pub username: String,
	pub password: String,
	pub smtp: String,
}

#[derive(Debug)]
pub struct Configuration {
	pub graphql_endpoint: String,
	pub node_endpoint: String,
	pub block_subscription_mode: BlockSubscriptionMode,
	pub token: Token,
	pub bid: Bid,
	pub notification: Notification,
}
pub struct Bid {
	pub para_id: ParaId,
	pub leases: SlotRange,
	pub watch_only: bool,
	pub r#type: BidType,
	pub real: AccountId,
	pub delegate: PairSigner<PolkadotConfig, Pair>,
	pub upper_limit: Balance,
	pub increment: Balance,
}
impl Debug for Bid {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		f.debug_struct("Configuration")
			.field("para_id", &self.para_id)
			.field("real", &array_bytes::bytes2hex("0x", self.real))
			.field("type", &self.r#type)
			.field("delegate", &self.delegate.account_id())
			.finish()
	}
}
#[derive(Debug)]
pub struct Notification {
	pub mail: Option<Mail>,
	pub webhooks: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockSubscriptionMode {
	Best,
	Finalized,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Network {
	Polkadot,
	Kusama,
}
impl Network {
	pub fn graphql_endpoint(&self) -> &'static str {
		match self {
			Self::Polkadot => "https://polkadot.explorer.subsquid.io/graphql",
			Self::Kusama => "https://kusama.explorer.subsquid.io/graphql",
		}
	}

	pub fn node_endpoint(&self) -> &'static str {
		match self {
			Self::Polkadot => "wss://rpc.polkadot.io:443",
			Self::Kusama => "wss://kusama-rpc.polkadot.io:443",
		}
	}

	pub fn token(&self) -> Token {
		match self {
			Self::Polkadot => Token { symbol: "DOT", decimals: 10. },
			Self::Kusama => Token { symbol: "KSM", decimals: 12. },
		}
	}
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BidType {
	SelfFunded,
	Crowdloan,
}
impl Display for BidType {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		match self {
			Self::SelfFunded => write!(f, "self-funded"),
			Self::Crowdloan => write!(f, "crowdloan"),
		}
	}
}

#[derive(Debug)]
pub struct Token {
	pub symbol: &'static str,
	pub decimals: f64,
}
impl Token {
	pub fn fmt(&self, balance: Balance) -> String {
		format!("{}({})", self.symbol, balance as f64 / 10_f64.powf(self.decimals))
	}
}

impl Hunter {
	pub async fn from_configuration(configuration: Configuration) -> Result<Self> {
		let client = Self::ws_connect(&configuration.node_endpoint).await?;
		let node = OnlineClient::from_rpc_client(client.clone()).await?;

		Ok(Self {
			configuration,
			http: util::http_json_client(),
			_ws_connection: client,
			node,
			auction_ending_period: 0,
			auction_sample_length: 0,
			bidder: AccountId::default(),
		})
	}
}
