/// Provides a wrapper around http api access for Dreamhost's DNS APIs.
/// Abstracts all http logic as well as json parsing from the main program.

const API_HOST : &str = "api.dreamhost.com";
const API_LIST_CMD : &str = "dns-list_records";
const API_REMOVE_CMD : &str = "dns-remove_record";
const API_ADD_CMD : &str = "dns-add_record";
const API_SCHEME : &str = "https";

use curl::easy::Easy;
use std::string::String;
use std::io::{Result, Error, ErrorKind};
use std::fmt::{Display, Formatter};
use serde_json::Value;
use std::net::IpAddr;
use std::str::FromStr;

#[derive(PartialEq, Eq)]
enum RecordKind {
    A,
    AAAA,
}

impl Display for RecordKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            RecordKind::A => {
                write!(f, "A")?;
            }
            RecordKind::AAAA => {
                write!(f, "AAAA")?;
            }
        }

        Ok(())
    }
}

impl FromStr for RecordKind {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Error> {
        match s {
            "A" => Ok(Self::A),
            "AAAA" => Ok(Self::AAAA),
            _ => Err(Error::new(ErrorKind::InvalidData, "Unmatched RecordKind")),
        }
    }
}

pub struct Record {
    /// Parsed value
    pub value: IpAddr,
    /// A record or AAAA
    r_type: RecordKind,
    /// Annoyingly, dreamhost can't match abbreviated ipv6. It has to be string-for-string match to delete.
    svalue: String,
}

impl Record {
    pub fn new(value: &IpAddr) -> Self {
        let r_type = match value {
            IpAddr::V6(_) => RecordKind::AAAA,
            IpAddr::V4(_) => RecordKind::A,
        };

        Record {
            value: *value,
            r_type,
            svalue: String::new(),
        }
    }
}

impl std::cmp::PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.r_type == other.r_type && self.value == other.value
    }
}

impl std::cmp::Eq for Record {}

pub struct Dreamhost {
    easy: Easy,
    key: String,
    ddns_host: String,
}

impl Dreamhost {
    fn execute(&mut self) -> Result<Value> {
        let mut buf = Vec::new();
        {
            let mut transfer = self.easy.transfer();
            transfer.write_function(|new_data| {
                let l = new_data.len();
                buf.extend_from_slice(new_data);
                Ok(l)
            })?;

            transfer.perform()?;
        }

        let data = match std::str::from_utf8(&buf) {
            Ok(data) => data,
            Err(error) => return Err(Error::new(ErrorKind::Other, error.to_string())),
        };

        Ok(serde_json::from_str(data)?)
    }

    pub fn new(key: String, ddns_host: String) -> Result<Self> {

        Ok(Dreamhost {
            easy: Easy::new(),
            key,
            ddns_host,
        })
    }

    /// Adds a record to the dreamhost API.
    pub fn add(&mut self, r: &Record) -> Result<()> {
        self.easy.url(&format!("{}://{}/?cmd={}&key={}&type={}&value={}&record={}&format=json",
                    API_SCHEME,
                    API_HOST,
                    API_ADD_CMD,
                    self.key,
                    r.r_type,
                    r.value,
                    self.ddns_host))?;

        let j = self.execute()?;

        if j["result"] != "success" {
            error!("Error: {}", j["data"]);
            return Err(Error::new(ErrorKind::Other, "Non-success result adding to dreamhost API"));
        }
        Ok(())
    }

    /// Removes a record from the dreamhost API. Parameter must be a record obtained with list().
    pub fn remove(&mut self, r: &Record) -> Result<()> {
        assert!(!r.svalue.is_empty(), "Only Records returned by list command can be removed.");
        self.easy.url(&format!("{}://{}/?cmd={}&key={}&type={}&value={}&record={}&format=json",
                    API_SCHEME,
                    API_HOST,
                    API_REMOVE_CMD,
                    self.key,
                    r.r_type,
                    r.svalue,
                    self.ddns_host))?;

        let j = self.execute()?;

        if j["result"] != "success" {
            error!("Error: {}", j["data"]);
            return Err(Error::new(ErrorKind::Other, "Non-success result deleting from dreamhost API"));
        }
        Ok(())
    }

    /// Return a vector of A or AAAA records that match our ddns hostname.
    pub fn list(&mut self) -> Result<Vec<Record>> {
        self.easy.url(&format!("{}://{}/?cmd={}&key={}&format=json", 
                    API_SCHEME,
                    API_HOST,
                    API_LIST_CMD,
                    self.key))?;

        let j : Value = self.execute()?;

        if j["result"] != "success" {
            error!("Error: {}", j["data"]);
            return Err(Error::new(ErrorKind::Other, "Non-success result reading from dreamhost API"));
        }

        let entries = match &j["data"] {
            Value::Array(entries) => entries,
            _ => {
                return Err(Error::new(ErrorKind::InvalidData, "'data' field wasn't array-type"));
            }
        };

        Ok(entries.iter().filter_map(|entry| {//for entry in entries {
            if entry["record"] != self.ddns_host ||
               (entry["type"] != "A" && entry["type"] != "AAAA") {
                   return None;
            }
            let ip = match &entry["value"] {
                Value::String(_ip) => _ip,
                _ => {
                    error!("Encountered entry value that wasn't a string. Ignoring.");
                    return None;
                },
            };

            let r_type_s = match entry["type"].as_str() {
                Some(s) => s,
                None => {
                    error!("Encountered entry type that wasn't a string. Ignoring.");
                    return None;
                },
            };
            
            let r_type = match RecordKind::from_str(r_type_s) {
                Ok(v) => v,
                Err(e) => {
                    error!("Couldn't parse entry type into string: {}. Ignoring.", e);
                    return None;
                }
            };

            let value = match IpAddr::from_str(ip) {
                Ok(v) => v,
                Err(e) => {
                    error!("Found an {} record that couldn't be parsed into an IP. \
                        Ignoring: {}. Error: {}.", r_type, ip, e);
                    return None;
                },
            };

            let svalue = match entry["value"].as_str() {
                Some(s) => s,
                None => {
                    error!("Couldn't convert json string value to &str, skipping record");
                    return None;
                },
            };

            Some(Record {
                value,
                r_type,
                svalue: String::from(svalue),
            })
        }).collect::<Vec<Record>>())
    }
}
