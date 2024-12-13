use anyhow::{bail, Result};
use binius_core::linear_code::LinearCode;
use binius_core::reed_solomon::reed_solomon::ReedSolomonCode;
use binius_field::BinaryField128b;
use rs_merkle::{algorithms::Sha256, Hasher, MerkleTree};
use sha2::Digest;

pub type Felt = BinaryField128b;

pub struct DataSquare {
    encoder: ReedSolomonCode<Felt>,
    q1_cols: Vec<Vec<Felt>>,
    width: usize,
}

pub struct ExtendedDataSquare {
    cols: Vec<Vec<Felt>>,
    rows: Vec<Vec<Felt>>,
    dr: Vec<Felt>,

    // over columns of (q1, q3)
    x_tree: MerkleTree<Sha256>,
    // over rows of (q1, q2)
    y_tree: MerkleTree<Sha256>,
    // over all quadrants (todo: what representation?)
    // z_tree: MerkleTree<Sha256>,

    //TODO: row_roots, col_roots
}

impl ExtendedDataSquare {
    fn from_cols(
        q1: Vec<Vec<Felt>>,
        q2: Vec<Vec<Felt>>,
        q3: Vec<Vec<Felt>>,
        q4: Vec<Vec<Felt>>,
        dr: Vec<Felt>,
        x_tree: MerkleTree<Sha256>,
        y_tree: MerkleTree<Sha256>,
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

        Self {
            cols,
            rows,
            dr,
            x_tree,
            y_tree,
        }
    }
}

impl DataSquare {
    // Extend the data square using Reed-Solomon encoding
    pub fn extend(&mut self) -> Result<ExtendedDataSquare> {
        let q3_cols = self.create_q3()?;
        let x_tree =
            self.create_tree(transpose(self.q1_cols.clone()), transpose(q3_cols.clone()))?;
        let root = match x_tree.root() {
            Some(r) => r,
            None => bail!("failed to get tree commitment"),
        };

        let dr = self.create_dr(&root);

        let q2_rows = self.commit_and_extend(dr.clone(), self.q1_cols.clone())?;
        let q4_rows = self.commit_and_extend(dr.clone(), q3_cols.clone())?;

        let y_tree = self.create_tree(self.q1_cols.clone(), q3_cols.clone())?;

        let eds = ExtendedDataSquare::from_cols(
            self.q1_cols.clone(),
            transpose(q2_rows),
            q3_cols,
            transpose(q4_rows),
            dr,
            x_tree,
            y_tree,
        );

        Ok(eds)
    }

    pub fn create_q3(&self) -> Result<Vec<Vec<Felt>>> {
        let mut q3: Vec<Vec<Felt>> = Vec::new();
        for col in self.q1_cols.iter() {
            let extended_col = col.clone();
            let new_col = self.encoder.encode(extended_col)?;
            q3.push(new_col);
        }
        Ok(q3)
    }

    pub fn create_tree(
        &self,
        matrix_1: Vec<Vec<Felt>>,
        matrix_2: Vec<Vec<Felt>>,
    ) -> Result<MerkleTree<Sha256>> {
        let mut repr = matrix_1.clone();
        repr.extend(matrix_2);
        // let mut rows = transpose(self.q1_cols.clone());
        // let q3_rows = transpose(q3_cols.clone());
        // rows.extend(q3_rows);

        let merkle_leaves: Vec<[u8; 32]> = repr
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
