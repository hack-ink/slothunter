// crates.io
use parity_scale_codec::{Decode, Encode};
use regex::Regex;
use reqwest::{
	header::{HeaderMap, CONTENT_TYPE},
	Client, ClientBuilder,
};
use scale_value::Composite;
use serde::ser::Serializer;
use sp_runtime::{traits::AccountIdConversion, TypeId};
use subxt::{
	dynamic::{self, Value},
	tx::Payload,
};
// slothunter
use crate::hunter::*;

const E_REGEX_MUST_BE_VALID: &str = "regex must be valid";

pub fn check_http_uri(uri: &str) -> bool {
	Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").expect(E_REGEX_MUST_BE_VALID).is_match(uri)
}
pub fn check_ws_uri(uri: &str) -> bool {
	Regex::new(r"^wss?://[^\s/$.?#].[^\s]*$").expect(E_REGEX_MUST_BE_VALID).is_match(uri)
}
pub fn check_smtp_uri(uri: &str) -> bool {
	Regex::new(r"^[a-zA-Z0-9.-]+(:\d+)?$").expect(E_REGEX_MUST_BE_VALID).is_match(uri)
}
#[test]
fn check_uri_should_work() {
	assert!(check_http_uri("http://localhost:8080/path/to/file.html"));
	assert!(check_http_uri("https://www.example.com"));
	assert!(!check_http_uri("invalid url"));

	assert!(check_ws_uri("ws://localhost:8080/socket"));
	assert!(check_ws_uri("wss://localhost:8080/socket"));
	assert!(!check_ws_uri("invalid url"));

	assert!(check_smtp_uri("smtp.example.com"));
	assert!(check_smtp_uri("smtp.example.com:587"));
	assert!(!check_smtp_uri("invalid url"));
}

pub fn http_json_client() -> Client {
	ClientBuilder::new()
		.default_headers(HeaderMap::from_iter([(
			CONTENT_TYPE,
			"application/json".parse().unwrap(),
		)]))
		.build()
		.unwrap()
}

pub fn crowdloan_id_of(index: u32) -> AccountId {
	#[derive(Encode, Decode)]
	pub struct CrowdloanId(pub [u8; 8]);
	impl TypeId for CrowdloanId {
		const TYPE_ID: [u8; 4] = *b"modl";
	}

	CrowdloanId(*b"py/cfund").into_sub_account_truncating(index)
}
#[test]
fn crowdloan_id_of_should_work() {
	assert_eq!(
		crowdloan_id_of(0),
		array_bytes::hex2array_unchecked(
			"0x6d6f646c70792f6366756e640000000000000000000000000000000000000000"
		)
	);
}

pub fn range_of(first_lease_period: u32) -> SlotRange {
	(first_lease_period, first_lease_period + C_RANGE_COUNT - 1)
}
#[test]
fn range_of_should_work() {
	assert_eq!(range_of(10), (10, 17));
}

pub fn check_leases(a: &SlotRange, b: &SlotRange) -> bool {
	(a.0 <= b.0) && (a.1 >= b.1)
}
#[test]
fn check_leases_should_work() {
	// Test when a completely overlaps b.
	let a = (5, 10);
	let b = (7, 8);
	assert!(check_leases(&a, &b));

	// Test when b completely overlaps a.
	let a = (5, 10);
	let b = (3, 12);
	assert!(!check_leases(&a, &b));

	// Test when a starts before b and ends before b.
	let a = (5, 10);
	let b = (7, 12);
	assert!(!check_leases(&a, &b));

	// Test when a starts before b and ends after b.
	let a = (5, 15);
	let b = (7, 12);
	assert!(check_leases(&a, &b));

	// Test when a starts after b and ends after b.
	let a = (7, 12);
	let b = (5, 10);
	assert!(!check_leases(&a, &b));

	// Test when a starts after b and ends before b.
	let a = (7, 8);
	let b = (5, 10);
	assert!(!check_leases(&a, &b));

	// Test when a and b are the same.
	let a = (5, 10);
	let b = (5, 10);
	assert!(check_leases(&a, &b));

	// Test when a and b do not overlap.
	let a = (5, 10);
	let b = (12, 15);
	assert!(!check_leases(&a, &b));
}

pub fn winning_offset_of(
	block_number: BlockNumber,
	ending_period_start_at: BlockNumber,
	sample_length: BlockNumber,
) -> u32 {
	if let Some(ending_duration) = block_number.checked_sub(ending_period_start_at) {
		ending_duration / sample_length
	} else {
		0
	}
}
#[test]
fn winning_offset_of_should_work() {
	const TWO_MINUTES: BlockNumber = 2 * 60 / 6;

	assert_eq!(winning_offset_of(4, 5, TWO_MINUTES), 0);
	assert_eq!(winning_offset_of(24, 5, TWO_MINUTES), 0);
	assert_eq!(winning_offset_of(25, 5, TWO_MINUTES), 1);
	assert_eq!(winning_offset_of(45, 5, TWO_MINUTES), 2);
	assert_eq!(winning_offset_of(65, 5, TWO_MINUTES), 3);
	assert_eq!(winning_offset_of(123456, 789, TWO_MINUTES), 6133);
}

pub fn position_in_ranges(slot_range: &SlotRange) -> Option<usize> {
	C_SLOT_RANGES.iter().position(|s| s == slot_range)
}
#[test]
fn position_in_ranges_should_work() {
	assert_eq!(position_in_ranges(&(0, 0)), Some(0));
	assert_eq!(position_in_ranges(&(0, 1)), Some(1));
	assert_eq!(position_in_ranges(&(0, 2)), Some(2));
	assert_eq!(position_in_ranges(&(0, 3)), Some(3));
	assert_eq!(position_in_ranges(&(0, 4)), Some(4));
	assert_eq!(position_in_ranges(&(0, 5)), Some(5));
	assert_eq!(position_in_ranges(&(0, 6)), Some(6));
	assert_eq!(position_in_ranges(&(0, 7)), Some(7));
	assert_eq!(position_in_ranges(&(0, 8)), None);
	assert_eq!(position_in_ranges(&(1, 1)), Some(8));
	assert_eq!(position_in_ranges(&(1, 2)), Some(9));
	assert_eq!(position_in_ranges(&(1, 3)), Some(10));
	assert_eq!(position_in_ranges(&(1, 4)), Some(11));
	assert_eq!(position_in_ranges(&(1, 5)), Some(12));
	assert_eq!(position_in_ranges(&(1, 6)), Some(13));
	assert_eq!(position_in_ranges(&(1, 7)), Some(14));
	assert_eq!(position_in_ranges(&(2, 2)), Some(15));
	assert_eq!(position_in_ranges(&(2, 3)), Some(16));
	assert_eq!(position_in_ranges(&(2, 4)), Some(17));
	assert_eq!(position_in_ranges(&(2, 5)), Some(18));
	assert_eq!(position_in_ranges(&(2, 6)), Some(19));
	assert_eq!(position_in_ranges(&(2, 7)), Some(20));
	assert_eq!(position_in_ranges(&(3, 3)), Some(21));
	assert_eq!(position_in_ranges(&(3, 4)), Some(22));
	assert_eq!(position_in_ranges(&(3, 5)), Some(23));
	assert_eq!(position_in_ranges(&(3, 6)), Some(24));
	assert_eq!(position_in_ranges(&(3, 7)), Some(25));
	assert_eq!(position_in_ranges(&(4, 4)), Some(26));
	assert_eq!(position_in_ranges(&(4, 5)), Some(27));
	assert_eq!(position_in_ranges(&(4, 6)), Some(28));
	assert_eq!(position_in_ranges(&(4, 7)), Some(29));
	assert_eq!(position_in_ranges(&(5, 5)), Some(30));
	assert_eq!(position_in_ranges(&(5, 6)), Some(31));
	assert_eq!(position_in_ranges(&(5, 7)), Some(32));
	assert_eq!(position_in_ranges(&(6, 6)), Some(33));
	assert_eq!(position_in_ranges(&(6, 7)), Some(34));
	assert_eq!(position_in_ranges(&(7, 7)), Some(35));
	assert_eq!(position_in_ranges(&(8, 7)), None);
}

pub fn leases_length(leases: &SlotRange) -> u32 {
	leases.1 - leases.0 + 1
}

pub fn ranges_are_intersecting(a: &SlotRange, b: &SlotRange) -> bool {
	a.0 <= b.1 && a.1 >= b.0
}
#[test]
fn ranges_are_intersecting_should_work() {
	// w = [1, 2], l = [2, 3]
	assert!(ranges_are_intersecting(&(1, 2), &(2, 3)));
	// w = [3, 4], l = [2, 3]
	assert!(ranges_are_intersecting(&(3, 4), &(2, 3)));
	// w = [1, 4], l = [2, 3]
	assert!(ranges_are_intersecting(&(1, 4), &(2, 3)));
}

pub fn combinations<T>(elements: &[T]) -> Vec<Vec<&T>> {
	let n = elements.len();
	let mut combinations = Vec::new();

	(1..=(1 << n) - 1).for_each(|i| {
		let mut combination = Vec::new();

		(0..n).for_each(|j| {
			if i & (1 << j) != 0 {
				combination.push(&elements[j]);
			}
		});

		combinations.push(combination);
	});

	combinations
}
#[test]
fn combinations_should_work() {
	assert_eq!(
		combinations(&['A', 'B', 'C', 'D']),
		vec![
			vec![&'A'],
			vec![&'B'],
			vec![&'A', &'B'],
			vec![&'C'],
			vec![&'A', &'C'],
			vec![&'B', &'C'],
			vec![&'A', &'B', &'C'],
			vec![&'D'],
			vec![&'A', &'D'],
			vec![&'B', &'D'],
			vec![&'A', &'B', &'D'],
			vec![&'C', &'D'],
			vec![&'A', &'C', &'D'],
			vec![&'B', &'C', &'D'],
			vec![&'A', &'B', &'C', &'D']
		]
	);
}

pub fn blocks2time(blocks_count: BlockNumber) -> String {
	const HOUR: u64 = 60 * 60;
	const DAY: u64 = HOUR * 24;
	const BLOCK_TIME: u64 = 6;

	let seconds = blocks_count as u64 * BLOCK_TIME;
	let d = seconds / DAY;
	let h = (seconds % DAY) / HOUR;
	let m = (seconds % HOUR) / 60;
	let s = seconds % 60;

	format!("{d}d:{h}h:{m}m:{s}s")
}
#[test]
fn blocks2time_should_work() {
	assert_eq!(blocks2time(0), "0d:0h:0m:0s");
	assert_eq!(blocks2time(1), "0d:0h:0m:6s");
	assert_eq!(blocks2time(10), "0d:0h:1m:0s");
	assert_eq!(blocks2time(600), "0d:1h:0m:0s");
	assert_eq!(blocks2time(14400), "1d:0h:0m:0s");
	assert_eq!(blocks2time(123456), "8d:13h:45m:36s");
}

pub fn serialize_account_id<S>(account_id: &AccountId, serializer: S) -> StdResult<S::Ok, S::Error>
where
	S: Serializer,
{
	serializer.serialize_str(&array_bytes::bytes2hex("0x", account_id))
}

pub fn proxy_of(real: &AccountId, call: Payload<Composite<()>>) -> Payload<Composite<()>> {
	dynamic::tx(
		"Proxy",
		"proxy",
		vec![
			Value::unnamed_variant("Id", [Value::from_bytes(real)]),
			Value::unnamed_variant("None", []),
			call.into_value(),
		],
	)
}
