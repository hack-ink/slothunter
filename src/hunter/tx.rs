// crates.io
use subxt::{
	dynamic::{self, Value},
	tx::TxPayload,
	Error,
};
// slothunter
use crate::hunter::*;

impl Hunter {
	async fn tx<C>(&self, call: &C) -> Result<DispatchResult>
	where
		C: TxPayload,
	{
		match self
			.node
			.tx()
			.sign_and_submit_then_watch_default(call, &self.configuration.bid.delegate)
			.await?
			.wait_for_in_block()
			.await?
			.wait_for_success()
			.await
		{
			Ok(r) => Ok(r
				// Always using proxy in production, this must be some.
				.find_first::<EProxyExecuted>()?
				.map(EProxyExecuted::into_dispatch_result)
				.unwrap_or_else(|| {
					tracing::warn!("this log should only appear in a test");

					Ok(())
				})),
			Err(Error::Runtime(e)) => Ok(Err(e.to_string())),
			Err(e) => Err(e)?,
		}
	}

	pub async fn bid(&self, auction_index: u32, value: Balance) -> Result<DispatchResult> {
		let bid = dynamic::tx(
			"Auctions",
			"bid",
			vec![
				Value::u128(self.configuration.bid.para_id as _),
				Value::u128(auction_index as _),
				Value::u128(self.configuration.bid.leases.0 as _),
				Value::u128(self.configuration.bid.leases.1 as _),
				Value::u128(value),
			],
		);
		let proxied_bid = util::proxy_of(&self.configuration.bid.real, bid);

		self.tx(&proxied_bid).await
	}

	pub async fn contribute(&self, value: Balance) -> Result<DispatchResult> {
		let contribute = dynamic::tx(
			"Crowdloan",
			"contribute",
			vec![
				Value::u128(self.configuration.bid.para_id as _),
				Value::u128(value),
				Value::unnamed_variant("None", []),
			],
		);
		let proxied_contribute = util::proxy_of(&self.configuration.bid.real, contribute);

		self.tx(&proxied_contribute).await
	}
}

#[cfg(feature = "node-test")]
#[tokio::test]
async fn tx_should_work() {
	let hunter = Hunter::tester().await;

	{
		// Use `Alice//stash` to transfer funds from `Alice` to `Alice//stash`.
		let transfer = dynamic::tx(
			"Balances",
			"transfer",
			vec![
				Value::unnamed_variant(
					"Id",
					[Value::from_bytes(hunter.configuration.bid.delegate.account_id())],
				),
				Value::u128(1_000_000_000_000),
			],
		);
		let proxied_transfer = util::proxy_of(&hunter.configuration.bid.real, transfer);

		assert!(hunter.tx(&proxied_transfer).await.unwrap().is_ok());
	}

	{
		// Use `Alice//stash` to transfer too much funds from `Alice` to `Alice//stash`.
		let transfer_too_much = dynamic::tx(
			"Balances",
			"transfer",
			vec![
				Value::unnamed_variant(
					"Id",
					[Value::from_bytes(hunter.configuration.bid.delegate.account_id())],
				),
				Value::u128(10_000_000_000_000_000_000),
			],
		);
		let proxied_transfer_too_much =
			util::proxy_of(&hunter.configuration.bid.real, transfer_too_much);

		assert_eq!(
			hunter.tx(&proxied_transfer_too_much).await.unwrap(),
			Err("Value { value: Variant(Variant { name: \"Token\", values: Unnamed([Value { value: Variant(Variant { name: \"FundsUnavailable\", values: Unnamed([]) }), context: 27 }]) }), context: 25 }".into())
		);
	}

	{
		// Transfer too much funds from `Alice//stash` to `Alice`.
		let transfer_too_much = dynamic::tx(
			"Balances",
			"transfer",
			vec![
				Value::unnamed_variant("Id", [Value::from_bytes(hunter.configuration.bid.real)]),
				Value::u128(10_000_000_000_000_000_000),
			],
		);

		assert_eq!(
			hunter.tx(&transfer_too_much).await.unwrap(),
			Err("Token error: Funds are unavailable.".into())
		);
	}

	{
		// Transfer all funds from `Alice//stash` to `Alice`.
		// Note: after this, `Alice//stash` will no longer be available for other tests.
		let transfer_all = dynamic::tx(
			"Balances",
			"transfer_all",
			vec![
				Value::unnamed_variant("Id", [Value::from_bytes(hunter.configuration.bid.real)]),
				Value::bool(true),
			],
		);

		assert!(hunter.tx(&transfer_all).await.unwrap().is_ok());
	}

	{
		// Use `Alice//stash` to transfer funds from `Alice` to `Alice//stash`.
		let transfer = dynamic::tx(
			"Balances",
			"transfer",
			vec![
				Value::unnamed_variant(
					"Id",
					[Value::from_bytes(hunter.configuration.bid.delegate.account_id())],
				),
				Value::u128(1_000_000_000_000),
			],
		);
		let proxied_transfer = util::proxy_of(&hunter.configuration.bid.real, transfer);

		assert_eq!(
			hunter.tx(&proxied_transfer).await.unwrap_err().to_string(),
			"Rpc error: RPC error: RPC call failed: ErrorObject { code: ServerError(1010), message: \"Invalid Transaction\", data: Some(RawValue(\"Inability to pay some fees (e.g. account balance too low)\")) }".to_string()
		);
	}
}
