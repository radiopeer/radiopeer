use clap::App;
use futures::prelude::*;
use libp2p::{
    floodsub::{self, Floodsub, FloodsubEvent},
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::NetworkBehaviourEventProcess,
    tokio_codec::{FramedRead, LinesCodec},
    tokio_io::{AsyncRead, AsyncWrite},
    NetworkBehaviour, PeerId, Swarm,
};
use secp256k1::rand::thread_rng;
use secp256k1::Secp256k1;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

fn create_home_dir(path: Option<&str>) -> PathBuf {
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

fn create_keys(
    create_new: bool,
    home_path: &PathBuf,
) -> Result<identity::Keypair, Box<std::error::Error>> {
    let mut key_path = PathBuf::from(home_path);
    key_path.push(".peer_key");
    if key_path.exists() && !create_new {
        let f = File::open(key_path)?;
        let mut f = BufReader::new(f);
        let mut key_str = String::new();
        f.read_line(&mut key_str)?;
        let secret_key = secp256k1::SecretKey::from_str(key_str.as_str())?;
        let secp = Secp256k1::new();
        let pub_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);

        println!("SECRET: {:?}", secret_key);

        Ok(identity::Keypair::generate_ed25519())
    } else {
        let mut key_file = File::create(key_path)?;
        let secp = Secp256k1::new();
        let (sec, pubk) = secp.generate_keypair(&mut thread_rng());
        key_file.write_all(&format!("{}", sec).into_bytes())?;

        Ok(identity::Keypair::generate_ed25519())
    }
}

fn main() {
    let matches = App::new("radiopeer")
        .version("0.1.0")
        .author("Spec")
        .args_from_usage(
            "
            --path=[path] 'Sets the home folder'
        ",
        )
        .get_matches();

    let path = matches.value_of("path");
    let home_path = create_home_dir(path);
    // TODO: argument that
    create_keys(false, &home_path);
    println!("Using home path: {}", home_path.display());
}
