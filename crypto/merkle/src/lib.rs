use kaspa_hashes::{Hash, HasherBase, MerkleBranchHash, ZERO_HASH};
#[derive(Default)]
pub enum LeafRoute {
    #[default]
    Left,
    Right,
}
pub struct WitnessSegment {
    companion_hash: Hash,
    leaf_route: LeafRoute,
}
impl WitnessSegment {
    fn leaf_route(&self) -> &LeafRoute {
        &self.leaf_route
    }
    fn companion_hash(&self) -> Hash {
        self.companion_hash
    }
}

fn derive_merkle_tree(hashes: impl ExactSizeIterator<Item = Hash>) -> Vec<Option<Hash>> {
    if hashes.len() == 0 {
        return vec![Some(ZERO_HASH)];
    }
    let next_pot = hashes.len().next_power_of_two(); //maximal number of  leaves in last level of tree
    let vec_len = 2 * next_pot - 1; //maximal number of nodes in tree
    let mut merkles = vec![None; vec_len];
    //store leaves in the bottom level of the tree
    for (i, hash) in hashes.enumerate() {
        merkles[i] = Some(hash);
    }
    //compute merkle tree
    let mut offset = next_pot;
    for i in (0..vec_len - 1).step_by(2) {
        if merkles[i].is_none() {
            merkles[offset] = None;
        } else {
            merkles[offset] = Some(merkle_hash(merkles[i].unwrap(), merkles[i + 1].unwrap_or(ZERO_HASH)));
        }
        offset += 1
    }
    merkles
}

pub fn calc_merkle_root(hashes: impl ExactSizeIterator<Item = Hash>) -> Hash {
    // derive the merkle tree
    // the last element in the tree is always the merkle tree root.
    let merkles = derive_merkle_tree(hashes);
    merkles.last().unwrap().unwrap()
}
pub fn create_merkle_witness(hashes: impl ExactSizeIterator<Item = Hash>, leaf_hash: Hash) -> Option<Vec<WitnessSegment>> {
    //leaf index must be smaller than amount of leaves, otherwise an error is returned

    let next_pot = hashes.len().next_power_of_two(); //maximal number of  leaves in last level of tree
    let mut leaf_index = None;
    let merkles = derive_merkle_tree(hashes);
    for (index, hash_element) in merkles.iter().enumerate().take(next_pot) {
        if hash_element.is_none() {
            continue;
        } else if leaf_hash == hash_element.unwrap() {
            leaf_index = Some(index);
            break;
        }
    }
    leaf_index?;
    let leaf_index = leaf_index.unwrap();
    let mut witness_vec = vec![];

    let mut level_start = 0;
    let mut level_length = next_pot;
    let mut level_index = leaf_index;
    //iterate over the indices per level corresponding to the route from leaf to the root and collect their "matches"
    //alongside the path - the merkle root itself is not collected
    while level_length > 1 {
        witness_vec.push({
            //the leaf_index describes the indexing of the leaf itself per level, we store its "companion" hash as witness
            if level_index % 2 == 0 {
                WitnessSegment {
                    companion_hash: merkles[level_start + level_index + 1].unwrap_or(ZERO_HASH),
                    leaf_route: LeafRoute::Left,
                }
            }
            //edge case relevant to the leaf level only
            else {
                WitnessSegment { companion_hash: merkles[level_start + level_index - 1].unwrap(), leaf_route: LeafRoute::Right }
            }
        });

        level_start += level_length;
        level_length /= 2;
        level_index /= 2;
    }
    // assert_eq!(level_start,vec_len-1);
    // assert_eq!(level_index,0);
    Some(witness_vec)
}

pub fn verify_merkle_witness(witness_vec: &[WitnessSegment], leaf_value: Hash, merkle_root_hash: Hash) -> bool {
    let mut current_hash = leaf_value;
    for witness_segment in witness_vec.iter() {
        //the LeafRoute describes which branch the leaf is at from bottom to top
        match witness_segment.leaf_route() {
            LeafRoute::Right => {
                current_hash = merkle_hash(witness_segment.companion_hash(), current_hash);
            }
            LeafRoute::Left => {
                current_hash = merkle_hash(current_hash, witness_segment.companion_hash());
            }
        }
    }
    current_hash == merkle_root_hash
}

fn merkle_hash(left_node: Hash, right_node: Hash) -> Hash {
    let mut hasher = MerkleBranchHash::new();
    hasher.update(left_node).update(right_node);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::{calc_merkle_root, create_merkle_witness, verify_merkle_witness};
    use kaspa_hashes::Hash;
    use kaspa_hashes::{HASH_SIZE, ZERO_HASH};
    // test the case of the empty tree which gets missed in the more general tests
    const HASH1: [u8; 32] = [0x1u8; HASH_SIZE];
    const HASH2: [u8; 32] = [0x2u8; HASH_SIZE];
    const HASH3: [u8; 32] = [0x3u8; HASH_SIZE];
    #[test]
    fn test_witnesses_empty() {
        let empty_vec = vec![];
        let empty_witness = create_merkle_witness(empty_vec.clone().into_iter(), ZERO_HASH).unwrap();
        let merkle_root = calc_merkle_root(empty_vec.clone().into_iter());

        //sanity checks
        assert_eq!(empty_vec, vec!());
        assert_eq!(merkle_root, ZERO_HASH);
        assert!(verify_merkle_witness(&empty_witness, ZERO_HASH, merkle_root));
        //check false is returned for other hashes
        assert!(!verify_merkle_witness(&empty_witness, Hash::from(HASH1), merkle_root));
        //check erronous case behaves as expected
        assert!(create_merkle_witness(empty_vec.clone().into_iter(), Hash::from(HASH1)).is_none());
    }
    // test separately the single leaf and double leaf tree cases
    #[test]
    fn test_witnesses_basic() {
        let single_vec = vec![Hash::from(HASH1)];
        let double_vec = vec![Hash::from(HASH1), Hash::from(HASH2)];
        assert!(verify_merkle_witness(
            &create_merkle_witness(single_vec.clone().into_iter(), Hash::from(HASH1)).unwrap(),
            Hash::from(HASH1),
            calc_merkle_root(single_vec.clone().into_iter())
        ));
        assert!(verify_merkle_witness(
            &create_merkle_witness(double_vec.clone().into_iter(), Hash::from(HASH1)).unwrap(),
            Hash::from(HASH1),
            calc_merkle_root(double_vec.clone().into_iter())
        ));
        assert!(verify_merkle_witness(
            &create_merkle_witness(double_vec.clone().into_iter(), Hash::from(HASH2)).unwrap(),
            Hash::from(HASH2),
            calc_merkle_root(double_vec.clone().into_iter())
        ));
        // //testing erronous case behaviour
        assert!(create_merkle_witness(single_vec.clone().into_iter(), Hash::from(HASH2)).is_none());
        assert!(create_merkle_witness(single_vec.clone().into_iter(), Hash::from(HASH3)).is_none());
        assert!(create_merkle_witness(double_vec.clone().into_iter(), Hash::from(HASH3)).is_none());
    }
    #[test]
    fn test_witnesses_consistency() {
        const TREE_LENGTH: usize = 30;

        let mut hash_vec = vec![];
        for i in 0..TREE_LENGTH {
            let temp = [(i + 2) as u8; HASH_SIZE]; //skip ZERO_HASH and HASH1
            hash_vec.push(Hash::from(temp));
        }
        for _ in 0..TREE_LENGTH {
            //feel up missing space with "garbage"
            hash_vec.push(Hash::from(HASH1));
        }
        for i in 1..TREE_LENGTH {
            //disregard the 0 edge case as it is tested separately
            for leaf_index in 0..i {
                let witness = create_merkle_witness(hash_vec.clone().into_iter().take(i), hash_vec[leaf_index]).unwrap();
                let merkle_root = calc_merkle_root(hash_vec.clone().into_iter().take(i));
                assert!(verify_merkle_witness(&witness, hash_vec[leaf_index], merkle_root));
                //check false is returned when witness doesn't match
                assert!(!verify_merkle_witness(&witness, hash_vec[leaf_index + 1], merkle_root));
            }
            //testing erronous case behaviour
            let leaf_index = 2 * i - 1;
            assert!(create_merkle_witness(hash_vec.clone().into_iter().take(i), hash_vec[leaf_index]).is_none());
        }
    }
}
