/// Choose the resource random hash and derive every hash that depends on it.
///
/// The map hash is intentionally short, so uniqueness is required only in the
/// same rolling window that the Python reference uses. The generic inputs keep
/// the collision path deterministically testable without weakening production
/// randomness or making a collision likely in normal transfers.
type CollisionFreeHashSelection = ([u8; RANDOM_HASH_SIZE], Vec<[u8; MAPHASH_LEN]>, Hash, Hash);

fn select_collision_free_random_hash<F, H>(
    identity_payload: &[u8],
    parts: &[Vec<u8>],
    mut next_random_hash: F,
    map_hash_fn: H,
) -> Result<CollisionFreeHashSelection, RnsError>
where
    F: FnMut() -> [u8; RANDOM_HASH_SIZE],
    H: Fn(&[u8], &[u8; RANDOM_HASH_SIZE]) -> [u8; MAPHASH_LEN],
{
    loop {
        let random_hash = next_random_hash();
        let mut guard = VecDeque::with_capacity(COLLISION_GUARD_SIZE);
        let mut map_hashes = Vec::with_capacity(parts.len());
        let mut collision = false;

        for part in parts {
            let hash = map_hash_fn(part, &random_hash);
            if guard.contains(&hash) {
                collision = true;
                break;
            }
            guard.push_back(hash);
            if guard.len() > COLLISION_GUARD_SIZE {
                guard.pop_front();
            }
            map_hashes.push(hash);
        }

        if collision {
            log::debug!("resource map hash collision, regenerating resource random hash");
            continue;
        }

        let mut hasher = sha2::Sha256::new();
        hasher.update(identity_payload);
        hasher.update(random_hash);
        let resource_hash = Hash::new(copy_hash(&hasher.finalize())?);

        let mut proof_hasher = sha2::Sha256::new();
        proof_hasher.update(identity_payload);
        proof_hasher.update(resource_hash.as_slice());
        let expected_proof = Hash::new(copy_hash(&proof_hasher.finalize())?);
        return Ok((random_hash, map_hashes, resource_hash, expected_proof));
    }
}
