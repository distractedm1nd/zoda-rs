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
    z_tree: MerkleTree<Sha256>,
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
        z_tree: MerkleTree<Sha256>,
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

        let rows = transpose(&cols);

        Self {
            cols,
            rows,
            dr,
            x_tree,
            z_tree,
        }
    }
}

impl DataSquare {
    // Extend the data square using Reed-Solomon encoding
    pub fn extend(&mut self) -> Result<ExtendedDataSquare> {
        let q3_cols = self.create_q3()?;
        let x_tree = self.create_tree(vec![&transpose(&self.q1_cols), &transpose(&q3_cols)])?;
        let root = match x_tree.root() {
            Some(r) => r,
            None => bail!("failed to get tree commitment"),
        };

        let dr = self.create_dr(&root);

        let mut q1_dr_cols = self.q1_cols.clone();
        let mut q3_dr_cols = q3_cols.clone();
        self.multiply_dr(&mut q1_dr_cols, &dr);
        self.multiply_dr(&mut q3_dr_cols, &dr);

        let q2_rows = self.extend_quadrant(&q1_dr_cols)?;
        let q4_rows = self.extend_quadrant(&q3_dr_cols)?;

        let z_tree = self.create_tree(vec![&q1_dr_cols, &transpose(&q2_rows), &q3_dr_cols, &transpose(&q4_rows)])?;

        let eds = ExtendedDataSquare::from_cols(
            self.q1_cols.clone(),
            transpose(&q2_rows),
            q3_cols,
            transpose(&q4_rows),
            dr,
            x_tree,
            z_tree,
        );

        Ok(eds)
    }

    pub fn multiply_dr(&self, matrix: &mut [Vec<Felt>], dr: &[Felt]) {
        for (i, repr) in matrix.iter_mut().enumerate() {
            repr.iter_mut().for_each(|elem| *elem *= dr[i]);
        }
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
        matrices: Vec<&[Vec<Felt>]>,
    ) -> Result<MerkleTree<Sha256>> {
        let repr = matrices.iter().flat_map(|matrix| matrix.iter()).collect::<Vec<_>>();

        let merkle_leaves: Vec<[u8; 32]> = repr
            .iter()
            .flat_map(|vec| vec.iter())
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

    pub(crate) fn extend_quadrant(&self, column_data: &[Vec<Felt>]) -> Result<Vec<Vec<Felt>>> {
        let mut extended_quadrant: Vec<Vec<Felt>> = Vec::new();
        for row in transpose(column_data).iter() {
            let encoded = self.encoder.encode(row.clone())?;
            extended_quadrant.push(encoded)
        }

        Ok(extended_quadrant)
    }
}

pub fn transpose(matrix: &[Vec<Felt>]) -> Vec<Vec<Felt>> {
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

pub fn transpose_and_flatten(matrix: &[Vec<Felt>]) -> Vec<Felt> {
    let mut transposed = Vec::new();
    for i in 0..matrix.len() {
        for col in matrix.iter() {
            transposed.push(col[i]);
        }
    }
    transposed
}
