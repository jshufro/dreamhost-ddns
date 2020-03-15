# dreamhost-ddns

Updates dreamhost's API to add an A/AAAA record for your local network's external IP V4/V6 address (respectively) to your DNS settings. Written in Rust so I could learn Rust.

## Getting Started
### From the source
1. [Get Rust](https://www.rust-lang.org/learn/get-started)
2. Build the project using `cargo build`.
3. Run the project using `cargo run -- --key [YOUR-DREAMHOST-API-KEY] --hostname [mysubdomain.mysite.com]`
4. Use `cargo install .` from the project directory to create to install.
5. Optionally run under cron using `--once` (recommended to run no more than hourly).

### Just install and use
TODO: figure out how to run under systemd and deploy on crates.io

### In Docker
TODO: Link to dockerfile repo

## Authors
* **Jacob Shufro** - Trying to learn a new systems language.

## License
This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details
