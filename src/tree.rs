use anyhow::{bail, Result};
use binius_core::linear_code::LinearCode;
use binius_core::reed_solomon::reed_solomon::ReedSolomonCode;
use binius_field::BinaryField128b;
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree};
use sha2::Digest;

pub type Felt = BinaryField128b;

pub struct DataSquare {
    encoder: ReedSolomonCode<Felt>,
    cols: Vec<Vec<Felt>>,
    width: usize,
}

pub struct ExtendedDataSquare {
    cols: Vec<Vec<Felt>>,
    rows: Vec<Vec<Felt>>,
    dr: Vec<Felt>,
    //TODO: row_roots, col_roots
}

impl ExtendedDataSquare {
    fn from_cols(
        q1: Vec<Vec<Felt>>,
        q2: Vec<Vec<Felt>>,
        q3: Vec<Vec<Felt>>,
        q4: Vec<Vec<Felt>>,
        dr: Vec<Felt>,
    ) -> Self {
        // step 1: combine q1 and q3
        let mut left_cols = q1.clone();
        for col in left_cols.iter_mut().zip(q3) {
            col.0.extend(col.1);
        }

        // step 2: combine q2 and q4
        let mut right_cols = q2.clone();
        for col in right_cols.iter_mut().zip(q4) {
            col.0.extend(col.1);
        }

        // step 3: combine left and right cols
        let mut cols = left_cols.clone();
        cols.extend(right_cols);

        let rows = transpose(cols.clone());

        Self { cols, rows, dr }
    }
}

impl DataSquare {
    // Extend the data square using Reed-Solomon encoding
    pub fn extend(&mut self) -> Result<ExtendedDataSquare> {
        let q3_cols = self.create_q3()?;
        let tree = self.create_tree(q3_cols.clone())?;
        let root = match tree.root() {
            Some(r) => r,
            None => bail!("failed to get tree commitment"),
        };

        let dr = self.create_dr(&root);

        let q2_rows = self.commit_and_extend(dr.clone(), self.cols.clone());
        let q4_rows = self.commit_and_extend(dr, q3_cols);
        Ok(())
    }

    pub fn create_q3(&self) -> Result<Vec<Vec<Felt>>> {
        let mut q3: Vec<Vec<Felt>> = Vec::new();
        for col in self.cols.iter() {
            let extended_col = col.clone();
            let new_col = self.encoder.encode(extended_col)?;
            q3.push(new_col);
        }
        Ok(q3)
    }

    pub fn create_tree(&self, q3_cols: Vec<Vec<Felt>>) -> Result<MerkleTree<Sha256>> {
        let mut rows = transpose(self.cols.clone());
        let q3_rows = transpose(q3_cols.clone());
        rows.extend(q3_rows);

        let merkle_leaves: Vec<[u8; 32]> = rows
            .iter()
            .flatten()
            .map(|elem| Sha256::hash(elem.val().to_be_bytes().as_ref()))
            .collect();

        Ok(MerkleTree::<Sha256>::from_leaves(&merkle_leaves))
    }

    pub fn create_dr(&self, tree_commitment: &[u8; 32]) -> Vec<Felt> {
        let mut dr: Vec<Felt> = Vec::new();
        for dr_i in 0..self.width {
            let mut hasher = sha2::Sha256::new();
            hasher.update(tree_commitment);
            hasher.update(dr_i.to_be_bytes());
            let digest = hasher.finalize();
            // truncate digest to 128 bits to make it into a felt
            // todo: don't make so nested
            dr.push(Felt::new(u128::from_be_bytes(
                digest[0..16].try_into().unwrap(),
            )));
        }
        dr
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
            for elem in original_col.iter() {
                new_col.push(dr[i] * elem);
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
        for col in matrix.iter() {
            row.push(col[i]);
        }
        transposed.push(row);
    }
    transposed
}
