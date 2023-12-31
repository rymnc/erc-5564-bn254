use ark_bn254::g1::{G1_GENERATOR_X, G1_GENERATOR_Y};
use ark_bn254::{Fr, G1Affine, G1Projective};
use ark_ff::UniformRand;
use ark_std::rand::rngs::OsRng;
use rln::hashers::{hash_to_field, poseidon_hash};

pub fn derive_public_key(private_key: Fr) -> G1Projective {
    let g1_generator_affine = G1Affine::new(G1_GENERATOR_X, G1_GENERATOR_Y);
    (G1Projective::from(g1_generator_affine)) * private_key
}

pub fn random_keypair() -> (Fr, G1Projective) {
    let private_key = generate_random_fr();
    let public_key = derive_public_key(private_key);
    (private_key, public_key)
}

pub fn generate_random_fr() -> Fr {
    let mut rng = OsRng;
    Fr::rand(&mut rng)
}

pub fn hash_to_fr(input: &[u8]) -> Fr {
    poseidon_hash(&[hash_to_field(input)])
}

pub fn compute_shared_point(private_key: Fr, other_public_key: G1Projective) -> G1Projective {
    other_public_key * private_key
}

pub fn generate_stealth_commitment(
    viewing_public_key: G1Projective,
    spending_public_key: G1Projective,
    ephemeral_private_key: Fr,
) -> (G1Projective, u64) {
    let q = compute_shared_point(ephemeral_private_key, viewing_public_key);
    let inputs = q.to_string();
    let q_hashed = hash_to_fr(inputs.as_bytes());

    let q_hashed_in_g1 = derive_public_key(q_hashed);
    let view_tag = q_hashed.0 .0[0];
    (q_hashed_in_g1 + spending_public_key, view_tag)
}

pub fn generate_stealth_private_key(
    ephemeral_public_key: G1Projective,
    viewing_key: Fr,
    spending_key: Fr,
    expected_view_tag: u64,
) -> Option<Fr> {
    let q_receiver = compute_shared_point(viewing_key, ephemeral_public_key);

    let inputs_receiver = q_receiver.to_string();
    let q_receiver_hashed = hash_to_fr(inputs_receiver.as_bytes());

    // Check if retrieved view tag matches the expected view tag
    let view_tag = q_receiver_hashed.0 .0[0];
    if view_tag == expected_view_tag {
        let stealth_private_key = spending_key + q_receiver_hashed;
        Some(stealth_private_key)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ec::CurveGroup;
    use rln::public::RLN;
    use std::io::Cursor;
    use ark_std::rand::thread_rng;
    use color_eyre::{Report, Result};
    use rln::utils::fr_to_bytes_le;
    use serde_json::json;

    #[test]
    fn test_random_keypair() {
        let (key, pub_key) = random_keypair();
        // Check the derived key matches the one generated from original key
        assert_eq!(derive_public_key(key), pub_key);
    }

    #[test]
    fn test_hash_to_fr() {
        // Test that hash_to_fr(input_1) != hash_to_fr(input_2) when input_1 != input_2
        let input_1 = b"input_1";
        let input_2 = b"input_2";
        assert_ne!(hash_to_fr(input_1), hash_to_fr(input_2));
    }

    #[test]
    fn test_compute_shared_point() {
        // In a multiple participant scenario, any participant's public key
        // combined with any other participant's private key should arrive at the same shared key
        let (key1, pub_key1) = random_keypair();
        let (key2, pub_key2) = random_keypair();

        let shared1 = compute_shared_point(key1, pub_key2);
        let shared2 = compute_shared_point(key2, pub_key1);

        // Convert Projective to Affine for equality comparison
        let shared1_affine = shared1.into_affine();
        let shared2_affine = shared2.into_affine();

        assert_eq!(shared1_affine.x, shared2_affine.x);
        assert_eq!(shared1_affine.y, shared2_affine.y);
    }

    #[test]
    fn test_stealth_commitment_generation() {
        let (spending_key, spending_public_key) = random_keypair();
        let (viewing_key, viewing_public_key) = random_keypair();

        // generate ephemeral keypair
        let (ephemeral_private_key, ephemeral_public_key) = random_keypair();

        let (stealth_commitment, view_tag) = generate_stealth_commitment(
            viewing_public_key,
            spending_public_key,
            ephemeral_private_key,
        );

        let stealth_private_key_opt =
            generate_stealth_private_key(ephemeral_public_key, viewing_key, spending_key, view_tag);

        if stealth_private_key_opt.is_none() {
            panic!("View tags did not match");
        }

        let derived_commitment = derive_public_key(stealth_private_key_opt.unwrap());
        assert_eq!(derived_commitment, stealth_commitment);
    }

    #[test]
    fn apply_stealth_membership_from_one_tree_to_another() -> Result<()> {
        let test_tree_height = 20;
        let resources = Cursor::new(json!({"resources_folder": "tree_height_20"}).to_string());
        let mut rln = RLN::new(test_tree_height, resources.clone())?;

        let alice_leaf = Fr::rand(&mut thread_rng());
        let (alice_known_spending_sk, alice_known_spending_pk) = random_keypair();
        let alice_leaf_buffer = Cursor::new(fr_to_bytes_le(&alice_leaf));
        rln.set_leaf(0, alice_leaf_buffer)?;

        // now the application sees that a user has been inserted into the tree
        let mut rln_app_tree = RLN::new(test_tree_height, resources)?;
        // the application generates a stealth commitment for alice
        let (ephemeral_private_key, ephemeral_public_key) = random_keypair();
        let (alice_stealth_commitment, view_tag) = generate_stealth_commitment(alice_known_spending_pk, alice_known_spending_pk, ephemeral_private_key);

        let parts = [alice_stealth_commitment.x, alice_stealth_commitment.y];
        let fr_parts = parts.map(|x| Fr::from(x.0));
        let alice_stealth_commitment_buffer = Cursor::new(fr_to_bytes_le(&poseidon_hash(&fr_parts)));
        rln_app_tree.set_leaf(0, alice_stealth_commitment_buffer)?;

        // now alice's stealth commitment has been inserted into the tree, but alice has not
        // yet derived the secret for it -
        let alice_stealth_private_key_opt = generate_stealth_private_key(ephemeral_public_key, alice_known_spending_sk, alice_known_spending_sk, view_tag);
        if alice_stealth_private_key_opt.is_none() {
            return Err(Report::msg("Invalid view tag"));
        }
        let alice_stealth_private_key = alice_stealth_private_key_opt.unwrap();

        assert_eq!(derive_public_key(alice_stealth_private_key), alice_stealth_commitment);

        // now alice may generate valid rln proofs for the rln app tree, using a commitment
        // derived from her commitment on the other tree
        Ok(())
    }
}
