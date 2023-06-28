pub mod util;

mod configuration;
pub use configuration::*;

mod graphql;
pub use graphql::*;

mod node;
pub use node::*;

mod notification;
pub use notification::*;

mod tx;
pub use tx::*;

pub use crate::prelude::*;

// std
use std::{mem, sync::Arc, thread, time::Duration};
// crates.io
use futures::StreamExt;
use jsonrpsee::{
	async_client::{Client as WsClient, ClientBuilder as WsClientBuilder},
	client_transport::ws::WsTransportClientBuilder,
};
use reqwest::Client;
use scale_value::ValueDef;
#[cfg(feature = "node-test")] use sp_core::{sr25519::Pair, Pair as _};
#[cfg(feature = "node-test")] use subxt::tx::PairSigner;
use subxt::{config::polkadot::H256, OnlineClient, PolkadotConfig};

type BlockStream = std::pin::Pin<
	Box<dyn Send + futures::Stream<Item = std::result::Result<Block, subxt::error::Error>>>,
>;
type Block =
	subxt::blocks::Block<subxt::PolkadotConfig, subxt::OnlineClient<subxt::PolkadotConfig>>;

#[derive(Debug)]
pub struct Hunter {
	pub configuration: Configuration,
	pub http: Client,
	_ws_connection: Arc<WsClient>,
	pub node: OnlineClient<PolkadotConfig>,
	pub auction: Option<AuctionDetail>,
	pub auction_ending_period: BlockNumber,
	pub auction_sample_length: BlockNumber,
	pub auction_is_open: bool,
	pub bidder: AccountId,
}
impl Hunter {
	#[allow(unused)]
	#[cfg(feature = "node-test")]
	async fn tester() -> Self {
		let configuration = Configuration {
			graphql_endpoint: "http://127.0.0.1:3000/graphql".into(),
			node_endpoint: "ws://127.0.0.1:9944".into(),
			block_subscription_mode: BlockSubscriptionMode::Best,
			token: Token { symbol: "UNIT", decimals: 12. },
			bid: Bid {
				para_id: 2000,
				leases: (0, 0),
				watch_only: false,
				r#type: BidType::SelfFunded,
				real: array_bytes::hex2array_unchecked(
					"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d",
				),
				delegate: PairSigner::new(Pair::from_seed(&array_bytes::hex2array_unchecked(
					"0x3c881bc4d45926680c64a7f9315eeda3dd287f8d598f3653d7c107799c5422b3",
				))),
				upper_limit: 100_000_000_000_000,
				increment: 1_000_000_000_000,
			},
			notification: Notification { mail: None, webhooks: Vec::new() },
		};
		let client = Self::ws_connect(&configuration.node_endpoint).await.unwrap();
		let node = OnlineClient::from_rpc_client(client.clone()).await.unwrap();

		Self {
			configuration,
			http: util::http_json_client(),
			_ws_connection: client,
			node,
			auction: None,
			auction_ending_period: 0,
			auction_sample_length: 0,
			auction_is_open: false,
			bidder: AccountId::default(),
		}
	}

	fn watch_only(&self) -> bool {
		self.configuration.bid.watch_only
	}

	fn is_self_funded(&self) -> bool {
		self.configuration.bid.r#type == BidType::SelfFunded
	}

	fn can_spend(&self, value: Balance) -> bool {
		self.configuration.bid.upper_limit >= value
	}

	fn is_bidder(&self, who: &AccountId, para_id: ParaId) -> bool {
		who == &self.bidder && para_id == self.configuration.bid.para_id
	}

	fn is_winner(&self, winners: &[Winner]) -> bool {
		winners.iter().any(|w| self.is_bidder(&w.who, w.para_id))
	}

	pub async fn start(&mut self) -> Result<()> {
		let mut block_stream = self.initialize().await?;
		let mut has_bid = false;

		loop {
			let block = if has_bid {
				tracing::info!("skip 1 block after tendering");

				has_bid = false;

				Self::next_block(&mut block_stream).await?;

				Self::next_block(&mut block_stream).await?
			} else {
				Self::next_block(&mut block_stream).await?
			};
			let block_height = block.number();
			let block_hash = block.hash();

			tracing::info!("block(#{block_height}, {block_hash:?})");

			self.update(&block_hash).await?;
			self.hunt(block_height, block_hash, &mut has_bid).await?;
		}
	}

	pub fn ws_is_connected(&self) -> bool {
		self._ws_connection.is_connected()
	}

	pub async fn ws_reconnect(&mut self, tried: &mut bool) -> Result<()> {
		if *tried {
			thread::sleep(Duration::from_secs(5));
		}

		*tried = true;

		let client = Self::ws_connect(&self.configuration.node_endpoint).await?;

		self.node = OnlineClient::from_rpc_client(client.clone()).await?;
		self._ws_connection = client;

		Ok(())
	}

	async fn ws_connect(uri: &str) -> Result<Arc<WsClient>> {
		let (tx, rx) = WsTransportClientBuilder::default().build(uri.parse()?).await?;

		Ok(Arc::new(WsClientBuilder::default().build_with_tokio(tx, rx)))
	}

	async fn initialize(&mut self) -> Result<BlockStream> {
		self.auction_ending_period = self.auction_ending_period().await?;
		self.auction_sample_length = self.auction_sample_length().await?;

		let mut block_stream = match self.configuration.block_subscription_mode {
			BlockSubscriptionMode::Best => self.node.blocks().subscribe_best().await?,
			BlockSubscriptionMode::Finalized => self.node.blocks().subscribe_finalized().await?,
		};
		let block_hash = Self::next_block(&mut block_stream).await?.hash();

		self.check(&block_hash).await?;

		self.auction = self.auction_at(&block_hash).await?;
		self.auction_is_open = self.auction.is_some();
		self.bidder = if self.is_self_funded() {
			self.configuration.bid.real
		} else {
			util::crowdloan_id_of(self.fund_index_at(&block_hash).await?.expect(&format!(
				"no existing crowdloan found for parachain({})",
				self.configuration.bid.para_id,
			)))
		};

		Ok(block_stream)
	}

	async fn next_block(block_stream: &mut BlockStream) -> Result<Block> {
		Ok(block_stream.next().await.ok_or(anyhow::anyhow!("failed to get the next block"))??)
	}

	async fn check(&self, block_hash: &H256) -> Result<()> {
		tracing::info!("############################################################");

		{
			let uri = &self.configuration.node_endpoint;

			if util::check_ws_uri(uri) {
				tracing::info!("node endpoint({uri})");
			} else {
				panic!("invalid node endpoint({uri})");
			}
		}

		{
			let uri = &self.configuration.graphql_endpoint;

			if util::check_http_uri(uri) {
				tracing::info!("graphql endpoint({uri})");
			} else {
				panic!("invalid graphql endpoint({uri})");
			}
		}

		tracing::info!("bid");
		tracing::info!("  hunting a slot for parachain({})", self.configuration.bid.para_id);
		tracing::info!("  watch-only({})", self.watch_only());

		if !self.watch_only() {
			const E_DELEGATE: &str = "no delegate(`ProxyType::All` or `ProxyType::Auction`) found, please check your configurations";

			tracing::info!("  funding type({})", self.configuration.bid.r#type);

			let real = &self.configuration.bid.real;
			let delegate = self.configuration.bid.delegate.account_id();

			tracing::info!("  real account({})", array_bytes::bytes2hex("0x", real));
			tracing::info!("  proxy delegate({})", array_bytes::bytes2hex("0x", delegate.0));

			let ValueDef::Variant(v) = self
				.proxies_at(block_hash, real)
				.await?
				.expect(E_DELEGATE)
				.into_iter()
				.find(|p| p.delegate.r#type == delegate.0)
				.expect(E_DELEGATE)
				.proxy_type
				.value else { panic!("{E_DELEGATE}") };

			if !matches!(v.name.as_str(), "All" | "Auction") {
				panic!("{E_DELEGATE}")
			}

			tracing::info!("  proxy type({})", v.name);
			tracing::info!(
				"  upper limit {}",
				self.configuration.token.fmt(self.configuration.bid.upper_limit)
			);

			let increment = self.configuration.bid.increment;

			tracing::info!("  increment {}", self.configuration.token.fmt(increment));

			assert!(
				increment as f64 / 10_f64.powf(self.configuration.token.decimals) >= 1.,
				"increment should be at least {}(1)",
				self.configuration.token.symbol
			);
		}

		tracing::info!("notification");
		tracing::info!("  webhooks");

		self.configuration.notification.webhooks.iter().for_each(|u| {
			tracing::info!("    uri({})", u);
		});

		if let Some(m) = &self.configuration.notification.mail {
			tracing::info!("  mail");
			tracing::info!("    sender({})", m.sender.username.email);

			if util::check_smtp_uri(&m.sender.smtp) {
				tracing::info!("    smtp({})", m.sender.smtp);
			} else {
				panic!("invalid smtp({})", m.sender.smtp);
			}

			for to in &m.receivers {
				tracing::info!("    receiver({})", to.email);
			}
		}

		tracing::info!("############################################################");

		Ok(())
	}

	async fn update(&mut self, block_hash: &H256) -> Result<()> {
		let previous_auction = {
			let auction = self.auction_at(block_hash).await?;

			mem::replace(&mut self.auction, auction)
		};

		self.auction_is_open = match (self.auction_is_open, self.auction.is_some()) {
			// An auction has just been opened.
			(false, true) => {
				let a = self.auction.as_ref().expect("`self.auction` must be some");

				self.notify_mail(a, "auction has just been started");
				self.notify_webhook(a, "auction has just been started").await;

				true
			},
			// An auction has just been closed.
			(true, false) => {
				let a = previous_auction.expect("`previous_auction` must be some");

				self.notify_mail(&a, "auction has just been closed");
				self.notify_webhook(&a, "auction has just been closed").await;

				false
			},
			// Still in/not in an auction.
			(_, is_some) => is_some,
		};

		Ok(())
	}

	async fn hunt(
		&self,
		block_height: BlockNumber,
		block_hash: H256,
		has_bid: &mut bool,
	) -> Result<()> {
		let Some(auction) = &self.auction else { return Ok(()) };
		let end_at = auction.ending_period_start_at + self.auction_ending_period;

		if end_at <= block_height {
			return Ok(());
		}

		tracing::info!("  {}", auction.fmt(block_height, end_at));

		let Some(self_bid) = self.analyze_bidders(&block_hash, auction, has_bid).await? else { return Ok(()); };
		let Some(winning) = self.analyze_winning(
			&block_hash,
			block_height,
			auction,
		).await? else { return Ok(()); };

		self.analyze_winners(block_height, &block_hash, auction, &winning, self_bid, has_bid).await
	}

	async fn analyze_bidders(
		&self,
		block_hash: &H256,
		auction: &AuctionDetail,
		has_bid: &mut bool,
	) -> Result<Option<Balance>> {
		tracing::info!("    bidders");

		let bidders = self.bidders_at(block_hash).await?;

		if bidders.is_empty() {
			tracing::info!("      no bidders were found");

			*has_bid = self.try_tender(auction.index, self.configuration.bid.increment, 0).await?;

			return Ok(None);
		}

		let mut self_bid = 0;

		bidders.into_iter().for_each(|b| {
			tracing::info!("      {}", b.fmt(&self.configuration.token));

			if let Some(l) = &b.last_accepted_bid {
				tracing::info!("        last accepted bid is {}", l.fmt(&self.configuration.token));

				if self.is_bidder(&b.who, b.para_id) {
					self_bid = l.amount;
				}
			}
		});

		Ok(Some(self_bid))
	}

	async fn analyze_winning(
		&self,
		block_hash: &H256,
		block_height: BlockNumber,
		auction: &AuctionDetail,
	) -> Result<Option<Winning>> {
		tracing::info!("    winning");

		let winning = self
			.winning_at(block_hash, block_height, auction.ending_period_start_at)
			.await?
			.expect("winning must be some if bidders is not empty");

		if winning.0.iter().all(Option::is_none) {
			tracing::info!("      no winning has been calculated yet");

			return Ok(None);
		}

		winning
			.fmt(&self.configuration.token, auction.first_lease_period)
			.into_iter()
			.for_each(|w| tracing::info!("      {w}"));

		Ok(Some(winning))
	}

	async fn analyze_winners(
		&self,
		block_height: BlockNumber,
		block_hash: &H256,
		auction: &AuctionDetail,
		winning: &Winning,
		self_bid: Balance,
		has_bid: &mut bool,
	) -> Result<()> {
		tracing::info!("    winner(s)");

		let (winners, threshold) = winning.result();
		let notification = winners
			.iter()
			.map(|w| {
				let n = w.fmt(&self.configuration.token, auction.first_lease_period);

				tracing::info!("      {n}");

				n
			})
			.collect::<Vec<_>>()
			.join(",");

		self.notify_webhook(
			&serde_json::json!({
				"block": {
					"height": block_height,
					"hash": block_hash,
				},
				"winning": winning.0.as_slice(),
				"winners": winners,

			}),
			&format!("at block(#{block_height}, {block_hash:?})\n{notification}"),
		)
		.await;

		if !self.is_winner(&winners) {
			let leases = (
				self.configuration.bid.leases.0 - auction.first_lease_period,
				self.configuration.bid.leases.1 - auction.first_lease_period,
			);
			let bid =
				winning.minimum_bid_to_win(&leases, threshold) + self.configuration.bid.increment;

			*has_bid = self.try_tender(auction.index, bid, self_bid).await?;
		}

		Ok(())
	}

	async fn try_tender(
		&self,
		auction_index: u32,
		bid: Balance,
		self_bid: Balance,
	) -> Result<bool> {
		if self.watch_only() {
			fn log(mode: &str, bid: String) {
				tracing::warn!("    slothunter is running under the watch-only mode, {mode} {bid} manually to win");
			}

			if self.is_self_funded() {
				log("bid", self.configuration.token.fmt(bid))
			} else {
				log("contribute", self.configuration.token.fmt(bid - self_bid))
			}

			Ok(false)
		} else {
			fn log(mode: &str, bid: String, upper_limit: String) -> String {
				format!("    skip {mode} {bid} because it exceeds the upper limit {upper_limit}")
			}

			let mut has_bid = false;
			let notification;

			if self.is_self_funded() {
				if self.can_spend(bid) {
					if let Err(e) = self.bid(auction_index, bid).await? {
						let n = format!("    bid failed due to error({e:?})");

						tracing::error!("{n}");

						notification = n.trim_start_matches(' ').to_string();
					} else {
						let n = format!("    bid with {}", self.configuration.token.fmt(bid));

						tracing::info!("{n}");

						has_bid = true;
						notification = n.trim_start_matches(' ').to_string();
					}
				} else {
					let n = log(
						"bidding",
						self.configuration.token.fmt(bid),
						self.configuration.token.fmt(self.configuration.bid.upper_limit),
					);

					tracing::warn!("{n}");

					notification = n.trim_start_matches(' ').to_string();
				}
			} else {
				let bid = bid - self_bid;

				if self.can_spend(bid) {
					if let Err(e) = self.contribute(bid).await? {
						let n = format!("    contribute failed due to error({e:?})");

						tracing::error!("{n}");

						notification = n.trim_start_matches(' ').to_string();
					} else {
						let n =
							format!("    contribute with {}", self.configuration.token.fmt(bid));

						tracing::info!("{n}");

						has_bid = true;
						notification = n.trim_start_matches(' ').to_string();
					}
				} else {
					let n = log(
						"contributing",
						self.configuration.token.fmt(bid),
						self.configuration.token.fmt(self.configuration.bid.upper_limit),
					);

					tracing::warn!("{n}");

					notification = n.trim_start_matches(' ').to_string();
				}
			}

			self.notify_mail(&None::<()>, &notification);
			self.notify_webhook(&None::<()>, &notification).await;

			Ok(has_bid)
		}
	}
}
