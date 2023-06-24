# Test guide
## Unit test
### Preparation
- Install
  - Rust toolchain
### Test
1. Go to the root directory of this repository and run `cargo test`

## Integration test
### Preparation
- Install
  - Rust toolchain
  - Docker
  - Docker Compose
- Download
  - [Polkadot v0.9.43](https://github.com/paritytech/polkadot/releases/download/v0.9.43/polkadot)
  - [Rococo testing chainspec](https://github.com/hack-ink/slothunter/releases/download/v0.1.0/rococo.json.fork-off)
- Addition
  - Move the downloads to the `test/integration/data` directory
  - Ensure that no other programs are using ports `3000`, `8000`, and `9944`
### Basic test
1. Go to the root directory of this repository
2. Run `docker-compose -f test/integration/docker-compose.yml up -d`
3. Run `cargo test --features node-test`
### Advance test
#### Configuration
1. Open `test/integration/rococo.toml`
2. Change the `para-id` if you want to test with different id
3. Change the `leases` if you want to test with different lease(s)
4. Change to `watch-only` to `true` if you want to test without the bid components
5. Change the `type` to test different bidding type
6. Adjust the `upper-limit` and `increment` to test more situation if you want
7. Add some `webhooks` to test the notification webhook component
  - You can use [webhook.site](https://webhook.site), the result should be listed on the left of the website
  - You can use any other webhook listener
8. Edit the `mail` to test the notification mail component
  - Add some `receivers`
  - Add a `sender`
    - Recommending using Gmail for testing
    - Add `username`, e.g. example@gmail.com
    - Add `password`, e.g. [app password](https://support.google.com/accounts/answer/185833?hl=en)
#### Test
1. Go to the root directory of this repository
2. Run
  - `docker-compose -f test/integration/docker-compose.yml down && rm -rf test/integration/data/db && docker-compose -f test/integration/docker-compose.yml up -d` if you have run the basic test before
  - `docker-compose -f test/integration/docker-compose.yml up -d` if you haven't run the basic test before
3. Run `cargo run -- -c test/integration/rococo.toml`
4. Open browser and navigate to [Polkadot/Substrate Portal](https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:9944#/explorer)
5. *Optional* for crowdloan
  1. Navigate to `Network tab -> Parachains tab -> Crowdloan tab -> Add fund button`
  2. Set `crowdfund cap` recommend `1000 UNIT`
  3. Set `ending block` recommend `1000`
  4. Set `periods` recommend `(0, 0)`
6. Navigate to `Developer tab -> Sudo tab -> auctions module -> newAuction call`
7. Submit the new auction call, remember to uncheck the `use a proxy for this call` in the pop-up window
8. A notification should appear in the webhook and mail to indicate the start of an auction
9. Additionally, a notification regarding the initial bid should be included in both the webhook and mail
10. Use `Charlie` and `Dave` to bid for `2001` and `2002` and check the results
  - E.G.
    - `Charlie` bid `5 UNIT` for `(0, 0)`. Slothunter should bid `7 UNIT` for `(0, 0)` with the default configurations
    - `Dave` bid `1 UNIT` for `(1, 1)` first, and then `Charlie` bid `5 UNIT` for `(0, 1)`. Slothunter should bid `11 UNIT` for `(0, 0)` with the default configurations
11. Wait until the auction ends, you should receive a notification via webhook and mail indicating that the auction has ended

## Addition
- Default real account is `//Alice`
- Default proxy accounts are `//Alice/stash` with `ProxyType::All` and `//Bob` with `ProxyType::Auction`
- Default parachains and their owners are `(Alice, 2000)`, `(Charlie, 2001)` and `(Dave, 2002)`

All of this data is pre-set and was built from Polkadot's latest Rococo-local chain with the assistance of [Subalfred fork-off](https://subalfred.hack.ink/user/cli/state.html#command-state-fork-off).
