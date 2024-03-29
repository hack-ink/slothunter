pub mod util;

mod configuration;
pub use configuration::*;

mod graphql;

mod node;

mod notification;
pub use notification::*;

mod tx;

pub use crate::prelude::*;

// std
use std::{mem, sync::Arc, thread, time::Duration};
// crates.io
use jsonrpsee::{
	async_client::{Client as WsClient, ClientBuilder as WsClientBuilder},
	client_transport::ws::WsTransportClientBuilder,
};
use reqwest::Client;
use scale_value::ValueDef;
#[cfg(feature = "node-test")] use sp_core::{sr25519::Pair, Pair as _};
#[cfg(feature = "node-test")] use subxt::tx::PairSigner;
use subxt::{backend::rpc::RpcClient, config::polkadot::H256, OnlineClient, PolkadotConfig};

type BlockStream = subxt::backend::StreamOf<std::result::Result<Block, subxt::Error>>;
type Block =
	subxt::blocks::Block<subxt::PolkadotConfig, subxt::OnlineClient<subxt::PolkadotConfig>>;

const E_STATE_AUCTION_MUST_BE_SOME: &str = "`state.auction` must be some";

#[derive(Debug)]
pub struct Hunter {
	pub configuration: Configuration,
	pub http: Client,
	_ws_connection: Arc<WsClient>,
	pub node: OnlineClient<PolkadotConfig>,
	pub auction_ending_period: BlockNumber,
	pub auction_sample_length: BlockNumber,
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
		let node = OnlineClient::from_rpc_client(RpcClient::new(client.clone())).await.unwrap();

		Self {
			configuration,
			http: util::http_json_client(),
			_ws_connection: client,
			node,
			auction_ending_period: 0,
			auction_sample_length: 0,
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
		let (mut state, mut block_stream) = self.initialize().await?;

		loop {
			if state.has_bid {
				tracing::info!("skip 1 block after tendering");

				state.has_bid = false;

				Self::next_block(&mut block_stream).await?;
			}

			let block = Self::next_block(&mut block_stream).await?;

			state.block_height = block.number();
			state.block_hash = block.hash();

			tracing::info!("block(#{}, {:?})", state.block_height, state.block_hash);

			self.update(&mut state).await?;
			self.hunt(&mut state).await?;
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

		self.node = OnlineClient::from_rpc_client(RpcClient::new(client.clone())).await?;
		self._ws_connection = client;

		Ok(())
	}

	async fn ws_connect(uri: &str) -> Result<Arc<WsClient>> {
		let (tx, rx) = WsTransportClientBuilder::default().build(uri.parse()?).await?;

		Ok(Arc::new(WsClientBuilder::default().build_with_tokio(tx, rx)))
	}

	async fn initialize(&mut self) -> Result<(State, BlockStream)> {
		self.auction_ending_period = self.auction_ending_period().await?;
		self.auction_sample_length = self.auction_sample_length().await?;

		let mut state = State::default();
		let mut block_stream = match self.configuration.block_subscription_mode {
			BlockSubscriptionMode::Best => self.node.blocks().subscribe_best().await?,
			BlockSubscriptionMode::Finalized => self.node.blocks().subscribe_finalized().await?,
		};
		let block_hash = Self::next_block(&mut block_stream).await?.hash();

		self.check(&block_hash).await?;

		self.bidder = if self.is_self_funded() {
			self.configuration.bid.real
		} else {
			util::crowdloan_id_of(self.fund_index_at(&block_hash).await?.unwrap_or_else(|| {
				panic!(
					"no existing crowdloan found for parachain({})",
					self.configuration.bid.para_id
				)
			}))
		};
		state.auction = self.auction_at(&block_hash).await?;
		state.auction_is_open = state.auction.is_some();

		Ok((state, block_stream))
	}

	// TODO: bug from clippy
	#[allow(clippy::needless_pass_by_ref_mut)]
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
				.value
			else {
				panic!("{E_DELEGATE}")
			};

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

	async fn update(&self, state: &mut State) -> Result<()> {
		let previous_auction = {
			let auction = self.auction_at(&state.block_hash).await?;

			mem::replace(&mut state.auction, auction)
		};

		state.auction_is_open = match (state.auction_is_open, state.auction.is_some()) {
			// An auction has just been opened.
			(false, true) => {
				let a = state.auction.as_ref().expect("`state.auction` must be some");

				self.notify_mail(a, "auction has just been started");
				self.notify_webhook(a, "auction has just been started").await;

				true
			},
			// An auction has just been closed.
			(true, false) => {
				let a = previous_auction.expect("`previous_auction` must be some");

				self.notify_mail(&a, "auction has just been closed");
				self.notify_webhook(&a, "auction has just been closed").await;

				*state = State::default();

				false
			},
			// Still in/not in an auction.
			(_, is_some) => is_some,
		};

		Ok(())
	}

	async fn hunt(&self, state: &mut State) -> Result<()> {
		let Some(auction) = &state.auction else { return Ok(()) };

		self.check_leases(auction.first_lease_period);

		let end_at = auction.ending_period_start_at + self.auction_ending_period;

		if end_at <= state.block_height {
			return Ok(());
		}

		tracing::info!("  {}", auction.fmt(state.block_height, end_at));

		if !self.analyze_bidders(state).await? {
			return Ok(());
		}
		if !self.analyze_winning(state).await? {
			return Ok(());
		}

		self.analyze_winners(state).await
	}

	fn check_leases(&self, first_lease_period: u32) {
		let a @ (first, last) = util::range_of(first_lease_period);
		let b @ (c_first, c_last) = self.configuration.bid.leases;

		if !util::check_leases(&a, &b) {
			panic!("invalid leases configuration, available range(#{first}, #{last}) but found range(#{c_first}, #{c_last})")
		}
	}

	async fn analyze_bidders(&self, state: &mut State) -> Result<bool> {
		let auction = state.auction.as_ref().expect(E_STATE_AUCTION_MUST_BE_SOME);

		tracing::info!("    bidders");

		let bidders = self.bidders_at(&state.block_hash).await?;

		if bidders.is_empty() {
			tracing::info!("      no bidders were found");

			self.try_tender(state, auction.index, self.configuration.bid.increment).await?;

			// No need to do further analysis if there is no bidder.
			return Ok(false);
		}

		bidders.into_iter().for_each(|b| {
			tracing::info!("      {}", b.fmt(&self.configuration.token));

			if let Some(l) = &b.last_accepted_bid {
				tracing::info!("        last accepted bid is {}", l.fmt(&self.configuration.token));

				if self.is_bidder(&b.who, b.para_id) {
					state.bid_amount = l.amount;
				}
			}
		});

		Ok(true)
	}

	async fn analyze_winning(&self, state: &mut State) -> Result<bool> {
		let auction = state.auction.as_ref().expect(E_STATE_AUCTION_MUST_BE_SOME);

		tracing::info!("    winning");

		state.winning = self
			.winning_at(&state.block_hash, state.block_height, auction.ending_period_start_at)
			.await?
			.expect("winning must be some if bidders is not empty");

		if state.winning.0.iter().all(Option::is_none) {
			tracing::info!("      no winning has been calculated yet");

			// No need to do further analysis if there is no winning.
			return Ok(false);
		}

		state
			.winning
			.fmt(&self.configuration.token, auction.first_lease_period)
			.into_iter()
			.for_each(|w| tracing::info!("      {w}"));

		Ok(true)
	}

	async fn analyze_winners(&self, state: &mut State) -> Result<()> {
		let auction = state.auction.as_ref().expect(E_STATE_AUCTION_MUST_BE_SOME);

		tracing::info!("    winner(s)");

		let (winners, threshold) = state.winning.result();
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
					"height": state.block_height,
					"hash": state.block_hash,
				},
				"winning": state.winning.0.as_slice(),
				"winners": winners,

			}),
			&format!("at block(#{}, {:?})\n{notification}", state.block_hash, state.block_height),
		)
		.await;

		if !self.is_winner(&winners) {
			let leases = (
				self.configuration.bid.leases.0 - auction.first_lease_period,
				self.configuration.bid.leases.1 - auction.first_lease_period,
			);
			let bid = state.winning.minimum_bid_to_win(&leases, threshold)
				+ self.configuration.bid.increment;

			self.try_tender(state, auction.index, bid).await?;
		}

		Ok(())
	}

	async fn try_tender(&self, state: &mut State, auction_index: u32, bid: Balance) -> Result<()> {
		if self.watch_only() {
			fn log(mode: &str, bid: String) -> String {
				format!("    slothunter is running under the watch-only mode, {mode} {bid} manually to win")
			}

			let notification = if self.is_self_funded() {
				log("bid", self.configuration.token.fmt(bid))
			} else {
				log("contribute", self.configuration.token.fmt(bid - state.bid_amount))
			};

			tracing::warn!("{notification}");

			self.notify_webhook(&None::<()>, &notification).await;

			Ok(())
		} else {
			fn log(mode: &str, bid: String, upper_limit: String) -> String {
				format!("    skip {mode} {bid} because it exceeds the upper limit {upper_limit}")
			}

			let notification;
			let unaffordable;

			if self.is_self_funded() {
				if self.can_spend(bid) {
					if let Err(e) = self.bid(auction_index, bid).await? {
						let n = format!("    bid failed due to error({e:?})");

						tracing::error!("{n}");

						state.has_bid = false;
						state.retries += 1;
						unaffordable = false;
						notification = n.trim_start_matches(' ').to_string();
					} else {
						let n = format!("    bid with {}", self.configuration.token.fmt(bid));

						tracing::info!("{n}");

						state.has_bid = true;
						state.retries = 0;
						unaffordable = false;
						notification = n.trim_start_matches(' ').to_string();
					}
				} else {
					let n = log(
						"bidding",
						self.configuration.token.fmt(bid),
						self.configuration.token.fmt(self.configuration.bid.upper_limit),
					);

					tracing::warn!("{n}");

					state.has_bid = false;
					state.retries = 0;
					unaffordable = true;
					notification = n.trim_start_matches(' ').to_string();
				}
			} else {
				let bid = bid - state.bid_amount;

				if self.can_spend(bid) {
					if let Err(e) = self.contribute(bid).await? {
						let n = format!("    contribute failed due to error({e:?})");

						tracing::error!("{n}");

						state.has_bid = false;
						state.retries += 1;
						unaffordable = false;
						notification = n.trim_start_matches(' ').to_string();
					} else {
						let n =
							format!("    contribute with {}", self.configuration.token.fmt(bid));

						tracing::info!("{n}");

						state.has_bid = true;
						state.retries = 0;
						unaffordable = false;
						notification = n.trim_start_matches(' ').to_string();
					}
				} else {
					let n = log(
						"contributing",
						self.configuration.token.fmt(bid),
						self.configuration.token.fmt(self.configuration.bid.upper_limit),
					);

					tracing::warn!("{n}");

					state.has_bid = false;
					state.retries = 0;
					unaffordable = true;
					notification = n.trim_start_matches(' ').to_string();
				}
			}

			if state.retries < 5 && !state.unaffordable {
				self.notify_mail(&None::<()>, &notification);
			}

			self.notify_webhook(&None::<()>, &notification).await;

			state.unaffordable = unaffordable;

			Ok(())
		}
	}
}

#[derive(Debug, Default)]
struct State {
	block_hash: H256,
	block_height: BlockNumber,
	auction: Option<AuctionDetail>,
	auction_is_open: bool,
	has_bid: bool,
	bid_amount: Balance,
	winning: Winning,
	retries: u8,
	unaffordable: bool,
}
