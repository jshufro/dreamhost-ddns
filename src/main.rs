extern crate trust_dns_resolver;
extern crate curl;
extern crate serde_json;
extern crate clap;
extern crate syslog;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use clap::Clap;
use std::{thread, time};
use std::cmp::min;
use libc::getpid;
use syslog::{Facility, Formatter3164, BasicLogger};
use log::LevelFilter;

mod ip_resolver;
use ip_resolver::IpResolver;

mod dreamhost;
use dreamhost::Dreamhost;
use dreamhost::Record;

lazy_static!{
    static ref OPTS: Opts = Opts::parse();
}

#[derive(Clap)]
struct Opts {
    /// Sets the hostname to use for DDNS on Dreamhost.
    #[clap(short = "h", long = "hostname")]
    hostname: String,

    /// Sets the API Key to use, from dreamhost's webpanel.
    /// Visit https://panel.dreamhost.com/?tree=home.api to get one.
    #[clap(short = "k", long = "key")]
    key: String,

    /// Verbosity. Only errors are logged by default.
    #[clap(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: i32,

    /// Minimum seconds to wait between refreshes.
    #[clap(short = "m", long = "min-sleep", default_value = "40")]
    min_sleep: u32,

    /// Maximum seconds to wati between refreshes.
    #[clap(short = "M", long = "max-sleep", default_value = "1800")]
    max_sleep: u32,
}

fn heartbeat(resolver : &IpResolver, dreamhost: &mut Dreamhost) -> bool {

    let home_ip_addrs = match resolver.lookup_ips() {
        Ok(ips) => ips,
        Err(error) => {
            error!("Error resolving home IP: {}", error);
            return false;
        }
    };

    let mut home_ips : Vec<Record> = home_ip_addrs.iter().map(Record::new).collect();

    if home_ips.is_empty() {
        error!("Got 0 ip addresses from dns service");
        return false;
    }

    /* Make a request to the dreamhost API */
    let mut dh_ips = match dreamhost.list() {
        Ok(ips) => ips,
        Err(error) => {
            error!("Error querying dreamhost list api: {}", error);
            return false;
        }
    };

    /* Dreamhost allows any number of A or AAAA records for ipv4 and ipv6 respectively.
     * First, remove ips from both lists when they match.
     */
    home_ips.retain(|home_ip| {
        let mut deleted = false;
        dh_ips.retain(|dh_ip| {
            if dh_ip == home_ip {
                deleted = true;
                return false;
            }

            true
        });
        !deleted
    });

    if dh_ips.is_empty() && home_ips.is_empty() {
        // If there was a match for every element in both arrays, we're already up to date.
        info!("Dreamhost was found to be up-to-date.");
        return true;
    }

    /* Next, delete any records from dreamhost that remain.
     * Any record that matched a home ip was already removed from the array,
     * so dh_ips now contains only records that must be removed.
     */
    for i in &dh_ips {
        match dreamhost.remove(i) {
            Ok(_) => info!("Removed ip from dreamhost: {}", i.value),
            Err(e) => error!("Error removing record {}: {}. Continuing.", i.value, e),
        }
    }

    /* Finally, if any ips remain in home_ips, add a DNS record to dreamhost. */
    for i in home_ips {
        match dreamhost.add(&i) {
            Ok(_) => info!("Added IP {} to dreamhost dns", i.value),
            Err(e) => {
                error!("Error adding new IP to dreamhost: {}. Will hopefully be added next pass.", e);
                return false;
            },
        }
    }
    
    true
}

fn update_timer(success : bool, last_s : u32) -> u32 {
    let min_sleep_s : u32 = OPTS.min_sleep;
    let max_sleep_s : u32 = OPTS.max_sleep;

    if success {
        /* If the heartbeat was successful, reset to regular intervals */
        return min_sleep_s;
    }

    /* If the last run fails, add a small delay to back-off.
     * Do not let this delay exceed maximum. */
    min(last_s + min_sleep_s, max_sleep_s)
}

fn setup_logging() {
    let formatter = Formatter3164 {
        facility: Facility::LOG_USER,
        hostname: None,
        process: "dreamhost-ddns".into(),
        pid: unsafe {getpid()},
    };

    let level = match OPTS.verbose {
        0 => LevelFilter::Error,
        _ => LevelFilter::Trace,
    };

    let logger = syslog::unix(formatter).expect("Couldn't connect to syslog");
    log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
        .map(|()| log::set_max_level(level)).unwrap();
}

fn main() {
    setup_logging();

    /* Set up a resolver */
    let resolver = ip_resolver::IpResolver::new().unwrap();
    /* Set up access to the dreamhost api */
    let mut dreamhost = dreamhost::Dreamhost::new(OPTS.key.clone(), OPTS.hostname.clone()).unwrap();

    /* Poll the resolver and update the IP */
    let mut sleep_s:u32 = 0;
    loop {
        /* Try to update dreamhost */
        let succeeded : bool = heartbeat(&resolver, &mut dreamhost);

        /* Determine how long to wait based on whether or not the update succeeded */
        sleep_s = update_timer(succeeded, sleep_s);

        /* Delay the subsequent attempt */
        thread::sleep(time::Duration::from_secs(sleep_s.into()));
    }        
}
