use libp2p::{multiaddr, Multiaddr, PeerId};
use structopt::{
  clap::{arg_enum, App, AppSettings, Arg, SubCommand},
  StructOpt,
};

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "radiopeer", about = "P2P radio")]
pub struct Params {
  #[structopt(long = "port", value_name = "PORT")]
  pub port: Option<u16>,
  #[structopt(long = "path", value_name = "PATH")]
  pub path: Option<String>,
  #[structopt(long = "bootnodes", value_name = "URL")]
  pub bootnodes: Vec<String>,
  #[structopt(long = "nodename", value_name = "NAME")]
  pub nodename: Option<String>,
}

use std::fmt;
#[derive(Debug)]
pub enum ParseErr {
  /// Error while parsing the multiaddress.
  MultiaddrParse(multiaddr::Error),
  /// Multihash of the peer ID is invalid.
  InvalidPeerId,
  /// The peer ID is missing from the address.
  PeerIdMissing,
}

impl fmt::Display for ParseErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ParseErr::MultiaddrParse(err) => write!(f, "{}", err),
      ParseErr::InvalidPeerId => write!(f, "Peer id at the end of the address is invalid"),
      ParseErr::PeerIdMissing => write!(f, "Peer id is missing from the address"),
    }
  }
}

impl std::error::Error for ParseErr {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      ParseErr::MultiaddrParse(err) => Some(err),
      ParseErr::InvalidPeerId => None,
      ParseErr::PeerIdMissing => None,
    }
  }
}

impl From<multiaddr::Error> for ParseErr {
  fn from(err: multiaddr::Error) -> ParseErr {
    ParseErr::MultiaddrParse(err)
  }
}

pub fn parse_str_addr(addr_str: &str) -> Result<(PeerId, Multiaddr), ParseErr> {
  let addr: Multiaddr = addr_str.parse()?;
  parse_addr(addr)
}

pub fn parse_addr(mut addr: Multiaddr) -> Result<(PeerId, Multiaddr), ParseErr> {
  let who = match addr.pop() {
    Some(multiaddr::Protocol::P2p(key)) => {
      PeerId::from_multihash(key).map_err(|_| ParseErr::InvalidPeerId)?
    }
    _ => return Err(ParseErr::PeerIdMissing),
  };

  Ok((who, addr))
}
