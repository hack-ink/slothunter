<div align="center">

# Slothunter
### A bot for Polkadot parachain auction.

[![License](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Checks](https://github.com/hack-ink/slothunter/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/hack-ink/slothunter/actions/workflows/checks.yml)
[![Release](https://github.com/hack-ink/slothunter/actions/workflows/release.yml/badge.svg)](https://github.com/hack-ink/slothunter/actions/workflows/release.yml)
[![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/hack-ink/slothunter)](https://github.com/hack-ink/slothunter/tags)
[![GitHub code lines](https://tokei.rs/b1/github/hack-ink/slothunter)](https://github.com/hack-ink/slothunter)
[![GitHub last commit](https://img.shields.io/github/last-commit/hack-ink/slothunter?color=red&style=plastic)](https://github.com/hack-ink/slothunter)

</div>

## Usage
```sh
# Use `--help` flag to get more information.
slothunter --help
```

### Configuration
To create a template configuration file for the program, run it directly for the first time and use the command below.
After that, press `CTRL-C`.
```sh
slothunter
# Or
slothunter -c config.toml
```

If a path is provided, it will create the configuration file at that location.
Otherwise, Slothunter will create the configuration file at the default path.
The default paths are:
```
Linux:   /home/alice/.config/slothunter
Windows: C:\Users\Alice\AppData\Roaming\slothunter
MacOS:   /Users/Alice/Library/Application Support/slothunter
```

Open the configuration file and edit the items. The file contains highly detailed documentation for each item.

### Addition
For more details, please refer to [guide.md](test/guide.md).
