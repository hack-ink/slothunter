mod runtime;
pub use runtime::*;

// std
#[cfg(test)] use std::fmt::{Debug, Formatter, Result as FmtResult};
// crates.io
use serde::Serialize;
// slothunter
use crate::hunter::*;

pub type BlockNumber = u32;
pub type AccountId = [u8; 32];
pub type Balance = u128;
pub type ParaId = u32;
pub type SlotRange = (u32, u32);

#[derive(Debug, Serialize)]
pub struct AuctionDetail {
	pub index: u32,
	pub first_lease_period: u32,
	pub ending_period_start_at: BlockNumber,
}
impl AuctionDetail {
	pub fn fmt(&self, now: BlockNumber, end_at: BlockNumber) -> String {
		let remain_blocks = end_at.saturating_sub(now);

		format!(
			"auction(#{}) for leases[#{}, #{}) has been activated for block range[#{}, #{end_at}], remain {remain_blocks} block(s) approximately {}",
			self.index,
			self.first_lease_period,
			self.first_lease_period + C_RANGE_COUNT,
			self.ending_period_start_at,
			util::blocks2time(remain_blocks)
		)
	}
}

#[derive(Debug)]
pub struct Bidder {
	pub who: AccountId,
	pub para_id: ParaId,
	pub reserved: Balance,
	pub existing_deposit: Balance,
	pub last_accepted_bid: Option<AcceptedBid>,
}
impl Bidder {
	pub fn fmt(&self, token: &Token) -> String {
		format!(
			"bidder({}, {}) has bid with extra reservation {} this turn{}",
			array_bytes::bytes2hex("0x", self.who),
			self.para_id,
			token.fmt(self.reserved),
			if self.existing_deposit == 0 {
				"".into()
			} else {
				format!(
					" and found it has an existing lease with deposit {}",
					token.fmt(self.existing_deposit)
				)
			},
		)
	}
}
#[derive(Debug)]
pub struct AcceptedBid {
	pub at: BlockNumber,
	pub amount: Balance,
	pub first_slot: u32,
	pub last_slot: u32,
}
impl AcceptedBid {
	pub fn fmt(&self, token: &Token) -> String {
		format!(
			"{} for lease(s)[#{}, #{}] at block height(#{})",
			token.fmt(self.amount),
			self.first_slot,
			self.last_slot,
			self.at,
		)
	}
}

#[derive(Debug)]
pub struct Winning(pub [Option<Winner>; 36]);
impl Winning {
	pub fn of(s_winning: SWinning) -> Self {
		Self(array_bytes::vec2array_unchecked(
			s_winning
				.iter()
				.cloned()
				.enumerate()
				.map(|(i, w)| w.map(|w| Winner::construct(w, C_SLOT_RANGES[i])))
				.collect(),
		))
	}

	pub fn fmt(&self, token: &Token, first_lease_period: u32) -> Vec<String> {
		self.0
			.iter()
			.filter_map(|w| w.as_ref().map(|w| w.fmt(token, first_lease_period)))
			.collect::<Vec<_>>()
	}

	pub fn result(&self) -> (Vec<Winner>, Balance) {
		fn winner_of(winning: &Winning, leases: &SlotRange) -> (Option<ParaId>, Balance) {
			util::position_in_ranges(leases)
				.and_then(|i| {
					winning.0.get(i).and_then(|w| {
						w.as_ref().map(|w| {
							(Some(w.para_id), w.value * util::leases_length(&w.leases) as Balance)
						})
					})
				})
				.unwrap_or_default()
		}

		let mut winning = [
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
			(Vec::new(), 0),
		];

		assert_eq!(winning.len(), C_RANGE_COUNT as usize);

		(0..C_RANGE_COUNT).for_each(|i| {
			let (para_id, value) = winner_of(self, &(0, i));

			winning[i as usize] = (para_id.map(|p| vec![p]).unwrap_or_default(), value);

			(0..i).for_each(|j| {
				let (para_id, mut value) = winner_of(self, &(j + 1, i));

				value += winning[j as usize].1;

				if value > winning[i as usize].1 {
					winning[i as usize] = (
						if let Some(p) = para_id {
							let mut winners = winning[j as usize].0.clone();

							winners.push(p);

							winners
						} else {
							winning[j as usize].0.clone()
						},
						value,
					);
				}
			});
		});

		winning
			.last()
			.map(|(winners, threshold)| {
				(
					winners
						.iter()
						.map(|p| {
							self.0
								.iter()
								.filter_map(Option::as_ref)
								.find(|w| &w.para_id == p)
								.expect("para id must exist")
						})
						.cloned()
						.collect(),
					*threshold,
				)
			})
			.expect("winners must be some")
	}

	pub fn minimum_bid_to_win(&self, leases: &SlotRange, threshold: Balance) -> Balance {
		let intersecting_leases = self
			.0
			.iter()
			.filter_map(Option::as_ref)
			.filter(|w| !util::ranges_are_intersecting(&w.leases, leases))
			.collect::<Vec<_>>();
		let mut combinations = util::combinations(&intersecting_leases);

		combinations.retain(|c| {
			for i in 0..c.len() {
				for j in i + 1..c.len() {
					if util::ranges_are_intersecting(&c[i].leases, &c[j].leases) {
						return false;
					}
				}
			}

			true
		});

		let leases_length = util::leases_length(leases) as Balance;

		combinations
			.into_iter()
			.map(|c| {
				(threshold
					- c.into_iter()
						.fold(0, |v, w| v + w.value * util::leases_length(&w.leases) as Balance))
					/ leases_length
			})
			.min()
			.unwrap_or(threshold / leases_length)
	}
}
impl Default for Winning {
	fn default() -> Self {
		Self([None; 36])
	}
}
#[derive(Clone, Copy, Serialize)]
#[cfg_attr(not(test), derive(Debug))]
#[cfg_attr(test, derive(PartialEq))]
pub struct Winner {
	#[serde(serialize_with = "util::serialize_account_id")]
	pub who: AccountId,
	pub para_id: ParaId,
	pub leases: SlotRange,
	pub value: Balance,
}
impl Winner {
	pub fn construct(s_winner: SWinner, leases: SlotRange) -> Self {
		Self { who: s_winner.0, para_id: s_winner.1, leases, value: s_winner.2 }
	}

	pub fn fmt(&self, token: &Token, first_lease_period: u32) -> String {
		format!(
			"bidder({}, {}) has won the lease(s)[#{}, #{}] with {}",
			array_bytes::bytes2hex("0x", self.who),
			self.para_id,
			first_lease_period + self.leases.0,
			first_lease_period + self.leases.1,
			token.fmt(self.value),
		)
	}
}
#[cfg(test)]
impl Debug for Winner {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		write!(
			f,
			"Winner {{ para_id: {}, leases: {:?}, value: {} }}",
			self.para_id as u8 as char, self.leases, self.value,
		)
	}
}
#[cfg(test)]
fn winner(para_id: char, leases: SlotRange, value: Balance) -> Winner {
	Winner { who: [0; 32], para_id: para_id as ParaId, leases, value }
}
#[cfg(test)]
fn add_winner(winning: &mut Winning, para_id: char, leases: SlotRange, value: Balance) {
	winning.0[util::position_in_ranges(&leases).unwrap()] = Some(winner(para_id, leases, value));
}
#[test]
fn winning_result_should_work() {
	{
		let mut winning = Winning([None; 36]);

		assert_eq!(winning.result(), (Vec::new(), 0));

		// A: (0, 1) -> 5
		// B: (0, 2) -> 6
		// C: (1, 2) -> 3
		// D: (0, 3) -> 7
		// E: (2, 3) -> 4
		add_winner(&mut winning, 'A', (0, 1), 5);
		add_winner(&mut winning, 'B', (0, 2), 6);
		add_winner(&mut winning, 'C', (1, 2), 3);
		add_winner(&mut winning, 'D', (0, 3), 7);
		add_winner(&mut winning, 'E', (2, 3), 4);

		assert_eq!(winning.result(), (vec![winner('D', (0, 3), 7)], 28));
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (0, 0) -> 5
		// B: (1, 1) -> 6
		// C: (2, 2) -> 7
		// D: (3, 3) -> 8
		// E: (0, 3) -> 3
		add_winner(&mut winning, 'A', (0, 0), 5);
		add_winner(&mut winning, 'B', (1, 1), 6);
		add_winner(&mut winning, 'C', (2, 2), 7);
		add_winner(&mut winning, 'D', (3, 3), 8);
		add_winner(&mut winning, 'E', (0, 3), 3);

		assert_eq!(
			winning.result(),
			(
				vec![
					winner('A', (0, 0), 5),
					winner('B', (1, 1), 6),
					winner('C', (2, 2), 7),
					winner('D', (3, 3), 8),
				],
				26
			),
		);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (0, 0) -> 5
		// B: (1, 1) -> 6
		// C: (2, 2) -> 7
		// D: (3, 3) -> 8
		// E: (0, 3) -> 3
		add_winner(&mut winning, 'A', (0, 7), 10);
		add_winner(&mut winning, 'B', (0, 2), 4);
		add_winner(&mut winning, 'C', (3, 3), 5);
		add_winner(&mut winning, 'D', (1, 7), 11);
		add_winner(&mut winning, 'E', (4, 7), 16);

		assert_eq!(
			winning.result(),
			(vec![winner('B', (0, 2), 4), winner('C', (3, 3), 5), winner('E', (4, 7), 16)], 81),
		);
	}
}
#[test]
fn minimum_bid_to_win_should_work() {
	{
		let mut winning = Winning([None; 36]);

		// A: (0, 3) -> 10
		add_winner(&mut winning, 'A', (0, 3), 10);

		assert_eq!(winning.minimum_bid_to_win(&(1, 2), winning.result().1), 20);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (0, 3) -> 10
		// B: (0, 1) -> 10
		add_winner(&mut winning, 'A', (0, 3), 10);
		add_winner(&mut winning, 'B', (0, 1), 5);

		assert_eq!(winning.minimum_bid_to_win(&(2, 3), winning.result().1), 15);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (0, 3) -> 10
		// B: (0, 1) -> 5
		// C: (0, 2) -> 8
		// D: (1, 2) -> 7
		add_winner(&mut winning, 'A', (0, 3), 10);
		add_winner(&mut winning, 'B', (0, 1), 5);
		add_winner(&mut winning, 'C', (0, 2), 8);
		add_winner(&mut winning, 'D', (1, 2), 7);

		assert_eq!(winning.minimum_bid_to_win(&(0, 0), winning.result().1), 26);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (1, 3) -> 11
		// B: (2, 6) -> 21
		// C: (7, 7) -> 34
		add_winner(&mut winning, 'A', (1, 3), 11);
		add_winner(&mut winning, 'B', (2, 6), 21);
		add_winner(&mut winning, 'C', (7, 7), 34);

		assert_eq!(winning.minimum_bid_to_win(&(0, 7), winning.result().1), 17);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (1, 2) -> 11
		// B: (3, 6) -> 21
		// C: (7, 7) -> 34
		add_winner(&mut winning, 'A', (1, 2), 11);
		add_winner(&mut winning, 'B', (3, 6), 21);
		add_winner(&mut winning, 'C', (7, 7), 34);

		assert_eq!(winning.minimum_bid_to_win(&(0, 7), winning.result().1), 17);
	}

	{
		let mut winning = Winning([None; 36]);

		// A: (0, 3) -> 10
		// B: (0, 1) -> 10
		// C: (2, 2) -> 5
		// D: (1, 3) -> 11
		// E: (3, 3) -> 16
		add_winner(&mut winning, 'A', (0, 3), 10);
		add_winner(&mut winning, 'B', (0, 1), 10);
		add_winner(&mut winning, 'C', (2, 2), 5);
		add_winner(&mut winning, 'D', (1, 3), 11);
		add_winner(&mut winning, 'E', (3, 3), 16);
		add_winner(&mut winning, 'F', (0, 0), 1);

		assert_eq!(winning.minimum_bid_to_win(&(1, 2), winning.result().1), 12);
	}
}
