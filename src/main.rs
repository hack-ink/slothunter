//! A bot for Polkadot parachain auction.

#![deny(missing_docs)]

mod primitive;

// crates.io
use anyhow::Result;
use reqwest::Client;
// slothunter
use primitive::event::*;

fn query(limit: u8, name: &str) -> String {
	format!(
		"{{\
		\"query\":\"{{\
			events(\
				limit:{limit},\
				orderBy:block_id_DESC,\
				where:{{\
					name_eq:\\\"{name}\\\"\
				}}\
			){{\
				args,\
				block{{height}}\
			}}\
		}}\"\
	}}"
	)
}

#[tokio::main]
async fn main() -> Result<()> {
	let resp = Client::new()
		.post("https://kusama.explorer.subsquid.io/graphql")
		.header("content-type", "application/json")
		.body(query(1, "Auctions.AuctionStarted"))
		.send()
		.await?
		.json::<EResponse<Event<EAuctionStarted>>>()
		.await?;

	println!("{resp:#?}");

	Ok(())
}
