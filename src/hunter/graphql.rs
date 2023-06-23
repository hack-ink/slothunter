// std
use std::fmt::{Display, Formatter, Result as FmtResult};
// crates.io
use serde::de::DeserializeOwned;
// slothunter
use crate::hunter::*;

#[derive(Debug)]
pub struct Query<'a> {
	pub limit: u32,
	pub name: &'a str,
	pub args_json_contains: Option<&'a str>,
}
#[allow(unused)]
impl<'a> Query<'a> {
	pub fn new(name: &'a str) -> Self {
		Self { limit: 1, name, args_json_contains: None }
	}

	pub fn limit(mut self, limit: u32) -> Self {
		self.limit = limit;

		self
	}

	pub fn name(mut self, name: &'a str) -> Self {
		self.name = name;

		self
	}

	pub fn args_json_contains(mut self, args_json_contains: &'a str) -> Self {
		self.args_json_contains = Some(args_json_contains);

		self
	}
}
impl<'a> Display for Query<'a> {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(
			f,
			"{{\
				\"query\":\"{{\
					events(\
						limit:{},\
						orderBy:block_id_DESC,\
						where:{{\
							name_eq:\\\"{}\\\"\
							{}\
						}}\
					){{\
						args,\
						block{{height}}\
					}}\
				}}\"\
			}}",
			self.limit,
			self.name,
			if let Some(args_json_contains) = self.args_json_contains {
				format!(",args_jsonContains:\\\"{}\\\"", args_json_contains)
			} else {
				"".into()
			}
		)
	}
}
#[test]
fn query_string_should_work() {
	assert_eq!(
		Query::new("Auctions.BidAccepted")
			.args_json_contains(
				"{{\\\\\\\"bidder\\\\\\\":\\\\\\\"0x1234\\\\\\\",\\\\\\\"paraId\\\\\\\":2000}}",
			)
			.to_string(),
		"{\"query\":\"{events(limit:1,orderBy:block_id_DESC,where:{name_eq:\\\"Auctions.BidAccepted\\\",args_jsonContains:\\\"{{\\\\\\\"bidder\\\\\\\":\\\\\\\"0x1234\\\\\\\",\\\\\\\"paraId\\\\\\\":2000}}\\\"}){args,block{height}}}\"}"
	);
}

impl Hunter {
	pub async fn query<'a, D>(&self, query: Query<'a>) -> Result<D>
	where
		D: DeserializeOwned,
	{
		Ok(self
			.http
			.post(&self.configuration.graphql_endpoint)
			.body(query.to_string())
			.send()
			.await?
			.json()
			.await?)
	}

	pub async fn last_accepted_bid_of(
		&self,
		who: &str,
		para_id: ParaId,
	) -> Result<Option<AcceptedBid>> {
		let mut events = self
			.query::<EResponse<Event<EBidAccepted>>>(
				Query::new("Auctions.BidAccepted").args_json_contains(&format!(
					"{{\\\\\\\"bidder\\\\\\\":\\\\\\\"{who}\\\\\\\",\\\\\\\"paraId\\\\\\\":{para_id}}}",
				)),
			)
			.await?
			.data
			.events;

		if events.is_empty() {
			Ok(None)
		} else {
			let e = events.remove(0);

			Ok(Some(AcceptedBid {
				at: e.block.height,
				amount: e.args.amount.parse().expect("`amount` must be invalid digit"),
				first_slot: e.args.first_slot,
				last_slot: e.args.last_slot,
			}))
		}
	}
}
