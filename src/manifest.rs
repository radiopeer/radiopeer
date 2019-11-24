use serde::{Deserialize, Serialize};
use std::collections::HashSet;

type SongHash = Vec<u8>;
type PeerID = Vec<u8>;

#[derive(Serialize, Deserialize, Debug)]
struct Manifest {
  // admins PeerIds
  admins: HashSet<PeerID>,
  // Songs
  songs: Vec<SongHash>,

  // Not to serialize
  #[serde(skip_serializing, skip_deserializing)]
  music_track: usize,
  #[serde(skip_serializing, skip_deserializing)]
  seconds_in_music: u32,
}
