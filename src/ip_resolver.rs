/// Wrapper to hide the implementation details of external ip address lookup.
/// Could easily be replaced by an http call to a service like icanhazip.com or 
/// whatismyipaddress.com.
///
/// Using opendns.com's myip subdomain just lets us skip the dns step of the
/// http call, which is a meaningless optimization given how infrequently this should
/// be used.
use trust_dns_resolver::Resolver;
use trust_dns_resolver::config::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::io::Result;
use std::vec::Vec;

pub struct IpResolver {
    resolver: Resolver,
}

impl IpResolver {
    pub fn new() -> Result<Self> {

        let ns = NameServerConfigGroup::from_ips_clear(&[
                IpAddr::V4(Ipv4Addr::new(208, 67, 222, 222)),
                IpAddr::V4(Ipv4Addr::new(208, 67, 220, 220)),
                IpAddr::V6(Ipv6Addr::new(2620, 119, 35, 0, 0, 0, 0, 35)),
                IpAddr::V6(Ipv6Addr::new(2620, 119, 53, 0, 0, 0, 0, 53)),
            ], 53);
        let config = ResolverConfig::from_parts(None, vec![], ns);
        let resolver = Resolver::new(config, ResolverOpts::default())?;

        Ok(IpResolver {
            resolver
        })
    }

    /// Returns a vec of external IpAddr for this service
    pub fn lookup_ips(&self) -> Result<Vec<IpAddr>>
    {
        /* Ask a DNS service for our IP address */
        let dns_response = self.resolver.lookup_ip("myip.opendns.com")?;
        Ok(dns_response.iter().collect())
    }
}
