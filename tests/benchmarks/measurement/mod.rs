//! Measurement benchmark suite helpers and definitions.

pub const MEASUREMENT_VERSION: &str = "1.1.0";

/// Validate that incremental analysis achieves a statistically meaningful surface reduction.
pub fn calculate_surface_reduction(cold_files: usize, warm_files: usize) -> f64 {
    if cold_files == 0 {
        return 0.0;
    }
    ((cold_files.saturating_sub(warm_files)) as f64 / cold_files as f64) * 100.0
}

/// Calculate measured speedup factor between cold and warm incremental runs.
pub fn calculate_speedup_factor(cold_ms: f64, warm_ms: f64) -> f64 {
    if warm_ms > 0.0001 {
        (cold_ms / warm_ms * 100.0).round() / 100.0
    } else {
        1.0
    }
}
