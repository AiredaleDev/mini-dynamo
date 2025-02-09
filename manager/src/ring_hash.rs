use sha1::{Digest, Sha1};
use smallvec::SmallVec;
use std::collections::BTreeMap;

pub struct RingHash {
    /// Relates hashes to node indices.
    ring_hash: BTreeMap<usize, usize>,
    /// Number of replicas and virtual nodes.
    repl: usize,
}

impl RingHash {
    pub fn new(repl: usize) -> Self {
        Self {
            ring_hash: BTreeMap::new(),
            repl,
        }
    }

    pub fn add_node(&mut self, node_index: usize) {
        for i in 0..self.repl {
            let hash = Sha1::digest(format!("node_{node_index}_rep_{i}"))[..8]
                .try_into()
                .unwrap();
            let hash = usize::from_ne_bytes(hash);
            self.ring_hash.insert(hash, node_index);
        }
    }

    pub fn remove_node(&mut self, node_index: usize) {
        for i in 0..self.repl {
            let hash = Sha1::digest(format!("node_{node_index}_rep_{i}"))[..8]
                .try_into()
                .unwrap();
            let hash = usize::from_ne_bytes(hash);
            self.ring_hash.remove(&hash);
        }
    }

    // A little bruh to be boxing when the size of each of these is technically
    // known at compile-time. I do NOT trust the compiler to infer that the ACTUAL
    // type is [usize; self.repl] and unbox all these slices.
    pub fn write_group(&self, key: &str) -> SmallVec<[usize; 16]> {
        let key_hash = Sha1::digest(key)[..8].try_into().unwrap();
        let key_hash = usize::from_ne_bytes(key_hash);

        // self.repl is small so `contains` on a `Vec` will be much faster than hashing.
        // This might actually be a good case for SmallVec, no reason this can't live on the stack.
        let mut group = SmallVec::with_capacity(self.repl);
        let mut nodes_iter = self
            .ring_hash
            .iter()
            .cycle()
            .skip_while(|(hash, _)| **hash <= key_hash);
        // It might be a little expensive to remove a node from the ring every time it goes down.
        while group.len() < self.repl || group.len() < self.ring_hash.len() {
            let (_, node) = nodes_iter.next().unwrap();
            if !group.contains(node) {
                group.push(*node);
            }
        }

        group
    }
}
