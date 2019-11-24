use libp2p::identity;
use libp2p::{multiaddr, Multiaddr, PeerId};
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

pub fn create_home_dir(path: Option<&str>) -> PathBuf {
  match path {
    Some(p) => match std::fs::create_dir(p) {
      Ok(_) => PathBuf::from(p),
      Err(err) => match err.kind() {
        std::io::ErrorKind::AlreadyExists => PathBuf::from(p),
        _ => panic!("Error trying to create dir: {}", err),
      },
    },
    None => match dirs::home_dir().as_mut() {
      Some(home_dir) => {
        home_dir.push(".radiopeer");
        create_home_dir(home_dir.to_str())
      }
      None => panic!("Cannot get home folder!"),
    },
  }
}

pub fn create_keys(home_path: &PathBuf) -> Result<identity::Keypair, Box<dyn std::error::Error>> {
  let mut key_path = PathBuf::from(home_path);
  key_path.push(".peer_key");
  let keypair;
  if key_path.exists() {
    let mut f = File::open(key_path)?;
    let mut key_buffer = Vec::new();
    f.read_to_end(&mut key_buffer)?;
    let secret = identity::secp256k1::SecretKey::from_bytes(key_buffer)?;
    keypair = identity::secp256k1::Keypair::from(secret);
  } else {
    let mut key_file = File::create(key_path)?;
    keypair = identity::secp256k1::Keypair::generate();
    let secret = keypair.secret();
    // TODO: use asn1_der to store keys
    // https://github.com/kizzycode/asn1_der
    key_file.write_all(&secret.to_bytes())?;
  }
  Ok(identity::Keypair::Secp256k1(keypair))
}
