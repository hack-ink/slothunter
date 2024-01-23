// crates.io
use parity_scale_codec::Decode;
use serde::Deserialize;
use subxt::{
	config::polkadot::H256,
	dynamic::{self, At, Value},
};
// slothunter
use crate::hunter::*;

const E_DE: &str = "failed to decode/deserialize, type need to be updated";
const E_TYPE_CONVERSION: &str = "type conversion never fails";

impl Hunter {
	pub async fn auction_ending_period(&self) -> Result<BlockNumber> {
		Ok(self
			.node
			.constants()
			.at(&dynamic::constant("Auctions", "EndingPeriod"))?
			.to_value()?
			.as_u128()
			.expect(E_TYPE_CONVERSION) as _)
	}

	pub async fn auction_sample_length(&self) -> Result<BlockNumber> {
		Ok(self
			.node
			.constants()
			.at(&dynamic::constant("Auctions", "SampleLength"))?
			.to_value()?
			.as_u128()
			.expect(E_TYPE_CONVERSION) as _)
	}

	pub async fn proxies_at(
		&self,
		block: &H256,
		real: &AccountId,
	) -> Result<Option<Vec<ProxyDefinition>>> {
		self.node
			.storage()
			.at(block.to_owned())
			.fetch(&dynamic::storage("Proxy", "Proxies", vec![Value::from_bytes(real)]))
			.await?
			.map(|p| {
				// https://github.com/paritytech/substrate/blob/51b2f0ed6af8dd4facb18f1a489e192fd0673f7b/frame/proxy/src/lib.rs#L573
				let (p, _) =
					<(Vec<UnnamedWrapper<ProxyDefinition>>, Balance)>::deserialize(p.to_value()?)
						.expect(E_DE);
				let r: Result<Vec<ProxyDefinition>> = Ok(p.into_iter().map(|p| p.r#type).collect());

				r
			})
			.transpose()
	}

	pub async fn auction_at(&self, block: &H256) -> Result<Option<AuctionDetail>> {
		const E_STORAGE_TYPE: &str = "`AuctionInfo` has an invalid storage type";

		if let Some(auction_info) = self
			.node
			.storage()
			.at(block.to_owned())
			.fetch(&dynamic::storage("Auctions", "AuctionInfo", <Vec<()>>::new()))
			.await?
		{
			let auction_info = auction_info.to_value()?;
			let auction_counter = self
				.node
				.storage()
				.at(block.to_owned())
				.fetch(&dynamic::storage("Auctions", "AuctionCounter", <Vec<()>>::new()))
				.await?
				.expect("`Auction::AuctionCounter` must exist");
			let auction_counter = auction_counter.to_value()?;

			Ok(Some(AuctionDetail {
				index: auction_counter.as_u128().expect(E_TYPE_CONVERSION) as _,
				first_lease_period: auction_info
					.at(0)
					.expect(E_STORAGE_TYPE)
					.as_u128()
					.expect(E_TYPE_CONVERSION) as _,
				ending_period_start_at: auction_info
					.at(1)
					.expect(E_STORAGE_TYPE)
					.as_u128()
					.expect(E_TYPE_CONVERSION) as _,
			}))
		} else {
			Ok(None)
		}
	}

	pub async fn fund_index_at(&self, block: &H256) -> Result<Option<u32>> {
		if let Some(f) = self
			.node
			.storage()
			.at(block.to_owned())
			.fetch(&dynamic::storage(
				"Crowdloan",
				"Funds",
				vec![Value::u128(self.configuration.bid.para_id as _)],
			))
			.await?
		{
			Ok(Some(
				f.to_value()?.at("fund_index").expect(E_DE).as_u128().expect(E_TYPE_CONVERSION)
					as _,
			))
		} else {
			Ok(None)
		}
	}

	pub async fn bidders_at(&self, block: &H256) -> Result<Vec<Bidder>> {
		let mut bidders = Vec::new();
		let mut bidders_storage = self
			.node
			.storage()
			.at(block.to_owned())
			.iter(dynamic::storage("Auctions", "ReservedAmounts", <Vec<()>>::new()))
			.await?;

		while let Some(r) = bidders_storage.next().await {
			let (k, v) = r?;
			// twox64_concat
			// (twox128(b"Auctions") + twox128(b"ReservedAmounts") + twox64(key)).len() = 40
			// key = k.0[40..]
			let (who, para_id) = <(AccountId, ParaId)>::decode(&mut &k[40..]).expect(E_DE);
			let existing_deposit = self
				.node
				.storage()
				.at(block.to_owned())
				.fetch(&dynamic::storage("Slots", "Leases", vec![Value::u128(para_id as _)]))
				.await?
				.and_then(|l: dynamic::DecodedValueThunk| {
					SLeases::decode(&mut &*l.into_encoded())
						.expect(E_DE)
						.into_iter()
						.filter_map(|l| l.and_then(|(w, a)| if who == w { Some(a) } else { None }))
						.max()
				})
				.unwrap_or_default();
			let last_accepted_bid =
				self.last_accepted_bid_of(&array_bytes::bytes2hex("0x", who), para_id).await?;

			bidders.push(Bidder {
				who,
				para_id,
				reserved: v.to_value()?.as_u128().expect(E_TYPE_CONVERSION),
				existing_deposit,
				last_accepted_bid,
			});
		}

		Ok(bidders)
	}

	pub async fn winning_at(
		&self,
		block: &H256,
		now: BlockNumber,
		ending_period_start_at: BlockNumber,
	) -> Result<Option<Winning>> {
		let winning_offset =
			util::winning_offset_of(now, ending_period_start_at, self.auction_sample_length);

		Ok(self
			.node
			.storage()
			.at(block.to_owned())
			.fetch(&dynamic::storage("Auctions", "Winning", vec![Value::u128(winning_offset as _)]))
			.await?
			.map(|w| Winning::of(SWinning::decode(&mut &*w.into_encoded()).expect(E_DE))))
	}
}
