use futures::prelude::*;
use futures03::{compat::Compat, TryFutureExt as _};
use futures_timer::Delay;
use libp2p::kad::record::{self, store::MemoryStore};
use libp2p::kad::{GetClosestPeersError, KademliaConfig};
use libp2p::kad::{Kademlia, KademliaEvent, Quorum, Record};
use libp2p::{
  core::{either::EitherOutput, ConnectedPoint, PeerId, PublicKey},
  identify::{Identify, IdentifyEvent, IdentifyInfo},
  swarm::{IntoProtocolsHandler, IntoProtocolsHandlerSelect, PollParameters, ProtocolsHandler},
  swarm::{NetworkBehaviour, NetworkBehaviourAction},
  tokio_io::{AsyncRead, AsyncWrite},
  Multiaddr,
};
use std::cmp;
use std::time::Duration;

pub struct Behaviour<TSubstream> {
  next_kad_random_query: Compat<Delay>,
  duration_to_next_kad: Duration,
  num_connections: u64,
  /// Periodically identifies the remote and responds to incoming requests.
  identify: Identify<TSubstream>,
  kademlia: Kademlia<TSubstream, MemoryStore>,
}

/// Event that can be emitted by the behaviour.
#[derive(Debug)]
pub enum AllEvents {
  /// We have obtained debug information from a peer, including the addresses it is listening on.
  Identified {
    /// Id of the peer that has been identified.
    peer_id: PeerId,
    /// Information about the peer.
    info: IdentifyInfo,
  },
  DiscoveryOut(DiscoveryOutT),
}

#[derive(Debug)]
pub enum DiscoveryOutT {
  /// The address of a peer has been added to the Kademlia routing table.
  ///
  /// Can be called multiple times with the same identity.
  Discovered(PeerId),

  /// A peer connected to this node for whom no listen address is known.
  ///
  /// In order for the peer to be added to the Kademlia routing table, a known
  /// listen address must be added via [`DiscoveryBehaviour::add_self_reported_address`],
  /// e.g. obtained through the `identify` protocol.
  UnroutablePeer(PeerId),

  /// The DHT yeided results for the record request, grouped in (key, value) pairs.
  ValueFound(Vec<(record::Key, Vec<u8>)>),

  /// The record requested was not found in the DHT.
  ValueNotFound(record::Key),

  /// The record with a given key was successfully inserted into the DHT.
  ValuePut(record::Key),

  /// Inserting a value into the DHT failed.
  ValuePutFailed(record::Key),
}

impl<TSubstream> Behaviour<TSubstream> {
  pub fn new(user_agent: String, local_public_key: PublicKey) -> Self {
    let identify = {
      let proto_version = "/radiopeer/0.1.0".to_string();
      Identify::new(proto_version, user_agent, local_public_key.clone())
    };
    let local_peer_id = local_public_key.clone().into_peer_id();
    let mut cfg = KademliaConfig::default();
    cfg.set_query_timeout(Duration::from_secs(5 * 60));
    let store = MemoryStore::new(local_peer_id.clone());
    Behaviour {
      next_kad_random_query: Delay::new(Duration::new(0, 0)).compat(),
      duration_to_next_kad: Duration::from_secs(1),
      num_connections: 0,
      identify,
      kademlia: Kademlia::with_config(local_peer_id.clone(), store, cfg),
    }
  }

  /// Returns the list of nodes that we know exist in the network.
  pub fn known_peers(&mut self) -> impl Iterator<Item = &PeerId> {
    self.kademlia.kbuckets_entries()
  }

  fn handle_identify_report(&mut self, peer_id: &PeerId, info: &IdentifyInfo) {
    // let address = info.listen_addrs[0].clone();
    for addr in &info.listen_addrs {
      println!(
        "Adding peer: {:?} at address: {:?} to kademlia",
        peer_id, addr
      );
      self.add_self_reported_address(&peer_id, addr.clone());
    }
  }

  pub fn put_value(&mut self, key: record::Key, value: Vec<u8>) {
    self
      .kademlia
      .put_record(Record::new(key, value), Quorum::All);
  }

  pub fn add_self_reported_address(&mut self, peer_id: &PeerId, addr: Multiaddr) {
    self.kademlia.add_address(peer_id, addr);
  }
}

impl<TSubstream> NetworkBehaviour for Behaviour<TSubstream>
where
  TSubstream: AsyncRead + AsyncWrite,
{
  type ProtocolsHandler = IntoProtocolsHandlerSelect<
    <Kademlia<TSubstream, MemoryStore> as NetworkBehaviour>::ProtocolsHandler,
    <Identify<TSubstream> as NetworkBehaviour>::ProtocolsHandler,
  >;
  type OutEvent = AllEvents;
  fn new_handler(&mut self) -> Self::ProtocolsHandler {
    IntoProtocolsHandler::select(self.kademlia.new_handler(), self.identify.new_handler())
  }
  fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
    let mut list = self.kademlia.addresses_of_peer(peer_id);
    list.extend_from_slice(&self.identify.addresses_of_peer(peer_id));
    list
  }
  fn inject_connected(&mut self, peer_id: PeerId, endpoint: ConnectedPoint) {
    self.num_connections += 1;
    self
      .kademlia
      .inject_connected(peer_id.clone(), endpoint.clone());
    self
      .identify
      .inject_connected(peer_id.clone(), endpoint.clone());
  }
  fn inject_disconnected(&mut self, peer_id: &PeerId, endpoint: ConnectedPoint) {
    self.num_connections -= 1;
    self.kademlia.inject_disconnected(peer_id, endpoint.clone());
    self.identify.inject_disconnected(peer_id, endpoint);
  }
  fn inject_node_event(
    &mut self,
    peer_id: PeerId,
    event: <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::OutEvent,
  ) {
    match event {
      EitherOutput::First(event) => self.kademlia.inject_node_event(peer_id, event),
      EitherOutput::Second(event) => self.identify.inject_node_event(peer_id, event),
    }
  }

  fn inject_new_external_addr(&mut self, addr: &Multiaddr) {
    println!("EXTERNAL:{}", addr);
    self.kademlia.inject_new_external_addr(addr);
    self.identify.inject_new_external_addr(addr);
  }

  fn poll(
    &mut self,
    params: &mut impl PollParameters,
  ) -> Async<
    NetworkBehaviourAction<
      <<Self::ProtocolsHandler as IntoProtocolsHandler>::Handler as ProtocolsHandler>::InEvent,
      Self::OutEvent,
    >,
  > {
    // Poll the stream that fires when we need to start a random Kademlia query.
    // self.kademlia.get_closest_peers(self.local_peer_id.clone());
    loop {
      match self.next_kad_random_query.poll() {
        Ok(Async::NotReady) => break,
        Ok(Async::Ready(_)) => {
          let random_peer_id = PeerId::random();
          println!(
            "Connected to {} peers, Starting random Kademlia request for {:?}",
            self.num_connections, random_peer_id
          );
          self.kademlia.get_closest_peers(random_peer_id);
          // Schedule the next random query with exponentially increasing delay,
          // capped at 60 seconds.
          self.next_kad_random_query = Delay::new(self.duration_to_next_kad).compat();
          self.duration_to_next_kad =
            cmp::min(self.duration_to_next_kad * 2, Duration::from_secs(60));
        }
        Err(err) => {
          println!("Kademlia query timer errored: {:?}", err);
          break;
        }
      }
    }
    loop {
      match self.kademlia.poll(params) {
        Async::NotReady => break,
        Async::Ready(NetworkBehaviourAction::GenerateEvent(ev)) => match ev {
          KademliaEvent::UnroutablePeer { peer, .. } => {
            println!("UNROUTABLE: {}", peer);
            let ev = DiscoveryOutT::UnroutablePeer(peer);
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(
              AllEvents::DiscoveryOut(ev),
            ));
          }
          KademliaEvent::RoutingUpdated { peer, .. } => {
            println!("RoutingUpdated: {}", peer);
            let ev = DiscoveryOutT::Discovered(peer);
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(
              AllEvents::DiscoveryOut(ev),
            ));
          }
          KademliaEvent::GetClosestPeersResult(res) => match res {
            Err(GetClosestPeersError::Timeout { key, peers }) => {
              println!(
                "Libp2p => Query for {:?} timed out with {} results",
                &key,
                peers.len()
              );
            }
            Ok(ok) => {
              println!("RoutingUpdated: {:?}", ok.peers);
              println!(
                "Libp2p => Query for {:?} yielded {:?} results",
                &ok.key,
                ok.peers.len()
              );
              if ok.peers.is_empty() && self.num_connections != 0 {
                println!(
                  "Libp2p => Random Kademlia query has yielded empty \
                   results"
                );
              }
            }
          },
          KademliaEvent::GetRecordResult(res) => {
            let ev = match res {
              Ok(ok) => {
                let results = ok.records.into_iter().map(|r| (r.key, r.value)).collect();

                // DiscoveryOut::ValueFound(results)
                DiscoveryOutT::ValueFound(results)
              }
              Err(e) => DiscoveryOutT::ValueNotFound(e.into_key()),
            };
            println!("GetRecordResult: {:?}", ev);
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(
              AllEvents::DiscoveryOut(ev),
            ));
          }
          KademliaEvent::PutRecordResult(res) => {
            let ev = match res {
              Ok(ok) => DiscoveryOutT::ValuePut(ok.key),
              Err(e) => DiscoveryOutT::ValuePutFailed(e.into_key()),
            };
            println!("PutRecordResult: {:?}", ev);
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(
              AllEvents::DiscoveryOut(ev),
            ));
          }
          KademliaEvent::RepublishRecordResult(res) => match res {
            Ok(ok) => println!("Libp2p => Record republished: {:?}", ok.key),
            Err(e) => println!(
              "Libp2p => Republishing of record {:?} failed with: {:?}",
              e.key(),
              e
            ),
          },
          KademliaEvent::Discovered { .. } => {
            // We are not interested in these events at the moment.
          }
          // We never start any other type of query.
          e => println!("Libp2p => Unhandled Kademlia event: {:?}", e), // if let PingEvent {
                                                                        //     peer,
                                                                        //     result: Ok(PingSuccess::Ping { rtt }),
                                                                        // } = ev
                                                                        // {
                                                                        //     self.handle_ping_report(&peer, rtt)
                                                                        // }
        },
        Async::Ready(NetworkBehaviourAction::DialAddress { address }) => {
          return Async::Ready(NetworkBehaviourAction::DialAddress { address })
        }
        Async::Ready(NetworkBehaviourAction::DialPeer { peer_id }) => {
          return Async::Ready(NetworkBehaviourAction::DialPeer { peer_id })
        }
        Async::Ready(NetworkBehaviourAction::SendEvent { peer_id, event }) => {
          return Async::Ready(NetworkBehaviourAction::SendEvent {
            peer_id,
            event: EitherOutput::First(event),
          })
        }
        Async::Ready(NetworkBehaviourAction::ReportObservedAddr { address }) => {
          return Async::Ready(NetworkBehaviourAction::ReportObservedAddr { address })
        }
      }
    }
    loop {
      match self.identify.poll(params) {
        Async::NotReady => break,
        Async::Ready(NetworkBehaviourAction::GenerateEvent(event)) => match event {
          IdentifyEvent::Received { peer_id, info, .. } => {
            self.handle_identify_report(&peer_id, &info);
            let event = AllEvents::Identified { peer_id, info };
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(event));
          }
          IdentifyEvent::Error { peer_id, error } => {
            println!("Identification with peer {:?} failed => {}", peer_id, error)
          }
          IdentifyEvent::Sent { .. } => {}
        },
        Async::Ready(NetworkBehaviourAction::DialAddress { address }) => {
          println!("DialAddress {:?}", address);
          return Async::Ready(NetworkBehaviourAction::DialAddress { address });
        }
        Async::Ready(NetworkBehaviourAction::DialPeer { peer_id }) => {
          println!("DialPeer {:?}", peer_id);
          return Async::Ready(NetworkBehaviourAction::DialPeer { peer_id });
        }
        Async::Ready(NetworkBehaviourAction::SendEvent { peer_id, event }) => {
          println!("SendEvent {:?} -- {:?}", peer_id, event);
        }
        Async::Ready(NetworkBehaviourAction::ReportObservedAddr { address }) => {
          println!("ReportObservedAddr {:?}", address);
          return Async::Ready(NetworkBehaviourAction::ReportObservedAddr { address });
        }
      }
    }
    Async::NotReady
  }
}
