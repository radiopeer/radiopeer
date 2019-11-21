use clap::App;
use futures::prelude::*;
use libp2p::{
    floodsub::{self, Floodsub, FloodsubEvent}, //TODO: replace floodsub by gossipsub or episub
    identity,
    NetworkBehaviour,
    PeerId,
    Swarm,
    mdns::{Mdns, MdnsEvent},
    swarm::NetworkBehaviourEventProcess,
    tokio_codec::{FramedRead, LinesCodec},
    tokio_io::{AsyncRead, AsyncWrite},
};
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

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
    home_path: &PathBuf,
) -> Result<identity::Keypair, Box<dyn std::error::Error>> {
    let mut key_path = PathBuf::from(home_path);
    key_path.push(".peer_key");
    if key_path.exists() {
        let mut f = File::open(key_path)?;
        let mut key_buffer = Vec::new();
        f.read_to_end(&mut key_buffer)?;

        let secret = identity::secp256k1::SecretKey::from_bytes(key_buffer)?;
        let keypair = identity::secp256k1::Keypair::from(secret);

        Ok(identity::Keypair::Secp256k1(keypair))
    } else {
        let mut key_file = File::create(key_path)?;
        let keypair = identity::secp256k1::Keypair::generate();
        let secret = keypair.secret();
        // TODO: use asn1_der to store keys
        // https://github.com/kizzycode/asn1_der
        key_file.write_all(&secret.to_bytes())?;

        Ok(identity::Keypair::Secp256k1(keypair))
    }
}

fn connect(
    local_key: identity::Keypair,
) {
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);
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
    println!("Using home path: {}", home_path.display());
    match create_keys(&home_path){
        Ok(local_key) => connect(local_key),
        Err(error) => println!("{}",error)
    };
}
