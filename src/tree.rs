use anyhow::Result;
use binius_core::linear_code::LinearCode;
use binius_core::reed_solomon::reed_solomon::ReedSolomonCode;
use binius_field::{arithmetic_traits::TaggedMul, BinaryField128b, BinaryField8b, Field};
use binius_hash::{GroestlDigest, GroestlDigestCompression, GroestlHasher, HashDigest};
use sha2::{Digest};
use rs_merkle::{MerkleTree, algorithms::Sha256};

pub type Felt = BinaryField128b;

pub struct DataSquare {
    encoder: ReedSolomonCode<Felt>,
    cols: Vec<Vec<Felt>>,
    width: usize,
}

impl DataSquare {
    // Extend the data square using Reed-Solomon encoding
    pub fn extend(&mut self) -> Result<()> {
        // Create a new tensor to store the extended columns
        let mut q3 = Vec::with_capacity(self.cols.len());

        // Process each column
        for col in self.cols.iter() {
            let mut extended_col = col.clone();
            let new_col = self.encoder.encode(extended_col)?;
            q3.push(new_col);
        }

        // self.cols = extended_cols.clone();
        let mut rows = transpose(self.cols.clone());
        let q3_rows = transpose(q3.clone());
        rows.extend(q3_rows);

        // TODO CRIT
        // let elements: Vec<BinaryField128b> = transpose(extended_cols).iter().flatten().collect();
        let merkle_leaves = rows
            .iter()
            .flatten()
            .map(|elem| Sha256::hash(elem));
            .collect();

        let simple_tree = MerkleTree::<Sha256>::from_leaves(&merkle_leaves);

        let tree = BinaryMerkleTree::<BinaryField128b>::build::<_, GroestlHasher<_>, _>(
            &GroestlDigestCompression::<BinaryField8b>::default(),
            elements.as_slice(),
            extended_cols.len(),
        )?;

        let root = tree.root();
        let merkle_commitment: u128 = root.val();

        // Creating the Dr vector from the entropy of the merkle commitment
        // TODO: find better name
        let mut dr: Vec<Felt> = Vec::new();
        for dr_i in 0..self.width {
            let mut hasher = sha2::Sha256::new();
            hasher.update(merkle_commitment.to_be_bytes());
            // fix this bad rust
            hasher.update(&vec![dr_i as u8]);
            let digest = hasher.finalize();
            // truncate digest to 128 bits to make it into a felt
            // todo: don't make so nested
            dr.push(Felt::new(u128::from_be_bytes(
                digest[0..16].try_into().unwrap(),
            )));
        }

        let q2 = self.commit_and_extend(dr, extended_cols[..self.width].to_vec());
        let q4 = self.commit_and_extend(dr, extended_cols[self.width..].to_vec());
        Ok(())
    }

    pub(crate) fn commit_and_extend(
        &self,
        dr: Vec<Felt>,
        column_data: Vec<Vec<Felt>>,
    ) -> Result<Vec<Vec<Felt>>> {
        let mut new_quadrant: Vec<Vec<Felt>> = Vec::new();
        let width = column_data.len();

        for i in 0..width {
            let mut new_col = Vec::new();
            let original_col = column_data[i].clone();
            for j in 0..width {
                new_col.push(dr[i] * original_col[j]);
            }
            new_quadrant.push(new_col);
        }

        let mut final_quadrant: Vec<Vec<Felt>> = Vec::new();
        for row in transpose(new_quadrant).iter() {
            let encoded = self.encoder.encode(row.clone())?;
            final_quadrant.push(encoded)
        }

        Ok(final_quadrant)
    }
}

pub fn transpose(matrix: Vec<Vec<Felt>>) -> Vec<Vec<Felt>> {
    let mut transposed = Vec::new();
    for i in 0..matrix.len() {
        let mut row = Vec::new();
        for j in 0..matrix.len() {
            row.push(matrix[j][i]);
        }
        transposed.push(row);
    }
    transposed
}
