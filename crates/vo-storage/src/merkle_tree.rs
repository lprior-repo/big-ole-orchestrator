//! Merkle tree hasher for content-addressed storage.
//!
//! Provides a binary Merkle tree structure for computing tamper-evident
//! content addresses. Leaf nodes are BLAKE3 hashes of data chunks;
//! internal nodes are BLAKE3 hashes of their children's content.
//!
//! Architecture: Data layer only — pure types and computation, no I/O.

use serde::{Deserialize, Serialize};

use crate::checksum::{Checksum, ChunkedHasher};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleNode {
    pub hash: [u8; 32],
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleTree {
    pub root_hash: [u8; 32],
    pub leaf_hashes: Vec<Checksum>,
    pub chunk_size: u64,
    levels: Vec<Vec<MerkleNode>>,
}

impl MerkleTree {
    #[must_use]
    pub fn new(data: &[u8], chunk_size: u64) -> Self {
        if data.is_empty() {
            return Self {
                root_hash: [0u8; 32],
                leaf_hashes: Vec::new(),
                chunk_size,
                levels: Vec::new(),
            };
        }

        let mut chunk_hasher = ChunkedHasher::new(chunk_size);
        chunk_hasher.update(data);
        let chunks = chunk_hasher.finalize();

        let leaf_hashes: Vec<Checksum> = chunks.iter().map(|c| c.checksum.clone()).collect();

        let mut levels = Vec::new();
        let leaf_nodes: Vec<MerkleNode> = chunks
            .iter()
            .map(|c| MerkleNode {
                hash: c.checksum.blake3,
                offset: c.offset,
                size: c.size,
            })
            .collect();
        levels.push(leaf_nodes);

        while levels.last().is_some_and(|level| level.len() > 1) {
            let last = levels.last().filter(|l| l.len() > 1);
            if let Some(current) = last {
                let parent_level = pair_and_hash(current);
                levels.push(parent_level);
            }
        }

        let root_hash = levels
            .last()
            .and_then(|level| level.first())
            .map_or([0u8; 32], |node| node.hash);

        Self {
            root_hash,
            leaf_hashes,
            chunk_size,
            levels,
        }
    }

    #[must_use]
    pub fn root_hash(&self) -> [u8; 32] {
        self.root_hash
    }

    #[must_use]
    pub fn proof(&self, leaf_index: usize) -> Option<MerkleProof> {
        if leaf_index >= self.leaf_hashes.len() {
            return None;
        }

        let mut proof_hashes = Vec::new();
        let mut current_index = leaf_index;

        for level in 0..self.levels.len() - 1 {
            let is_left = current_index.is_multiple_of(2);
            let sibling_index = if is_left {
                current_index + 1
            } else {
                current_index.saturating_sub(1)
            };

            let current_level = &self.levels[level];
            if sibling_index < current_level.len() {
                proof_hashes.push((current_level[sibling_index].hash, is_left));
            } else {
                proof_hashes.push((current_level[current_index].hash, is_left));
            }

            current_index /= 2;
        }

        Some(MerkleProof {
            leaf_hash: self.leaf_hashes[leaf_index].blake3,
            leaf_index,
            proof_hashes,
            chunk_size: self.chunk_size,
        })
    }
}

fn pair_and_hash(nodes: &[MerkleNode]) -> Vec<MerkleNode> {
    let mut parent_nodes = Vec::new();

    for pair in nodes.chunks(2) {
        let (hash, size) = if pair.len() == 2 {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&pair[0].hash);
            hasher.update(&pair[1].hash);
            (*hasher.finalize().as_bytes(), pair[0].size + pair[1].size)
        } else {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&pair[0].hash);
            hasher.update(&pair[0].hash);
            (*hasher.finalize().as_bytes(), pair[0].size * 2)
        };

        parent_nodes.push(MerkleNode {
            hash,
            offset: pair[0].offset,
            size,
        });
    }

    parent_nodes
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: [u8; 32],
    pub leaf_index: usize,
    pub proof_hashes: Vec<([u8; 32], bool)>,
    pub chunk_size: u64,
}

impl MerkleProof {
    #[must_use]
    pub fn verify(&self, expected_root: [u8; 32]) -> bool {
        let mut current_hash = self.leaf_hash;

        for (sibling_hash, is_left) in &self.proof_hashes {
            let mut hasher = blake3::Hasher::new();
            if *is_left {
                hasher.update(&current_hash);
                hasher.update(sibling_hash);
            } else {
                hasher.update(sibling_hash);
                hasher.update(&current_hash);
            }
            current_hash = *hasher.finalize().as_bytes();
        }

        current_hash == expected_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::StreamingHasher;

    #[test]
    fn empty_data_returns_zero_root() {
        let data = b"";
        let tree = MerkleTree::new(data, 1024);
        assert_eq!(tree.root_hash(), [0u8; 32]);
        assert!(tree.leaf_hashes.is_empty());
    }

    #[test]
    fn single_chunk_merkle_tree() {
        let data = b"hello world";
        let tree = MerkleTree::new(data, 1024);

        let expected_leaf_hash = {
            let mut hasher = StreamingHasher::new();
            hasher.update(data);
            hasher.finalize().blake3
        };

        assert_eq!(tree.leaf_hashes.len(), 1);
        assert_eq!(tree.leaf_hashes[0].blake3, expected_leaf_hash);
        assert_eq!(tree.levels.len(), 1);
        assert_eq!(tree.levels[0][0].hash, expected_leaf_hash);
        assert_eq!(tree.root_hash(), expected_leaf_hash);
    }

    #[test]
    fn two_chunk_merkle_tree() {
        let data = b"0123456789ABCDEF";
        let tree = MerkleTree::new(data, 8);

        assert_eq!(tree.leaf_hashes.len(), 2);
        assert_eq!(tree.levels.len(), 2);
        assert_eq!(tree.levels[0].len(), 2);
        assert_eq!(tree.levels[1].len(), 1);

        let mut expected_parent_hasher = blake3::Hasher::new();
        expected_parent_hasher.update(&tree.levels[0][0].hash);
        expected_parent_hasher.update(&tree.levels[0][1].hash);
        let expected_parent = *expected_parent_hasher.finalize().as_bytes();

        assert_eq!(tree.levels[1][0].hash, expected_parent);
        assert_eq!(tree.root_hash(), expected_parent);
    }

    #[test]
    fn uneven_chunk_merkle_tree() {
        let data = b"0123456789ABCDE";
        let tree = MerkleTree::new(data, 8);

        assert_eq!(tree.leaf_hashes.len(), 2);
        assert!(tree.levels.len() >= 2);
    }

    #[test]
    fn proof_verification_single_chunk() {
        let data = b"hello world";
        let tree = MerkleTree::new(data, 1024);

        let proof = tree.proof(0).expect("should have proof");
        assert!(proof.verify(tree.root_hash()));
    }

    #[test]
    fn proof_verification_two_chunks() {
        let data = b"0123456789ABCDEF";
        let tree = MerkleTree::new(data, 8);
        let root = tree.root_hash();

        let proof0 = tree.proof(0).expect("should have proof for leaf 0");
        let proof1 = tree.proof(1).expect("should have proof for leaf 1");

        assert!(proof0.verify(root));
        assert!(proof1.verify(root));
    }

    #[test]
    fn proof_verification_fails_with_wrong_root() {
        let data = b"hello world";
        let tree = MerkleTree::new(data, 1024);

        let proof = tree.proof(0).expect("should have proof");
        let wrong_root = [0u8; 32];
        assert!(!proof.verify(wrong_root));
    }

    #[test]
    fn proof_invalid_index_returns_none() {
        let data = b"hello world";
        let tree = MerkleTree::new(data, 1024);

        assert!(tree.proof(100).is_none());
    }

    #[test]
    fn different_data_produces_different_root() {
        let data1 = b"hello world";
        let data2 = b"hello world!";
        let tree1 = MerkleTree::new(data1, 1024);
        let tree2 = MerkleTree::new(data2, 1024);

        assert_ne!(tree1.root_hash(), tree2.root_hash());
    }

    #[test]
    fn same_data_same_chunk_size_produces_same_root() {
        let data = b"hello world";
        let tree1 = MerkleTree::new(data, 1024);
        let tree2 = MerkleTree::new(data, 1024);

        assert_eq!(tree1.root_hash(), tree2.root_hash());
    }

    #[test]
    fn merkle_tree_serde_roundtrip() {
        let data = b"test data for serialization";
        let tree = MerkleTree::new(data, 64);

        let json = serde_json::to_string(&tree).expect("serialize");
        let recovered: MerkleTree = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tree.root_hash(), recovered.root_hash());
        assert_eq!(tree.leaf_hashes.len(), recovered.leaf_hashes.len());
    }

    #[test]
    fn merkle_proof_serde_roundtrip() {
        let data = b"0123456789ABCDEF";
        let tree = MerkleTree::new(data, 8);
        let proof = tree.proof(0).expect("should have proof");

        let json = serde_json::to_string(&proof).expect("serialize");
        let recovered: MerkleProof = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(proof.leaf_hash, recovered.leaf_hash);
        assert!(recovered.verify(tree.root_hash()));
    }

    #[test]
    fn proof_hash_count_correct() {
        let data: Vec<u8> = (0..100u8).collect();
        let tree = MerkleTree::new(&data, 8);
        let root = tree.root_hash();

        for i in 0..tree.leaf_hashes.len() {
            let proof = tree.proof(i).expect("should have proof");
            assert!(proof.verify(root), "proof {i} should verify");
        }
    }

    #[test]
    fn proof_hash_count_matches_tree_height() {
        let data: Vec<u8> = (0..100u8).collect();
        let tree = MerkleTree::new(&data, 8);

        let expected_proof_length = tree.levels.len() - 1;
        for i in 0..tree.leaf_hashes.len() {
            let proof = tree.proof(i).expect("should have proof");
            assert_eq!(
                proof.proof_hashes.len(),
                expected_proof_length,
                "proof {i} length mismatch"
            );
        }
    }
}
