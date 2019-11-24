//! A basic example demonstrating some core APIs and concepts of libp2p.
//!
//! In the first terminal window, run:
//!
//! ```sh
//! cargo run
//! ```
//!
//! It will print the PeerId and the listening address, e.g. `Listening on
//! "/ip4/0.0.0.0/tcp/24915"`
//!
//! In the second terminal window, start a new instance of the example with:
//!
//! ```sh
//! cargo run - /ip4/127.0.0.1/tcp/24915
//! ```
//!
use clap::App;
use futures::prelude::*;
use libp2p::{
    core::PeerId,
    identity,
    tokio_codec::{FramedRead, LinesCodec},
    Swarm,
};
use libp2p::{multiaddr, Multiaddr};
use log::{error, info, warn};
use radiopeer::behaviour::Behaviour;
use radiopeer::params::*;
use radiopeer::utils::*;
use structopt::StructOpt;

fn main() {
    let opt = Params::from_args();
    let path = opt.path;
    let port = opt.port.unwrap_or(0);

    let home_path = create_home_dir(path.as_ref().map(String::as_str));
    // // TODO: argument that
    println!("Using home path: {}", home_path.display());
    let local_key = create_keys(&home_path).unwrap();
    let local_public = local_key.public();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {}", local_peer_id);
    // Create a transport.
    let transport = libp2p::build_development_transport(local_key);
    let mut swarm = {
        let user_agent = format!(
            "{} ({})",
            "radiopeer",
            opt.nodename.unwrap_or("robot".to_owned())
        );
        let behaviour = Behaviour::new(user_agent, local_public.clone());
        // behaviour.kademlia.bootstrap();
        Swarm::new(transport, behaviour, local_peer_id)
    };
    // Read full lines from stdin
    let stdin = tokio_stdin_stdout::stdin(0);
    let mut framed_stdin = FramedRead::new(stdin, LinesCodec::new());
    let addr = format!("/ip4/0.0.0.0/tcp/{}", port);

    // Format: /ip4/<ip>/tcp/<port>/p2p/<hash>
    for bootnode in opt.bootnodes {
        match parse_str_addr(bootnode.as_str()) {
            Ok((peer_id, addr)) => {
                println!("Connecting to bootnode: {} {}", addr, peer_id);
                swarm.add_self_reported_address(&peer_id, addr);
            }
            Err(_) => panic!("Not a valid bootnode address: {}", bootnode),
        }
    }

    Swarm::listen_on(&mut swarm, addr.parse().unwrap()).unwrap();
    // match port {
    //     Some(port) => {
    //         // let po = ;
    //         // let addr = "/ip4/0.0.0.0/tcp/".to_string() + port.parse::<u16>().unwrap().to_string();
    //         let addr = format!("/ip4/0.0.0.0/tcp/{}", port.parse::<u16>().unwrap());
    //         Swarm::listen_on(&mut swarm, addr.parse().unwrap()).unwrap();
    //         // swarm.add_self_reported_address(&args[2].parse().unwrap(), args[1].parse().unwrap());
    //     }
    //     None => {} // Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
    //                // println!("ADDING PEERS!");
    //                // swarm.add_self_reported_address(&args[2].parse().unwrap(), args[1].parse().unwrap());
    // }
    // Kick it off
    let mut listening = false;
    tokio::run(futures::future::poll_fn(move || -> Result<_, ()> {
        loop {
            match framed_stdin.poll().expect("Error while polling stdin") {
                Async::Ready(Some(line)) => {
                    // swarm.floodsub.publish(&floodsub_topic, line.as_bytes())
                    println!("{:?}", line);
                }
                Async::Ready(None) => panic!("Stdin closed"),
                Async::NotReady => break,
            };
        }
        loop {
            match swarm.poll().expect("Error while polling swarm") {
                Async::Ready(Some(event)) => println!("{:?}", event),
                Async::Ready(None) | Async::NotReady => {
                    if !listening {
                        if let Some(a) = Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    break;
                }
            }
        }
        Ok(Async::NotReady)
    }));
}
