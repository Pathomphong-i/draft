use crate::error::ConvergenceError;
use daft_dimension::{DimensionLockGuard, DimensionManager};

pub struct MultiDimensionLockGuard {
    _guards: Vec<DimensionLockGuard>,
}

impl MultiDimensionLockGuard {
    pub fn acquire(
        dim_manager: &DimensionManager,
        mut dims: Vec<String>,
    ) -> Result<Self, ConvergenceError> {
        dims.sort();
        dims.dedup();
        let mut guards = Vec::with_capacity(dims.len());
        for dim in dims {
            if dim == "mainline" {
                continue;
            }
            if dim_manager.dimensions_dir().join(&dim).exists() {
                let guard = DimensionLockGuard::acquire_timeout(
                    dim_manager.dimensions_dir(),
                    &dim,
                    None,
                    "convergence",
                    std::time::Duration::from_secs(5),
                )
                .map_err(|e| ConvergenceError::LockFailure(dim.clone(), e.to_string()))?;
                guards.push(guard);
            }
        }
        Ok(Self { _guards: guards })
    }
}
