//! A bot for Polkadot parachain auction.

mod hunter;
use hunter::*;

mod primitive;

mod prelude {
	pub use std::result::Result as StdResult;

	pub use anyhow::Result;

	pub use crate::primitive::*;
}

// std
use std::path::PathBuf;
// crates.io
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("CARGO_PKG_VERSION"),
		"-",
		env!("VERGEN_GIT_SHA"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	about,
	rename_all = "kebab",
)]
struct Cli {
	/// Path to the configuration toml's file or folder.
	///
	/// If a file is provided, it will be loaded as the configuration TOML.
	/// Otherwise, Slothunter will search for the config.toml file in the specified folder.
	///
	/// Default paths are:
	///   Linux:   /home/alice/.config/slothunter
	///   Windows: C:\Users\Alice\AppData\Roaming\slothunter
	///   MacOS:   /Users/Alice/Library/Application Support/slothunter
	#[arg(long, short, value_name = "PATH", verbatim_doc_comment)]
	configuration: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
	color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
	tracing_subscriber::fmt::init();

	let Cli { configuration } = Cli::parse();

	Hunter::from_configuration(ConfigurationToml::load(configuration)?.try_into_configuration()?)
		.await?
		.start()
		.await?;

	Ok(())
}
