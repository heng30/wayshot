use ndarray::{Array1, Array2, ArrayView2};
use std::fmt;

#[derive(Debug)]
pub struct LinalgError(String);

impl fmt::Display for LinalgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LinalgError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UPLO {
    Lower,
    #[expect(dead_code)]
    Upper,
}

pub trait Inverse {
    fn inv(&self) -> Result<Array2<f64>, LinalgError>;
}

impl Inverse for ArrayView2<'_, f64> {
    fn inv(&self) -> Result<Array2<f64>, LinalgError> {
        let n = self.nrows();
        if self.ncols() != n {
            return Err(LinalgError("matrix must be square".into()));
        }
        gaussian_elimination_inv(self, n)
    }
}

impl Inverse for Array2<f64> {
    fn inv(&self) -> Result<Array2<f64>, LinalgError> {
        self.view().inv()
    }
}

fn gaussian_elimination_inv(a: &ArrayView2<f64>, n: usize) -> Result<Array2<f64>, LinalgError> {
    let mut aug = Array2::<f64>::zeros((n, 2 * n));
    for i in 0..n {
        for j in 0..n {
            aug[[i, j]] = a[[i, j]];
        }
        aug[[i, n + i]] = 1.0;
    }

    for col in 0..n {
        let mut pivot_row = col;
        let mut pivot_val = aug[[col, col]].abs();
        for row in (col + 1)..n {
            let val = aug[[row, col]].abs();
            if val > pivot_val {
                pivot_val = val;
                pivot_row = row;
            }
        }
        if pivot_val < 1e-12 {
            return Err(LinalgError("singular matrix".into()));
        }
        if pivot_row != col {
            for j in 0..2 * n {
                let tmp = aug[[col, j]];
                aug[[col, j]] = aug[[pivot_row, j]];
                aug[[pivot_row, j]] = tmp;
            }
        }
        let pivot = aug[[col, col]];
        for j in 0..2 * n {
            aug[[col, j]] /= pivot;
        }
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = aug[[row, col]];
            if factor.abs() < 1e-15 {
                continue;
            }
            for j in 0..2 * n {
                aug[[row, j]] -= factor * aug[[col, j]];
            }
        }
    }

    let mut result = Array2::<f64>::zeros((n, n));
    for i in 0..n {
        for j in 0..n {
            result[[i, j]] = aug[[i, n + j]];
        }
    }
    Ok(result)
}

pub trait Eigh {
    fn eigh(&self, uplo: UPLO) -> Result<(Array1<f64>, (Array2<f64>, Array2<f64>)), LinalgError>;
}

impl Eigh for (Array2<f64>, Array2<f64>) {
    fn eigh(&self, uplo: UPLO) -> Result<(Array1<f64>, (Array2<f64>, Array2<f64>)), LinalgError> {
        let (a, b) = self;
        let n = a.nrows();
        if a.ncols() != n || b.nrows() != n || b.ncols() != n {
            return Err(LinalgError("matrices must be square and same size".into()));
        }
        generalized_eigh(a, b, uplo, n)
    }
}

fn generalized_eigh(
    a: &Array2<f64>,
    b: &Array2<f64>,
    uplo: UPLO,
    n: usize,
) -> Result<(Array1<f64>, (Array2<f64>, Array2<f64>)), LinalgError> {
    let l = cholesky(b, uplo, n)?;
    let l_inv = l.view().inv()?;
    let l_inv_t = l_inv.t().to_owned();
    let c = l_inv.dot(&a.dot(&l_inv_t));
    let (eigenvalues, eigenvectors_c) = symmetric_eigh(&c, UPLO::Lower, n)?;
    let eigenvectors = l_inv_t.dot(&eigenvectors_c);
    Ok((eigenvalues, (eigenvectors, b.clone())))
}

fn cholesky(a: &Array2<f64>, uplo: UPLO, n: usize) -> Result<Array2<f64>, LinalgError> {
    let mut l = Array2::<f64>::zeros((n, n));
    match uplo {
        UPLO::Lower => {
            for i in 0..n {
                for j in 0..=i {
                    let sum = (0..j).map(|k| l[[i, k]] * l[[j, k]]).sum::<f64>();
                    if j == i {
                        let diag = a[[i, i]] - sum;
                        if diag <= 0.0 {
                            return Err(LinalgError("matrix not positive definite".into()));
                        }
                        l[[i, j]] = diag.sqrt();
                    } else {
                        l[[i, j]] = (a[[i, j]] - sum) / l[[j, j]];
                    }
                }
            }
        }
        UPLO::Upper => {
            for i in 0..n {
                for j in i..n {
                    let sum = (0..i).map(|k| l[[k, i]] * l[[k, j]]).sum::<f64>();
                    if j == i {
                        let diag = a[[i, i]] - sum;
                        if diag <= 0.0 {
                            return Err(LinalgError("matrix not positive definite".into()));
                        }
                        l[[i, j]] = diag.sqrt();
                    } else {
                        l[[i, j]] = (a[[i, j]] - sum) / l[[i, i]];
                    }
                }
            }
        }
    }
    Ok(l)
}

fn symmetric_eigh(
    a: &Array2<f64>,
    _uplo: UPLO,
    n: usize,
) -> Result<(Array1<f64>, Array2<f64>), LinalgError> {
    let mut mat = a.clone();
    let mut v = Array2::<f64>::eye(n);

    const MAX_SWEEPS: usize = 100;
    const TOL: f64 = 1e-12;

    for _ in 0..MAX_SWEEPS {
        let mut off_diag = 0.0f64;
        for i in 0..n {
            for j in (i + 1)..n {
                off_diag += mat[[i, j]] * mat[[i, j]];
            }
        }
        if off_diag < TOL {
            break;
        }

        for p in 0..n {
            for q in (p + 1)..n {
                let apq = mat[[p, q]];
                if apq.abs() < 1e-15 {
                    continue;
                }
                let app = mat[[p, p]];
                let aqq = mat[[q, q]];
                let theta = if (app - aqq).abs() < 1e-15 {
                    std::f64::consts::FRAC_PI_4
                } else {
                    0.5 * ((aqq - app) / (2.0 * apq)).atan()
                };
                let c = theta.cos();
                let s = theta.sin();

                for i in 0..n {
                    if i == p || i == q {
                        continue;
                    }
                    let aip = mat[[i, p]];
                    let aiq = mat[[i, q]];
                    mat[[i, p]] = c * aip + s * aiq;
                    mat[[p, i]] = mat[[i, p]];
                    mat[[i, q]] = -s * aip + c * aiq;
                    mat[[q, i]] = mat[[i, q]];
                }
                mat[[p, p]] = c * c * app + 2.0 * s * c * apq + s * s * aqq;
                mat[[q, q]] = s * s * app - 2.0 * s * c * apq + c * c * aqq;
                mat[[p, q]] = 0.0;
                mat[[q, p]] = 0.0;

                for i in 0..n {
                    let vip = v[[i, p]];
                    let viq = v[[i, q]];
                    v[[i, p]] = c * vip + s * viq;
                    v[[i, q]] = -s * vip + c * viq;
                }
            }
        }
    }

    let mut eigenvalues: Vec<(f64, usize)> = (0..n).map(|i| (mat[[i, i]], i)).collect();
    eigenvalues.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut sorted_vals = Array1::<f64>::zeros(n);
    let mut sorted_vecs = Array2::<f64>::zeros((n, n));
    for (idx, &(val, src_col)) in eigenvalues.iter().enumerate() {
        sorted_vals[idx] = val;
        for row in 0..n {
            sorted_vecs[[row, idx]] = v[[row, src_col]];
        }
    }

    Ok((sorted_vals, sorted_vecs))
}
