use anyhow::{bail, Result};

pub enum Axis {
    Row,
    Col,
}

pub struct Square {
    pub data: Vec<Vec<Vec<u8>>>,
    pub roots: Vec<Vec<u8>>,
    pub axis: Axis,
}

impl Square {
    // TODO: Construct here instead of in DataSquare::new
    pub fn new(data: Vec<Vec<Vec<u8>>>, axis: Axis) -> Self {
        Self {
            data,
            roots: vec![],
            axis,
        }
    }
}

pub struct DataSquare {
    pub row_data: Square,
    pub col_data: Square,

    // TODO: Can we somehow encode this into [`Square`]?
    pub width: usize,
    pub share_size: usize,
}

impl DataSquare {
    pub fn new(data: Vec<Vec<u8>>, share_size: usize) -> Self {
        let width = (data.len() as f64).sqrt().ceil() as usize;
        if width.pow(2) != data.len() {
            panic!("DataSquare must be square");
        }

        // TODO: maybe have this check be done via type system
        for share in data.iter() {
            if share.len() != share_size {
                panic!("All shares must be the same size");
            }
        }

        let mut square_rows = Vec::with_capacity(width);
        for row_idx in 0..width {
            let row = data[row_idx * width..(row_idx + 1) * width].to_vec();
            square_rows.push(row);
        }

        let mut square_col: Vec<Vec<Vec<u8>>> = vec![vec![vec![]; width]; width];
        for col_idx in 0..width {
            for row_idx in 0..width {
                square_col[col_idx][row_idx] = data[row_idx * width + col_idx].clone();
            }
        }

        Self {
            row_data: Square::new(square_rows, Axis::Row),
            col_data: Square::new(square_col, Axis::Col),
            width,
            share_size,
        }
    }

    pub fn extend_square(&mut self, extended_width: usize, filler_share: Vec<u8>) -> Result<()> {
        if filler_share.len() != self.share_size {
            bail!("Filler share must be the same size as the existing shares");
        }

        let new_width = self.width + extended_width;
        let mut new_square_row: Vec<Vec<Vec<u8>>> = Vec::with_capacity(new_width);

        let filler_extended_row: Vec<Vec<u8>> = vec![filler_share.clone(); extended_width];
        let filler_row = vec![filler_share; new_width];

        // extend original rows from first quadrant to new width
        for i in 0..self.width {
            let mut new_row = self.row_data.data[i].clone();
            new_row.extend_from_slice(&filler_extended_row);
            new_square_row.push(new_row);
        }

        // add new rows
        for _ in self.width..new_width {
            new_square_row.push(filler_row.clone());
        }

        self.row_data = Square::new(new_square_row, Axis::Row);

        let mut new_square_col: Vec<Vec<Vec<u8>>> = vec![vec![vec![]; new_width]; new_width];
        for col_idx in 0..new_width {
            for row_idx in 0..new_width {
                new_square_col[col_idx][row_idx] = self.row_data.data[row_idx][col_idx].clone();
            }
        }

        self.col_data = Square::new(new_square_col, Axis::Col);
        self.width = new_width;

        Ok(())
    }
}
