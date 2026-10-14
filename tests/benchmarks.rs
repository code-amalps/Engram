//! Verification Suite for Benchmark Definitions (Measurement & North-Star modules).

#[path = "benchmarks/measurement/mod.rs"]
pub mod measurement;

#[path = "benchmarks/north_star/mod.rs"]
pub mod north_star;

#[test]
fn test_measurement_framework_definitions_and_calculations() {
    assert_eq!(measurement::MEASUREMENT_VERSION, "1.1.0");

    let reduction = measurement::calculate_surface_reduction(10, 1);
    assert!((reduction - 90.0).abs() < f64::EPSILON, "10 -> 1 files must be 90% reduction");

    let zero_reduction = measurement::calculate_surface_reduction(0, 0);
    assert_eq!(zero_reduction, 0.0);

    let speedup = measurement::calculate_speedup_factor(100.0, 50.0);
    assert_eq!(speedup, 2.0);

    let instant_speedup = measurement::calculate_speedup_factor(10.0, 0.0);
    assert_eq!(instant_speedup, 1.0);
}

#[test]
fn test_north_star_benchmark_definitions_and_stages() {
    assert_eq!(north_star::NORTH_STAR_VERSION, "1.1.0");

    let stages = north_star::NorthStarStage::all_stages();
    assert_eq!(stages.len(), 12);
    assert_eq!(stages[0], north_star::NorthStarStage::TaskInitiation);
    assert_eq!(stages[11], north_star::NorthStarStage::NextPreflightGrounding);
}
