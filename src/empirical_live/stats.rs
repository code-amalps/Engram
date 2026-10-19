//! Paired statistical engine, Holm-Bonferroni multi-comparison correction,
//! and reproducible sample sizing for Engram Phase 2B.
//!
//! # Epistemological Standards & Methodological Invariants
//! 1. **Single Pre-Registered Primary Outcome:** Per-Pair Billed-Token Savings Rate ($S_i$):
//!    $$S_i = \frac{C_i - T_i}{C_i}$$
//!    evaluated strictly on eligible valid matched pairs.
//! 2. **Zero-Baseline Handling:** If $C_i = 0$, $S_i$ is undefined (`None`). The pair is
//!    excluded from the continuous estimator $\bar{S}$ and explicitly tracked in the
//!    denominator count (`n of N`).
//! 3. **Raw Difference Distinction:** Raw Paired Token Difference ($\Delta_i = C_i - T_i$)
//!    is registered as a distinct secondary endpoint. Raw differences are never conflated
//!    or interchanged with ratio savings ($S_i$).
//! 4. **Incomplete Sessions & ITT Sensitivity:** Per-protocol primary estimates are evaluated
//!    strictly on valid completed sessions. Incomplete sessions are evaluated in a separately
//!    labelled Intent-To-Treat (ITT) sensitivity analysis where $T_i$ is imputed as the
//!    scenario budget cap ($B_{\text{max}}$).
//! 5. **Family-Wise Error Rate Control:** Secondary endpoint $p$-values are adjusted using
//!    the step-down Holm-Bonferroni procedure to guarantee $\text{FWER} \le 0.05$.
//! 6. **Iterative Confirmatory Sizing:** Sample size calculations resolve degrees-of-freedom
//!    circularity via an iterative paired-$t$ power algorithm adjusted for $80\%$ $\chi^2$
//!    pilot variance uncertainty.

use serde::{Deserialize, Serialize};

/// Critical value multiplier for upper 80% confidence bound on pilot standard deviation
/// with N_pilot = 5 (degrees of freedom nu = 4):
/// sqrt((N - 1) / chi^2_{0.20, N - 1}) = sqrt(4 / 1.64878) ≈ 1.55745.
pub const PILOT_VARIANCE_UPPER_80_MULTIPLIER: f64 = 1.55745;

/// Default target Minimum Detectable Effect for net token savings rate (20%).
pub const DEFAULT_TARGET_MDE: f64 = 0.20;

/// Default significance level (two-tailed alpha = 0.05).
pub const DEFAULT_ALPHA: f64 = 0.05;

/// Default statistical power (80% power, 1 - beta = 0.80).
pub const DEFAULT_POWER: f64 = 0.80;

/// Default buffer added for infrastructure attrition (+15%).
pub const DEFAULT_ATTRITION_BUFFER: f64 = 0.15;

/// Input observation representing one matched pair trial (Control vs Treatment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedPairInput {
    pub scenario_id: String,
    pub pair_index: usize,
    pub control_billed_tokens: u64,
    pub treatment_billed_tokens: u64,
    pub control_valid: bool,
    pub treatment_valid: bool,
    pub scenario_budget_cap_tokens: u64,
}

/// Evaluated observation record for a single matched pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedPair {
    pub scenario_id: String,
    pub pair_index: usize,
    pub control_tokens: u64,
    pub treatment_tokens: u64,
    pub is_valid_protocol_pair: bool,
    /// Per-pair ratio savings: (C - T) / C. None if C == 0 or pair is invalid.
    pub savings_rate: Option<f64>,
    /// Raw token count difference: C - T.
    pub raw_difference: i64,
    /// Detailed exclusion reason if excluded from primary per-protocol estimator.
    pub exclusion_reason: Option<String>,
}

/// Complete results of the primary paired billed-token statistical analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedTokenSavingsStats {
    pub total_pairs_attempted: usize,
    pub valid_protocol_pairs_count: usize,
    pub zero_denominator_excluded_count: usize,
    pub incomplete_pairs_excluded_count: usize,
    /// Primary Estimator: Sample mean of per-pair savings rate across valid pairs.
    pub mean_savings_rate: Option<f64>,
    /// Sample standard deviation of per-pair savings rate.
    pub std_dev_savings_rate: Option<f64>,
    /// Sample median of per-pair savings rate.
    pub median_savings_rate: Option<f64>,
    /// Interquartile Range (IQR = Q3 - Q1) of per-pair savings rate.
    pub iqr_savings_rate: Option<f64>,
    /// One-sample t-statistic testing H0: mu_S <= 0 vs H1: mu_S > 0.
    pub t_statistic: Option<f64>,
    pub t_test_p_value_one_tailed: Option<f64>,
    pub t_test_p_value_two_tailed: Option<f64>,
    pub cohens_d: Option<f64>,
    /// Wilcoxon signed-rank test statistic W.
    pub wilcoxon_w: Option<f64>,
    pub wilcoxon_p_value: Option<f64>,
    /// Secondary Endpoint: Mean raw token difference (C - T).
    pub mean_raw_difference: f64,
    pub std_dev_raw_difference: f64,
    pub paired_cohens_dz: Option<f64>,
    pub evaluated_pairs: Vec<EvaluatedPair>,
}

/// Result of the Intent-To-Treat (ITT) sensitivity analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitivityAnalysisResult {
    pub total_itt_pairs: usize,
    pub itt_imputed_pairs_count: usize,
    pub itt_mean_savings_rate: Option<f64>,
    pub itt_std_dev_savings_rate: Option<f64>,
    pub itt_median_savings_rate: Option<f64>,
    pub itt_iqr_savings_rate: Option<f64>,
    pub evaluated_itt_pairs: Vec<EvaluatedPair>,
}

/// A secondary hypothesis test subject to Holm-Bonferroni correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecondaryHypothesis {
    pub endpoint_name: String,
    pub unadjusted_p_value: f64,
    pub observed_effect: f64,
}

/// Result of applying Holm-Bonferroni step-down correction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HolmBonferroniResult {
    pub endpoint_name: String,
    pub unadjusted_p_value: f64,
    pub step_rank: usize,
    pub adjusted_alpha_threshold: f64,
    pub adjusted_p_value: f64,
    pub is_significant: bool,
    pub observed_effect: f64,
}

/// Scenario row in the confirmatory sample size planning matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleSizeMatrixEntry {
    pub assumed_sd: f64,
    pub upper_80_bound_sd: f64,
    pub mde: f64,
    pub required_n: usize,
    pub required_n_with_buffer: usize,
}

/// Full sample size planning plan derived from exploratory pilot variance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmatorySampleSizePlan {
    pub pilot_n: usize,
    pub pilot_measured_sd: f64,
    pub inflated_upper_80_sd: f64,
    pub target_mde: f64,
    pub target_alpha: f64,
    pub target_power: f64,
    pub initial_n0: usize,
    pub converged_n: usize,
    pub attrition_buffer_percent: f64,
    pub recommended_sample_size: usize,
    pub sensitivity_matrix: Vec<SampleSizeMatrixEntry>,
}

// ============================================================================
// Mathematical & Distribution Functions
// ============================================================================

/// Error function approximation (Abramowitz & Stegun formula 7.1.26).
/// Maximum absolute error < 1.5e-7.
pub fn erf(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;

    let t = 1.0 / (1.0 + p * x);
    let poly = t * (a1 + t * (a2 + t * (a3 + t * (a4 + t * a5))));
    let result = 1.0 - poly * (-x * x).exp();

    sign * result
}

/// Standard normal cumulative distribution function Phi(z).
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

/// Inverse standard normal cumulative distribution function (probit) Phi^-1(p).
/// Uses Peter Acklam's rational Chebyshev approximation (max error < 1.15e-9).
#[allow(clippy::excessive_precision)]
pub fn normal_quantile(p: f64) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    // Coefficients in rational approximations
    let a = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    let b = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    let c = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    let d = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    let p_low = 0.02425;
    let p_high = 1.0 - p_low;

    if p < p_low {
        // Rational approximation for lower region
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= p_high {
        // Rational approximation for central region
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        // Rational approximation for upper region
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    }
}

/// Student's t quantile (inverse CDF) using a 3-term Cornish-Fisher asymptotic expansion.
///
/// # Domain of Validity & Precision Profile
/// - **Highly Accurate ($<0.05\%$ error):** For degrees of freedom $\nu \ge 8$ and central probabilities
///   $p \in [0.01, 0.99]$. Confirmatory sizing uses $p \in \{0.80, 0.975\}$ with $\nu \ge 8$.
/// - **Asymptotic Truncation Limits:** For small degrees of freedom ($\nu \le 4$) at extreme tails
///   ($p \ge 0.999$), the 3-term asymptotic series underestimates the exact quantile by up to $3.5\%$.
///   It is undefined/unreliable for $\nu \le 2$.
pub fn student_t_quantile(p: f64, nu: f64) -> f64 {
    if nu <= 0.0 {
        return f64::NAN;
    }
    let z = normal_quantile(p);
    let nu2 = nu * nu;
    let nu3 = nu2 * nu;

    let term1 = (z * z * z + z) / (4.0 * nu);
    let term2 = (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / (96.0 * nu2);
    let term3 = (3.0 * z.powi(7) + 19.0 * z.powi(5) + 17.0 * z.powi(3) - 15.0 * z) / (384.0 * nu3);

    z + term1 + term2 + term3
}

/// Regularized incomplete beta function I_x(a, b) via continued fraction (Lentz's method).
pub fn beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    // Symmetry transformation if x > (a + 1)/(a + b + 2)
    if x > (a + 1.0) / (a + b + 2.0) {
        return 1.0 - beta_inc(b, a, 1.0 - x);
    }

    // Factors in front of continued fraction
    let ln_beta = ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b);
    let front = (a * x.ln() + b * (1.0 - x).ln() - ln_beta).exp() / a;

    // Continued fraction via modified Lentz method
    let f_eps = 1e-30;
    let mut c = 1.0;
    let mut d = 1.0 - (a + b) * x / (a + 1.0);
    if d.abs() < f_eps {
        d = f_eps;
    }
    d = 1.0 / d;
    let mut f = d;

    for m in 1..=200 {
        let mf = m as f64;

        // Sub-step 1: even index 2m (d_{2m})
        let numerator_even = mf * (b - mf) * x / ((a + 2.0 * mf - 1.0) * (a + 2.0 * mf));
        d = 1.0 + numerator_even * d;
        if d.abs() < f_eps {
            d = f_eps;
        }
        c = 1.0 + numerator_even / c;
        if c.abs() < f_eps {
            c = f_eps;
        }
        d = 1.0 / d;
        f *= c * d;

        // Sub-step 2: odd index 2m + 1 (d_{2m+1})
        let numerator_odd = -(a + mf) * (a + b + mf) * x / ((a + 2.0 * mf) * (a + 2.0 * mf + 1.0));
        d = 1.0 + numerator_odd * d;
        if d.abs() < f_eps {
            d = f_eps;
        }
        c = 1.0 + numerator_odd / c;
        if c.abs() < f_eps {
            c = f_eps;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;

        if (delta - 1.0).abs() < 1e-15 {
            break;
        }
    }

    front * f
}

/// Natural log of Gamma function ln(Gamma(x)) via Lanczos approximation (g=7, N=9).
#[allow(clippy::excessive_precision)]
pub fn ln_gamma(x: f64) -> f64 {
    if x <= 0.0 {
        return f64::NAN;
    }
    let p = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    let g = 7.0;
    let z = x - 1.0;
    let mut sum = p[0];
    for (i, val) in p.iter().enumerate().skip(1) {
        sum += val / (z + i as f64);
    }
    let t = z + g + 0.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + sum.ln()
}

/// Student's t cumulative distribution function F(t, nu).
pub fn student_t_cdf(t: f64, nu: f64) -> f64 {
    if nu <= 0.0 {
        return f64::NAN;
    }
    let x = nu / (nu + t * t);
    let p = 0.5 * beta_inc(0.5 * nu, 0.5, x);
    if t >= 0.0 { 1.0 - p } else { p }
}

// ============================================================================
// Core Statistical Evaluation Engine
// ============================================================================

pub struct StatisticalEngine;

impl StatisticalEngine {
    /// Calculate primary per-pair token savings rate statistics across matched pairs.
    pub fn evaluate_paired_savings(pairs: &[MatchedPairInput]) -> PairedTokenSavingsStats {
        let total_pairs_attempted = pairs.len();
        let mut evaluated_pairs = Vec::with_capacity(pairs.len());
        let mut valid_savings = Vec::new();
        let mut raw_diffs = Vec::new();
        let mut zero_denom_count = 0usize;
        let mut incomplete_count = 0usize;

        for pair in pairs {
            let is_valid = pair.control_valid && pair.treatment_valid;
            let raw_diff = pair.control_billed_tokens as i64 - pair.treatment_billed_tokens as i64;
            raw_diffs.push(raw_diff as f64);

            if !is_valid {
                incomplete_count += 1;
                evaluated_pairs.push(EvaluatedPair {
                    scenario_id: pair.scenario_id.clone(),
                    pair_index: pair.pair_index,
                    control_tokens: pair.control_billed_tokens,
                    treatment_tokens: pair.treatment_billed_tokens,
                    is_valid_protocol_pair: false,
                    savings_rate: None,
                    raw_difference: raw_diff,
                    exclusion_reason: Some(
                        "Incomplete session (per-protocol exclusion)".to_string(),
                    ),
                });
                continue;
            }

            if pair.control_billed_tokens == 0 {
                zero_denom_count += 1;
                evaluated_pairs.push(EvaluatedPair {
                    scenario_id: pair.scenario_id.clone(),
                    pair_index: pair.pair_index,
                    control_tokens: 0,
                    treatment_tokens: pair.treatment_billed_tokens,
                    is_valid_protocol_pair: false,
                    savings_rate: None,
                    raw_difference: raw_diff,
                    exclusion_reason: Some(
                        "Zero baseline tokens (Undefined denominator)".to_string(),
                    ),
                });
                continue;
            }

            let s_i = (pair.control_billed_tokens as f64 - pair.treatment_billed_tokens as f64)
                / (pair.control_billed_tokens as f64);
            valid_savings.push(s_i);

            evaluated_pairs.push(EvaluatedPair {
                scenario_id: pair.scenario_id.clone(),
                pair_index: pair.pair_index,
                control_tokens: pair.control_billed_tokens,
                treatment_tokens: pair.treatment_billed_tokens,
                is_valid_protocol_pair: true,
                savings_rate: Some(s_i),
                raw_difference: raw_diff,
                exclusion_reason: None,
            });
        }

        let valid_count = valid_savings.len();

        let (mean_s, std_s, median_s, iqr_s, t_stat, p_one, p_two, cohens_d) = if valid_count >= 2 {
            let sum: f64 = valid_savings.iter().sum();
            let mean = sum / valid_count as f64;
            let variance: f64 = valid_savings
                .iter()
                .map(|s| (s - mean).powi(2))
                .sum::<f64>()
                / (valid_count - 1) as f64;
            let sd = variance.sqrt();

            let (median, iqr) = Self::compute_median_and_iqr(&valid_savings);

            let se = sd / (valid_count as f64).sqrt();
            let (t, p_one_tail, p_two_tail, d) = if se > 0.0 {
                let t_val = mean / se;
                let nu = (valid_count - 1) as f64;
                let p_two = 2.0 * (1.0 - student_t_cdf(t_val.abs(), nu));
                let p_one = 1.0 - student_t_cdf(t_val, nu);
                let d_val = mean / sd;
                (Some(t_val), Some(p_one), Some(p_two), Some(d_val))
            } else if mean.abs() < 1e-12 {
                // Exactly zero mean savings and zero variance
                (Some(0.0), Some(0.50), Some(1.00), Some(0.0))
            } else {
                // Positive or negative mean savings with zero sample variance (degenerate variance: undefined t-statistic)
                (None, None, None, None)
            };

            (
                Some(mean),
                Some(sd),
                Some(median),
                Some(iqr),
                t,
                p_one_tail,
                p_two_tail,
                d,
            )
        } else if valid_count == 1 {
            (
                Some(valid_savings[0]),
                None,
                Some(valid_savings[0]),
                Some(0.0),
                None,
                None,
                None,
                None,
            )
        } else {
            (None, None, None, None, None, None, None, None)
        };

        let (wilcoxon_w, wilcoxon_p) = if valid_count >= 2 {
            Self::compute_wilcoxon_signed_rank(&valid_savings)
        } else {
            (None, None)
        };

        // Secondary endpoint: raw difference statistics across all attempted pairs
        let raw_count = raw_diffs.len();
        let (mean_raw, std_raw, dz) = if raw_count >= 2 {
            let sum: f64 = raw_diffs.iter().sum();
            let mean = sum / raw_count as f64;
            let var =
                raw_diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (raw_count - 1) as f64;
            let sd = var.sqrt();
            let dz = if sd > 0.0 { Some(mean / sd) } else { None };
            (mean, sd, dz)
        } else if raw_count == 1 {
            (raw_diffs[0], 0.0, None)
        } else {
            (0.0, 0.0, None)
        };

        PairedTokenSavingsStats {
            total_pairs_attempted,
            valid_protocol_pairs_count: valid_count,
            zero_denominator_excluded_count: zero_denom_count,
            incomplete_pairs_excluded_count: incomplete_count,
            mean_savings_rate: mean_s,
            std_dev_savings_rate: std_s,
            median_savings_rate: median_s,
            iqr_savings_rate: iqr_s,
            t_statistic: t_stat,
            t_test_p_value_one_tailed: p_one,
            t_test_p_value_two_tailed: p_two,
            cohens_d,
            wilcoxon_w,
            wilcoxon_p_value: wilcoxon_p,
            mean_raw_difference: mean_raw,
            std_dev_raw_difference: std_raw,
            paired_cohens_dz: dz,
            evaluated_pairs,
        }
    }

    /// Calculate Intent-To-Treat (ITT) sensitivity analysis imputing incomplete sessions at budget cap.
    pub fn evaluate_itt_sensitivity(pairs: &[MatchedPairInput]) -> SensitivityAnalysisResult {
        let mut itt_savings = Vec::new();
        let mut evaluated_itt = Vec::new();
        let mut imputed_count = 0usize;

        for pair in pairs {
            let (t_tokens, was_imputed) = if pair.treatment_valid {
                (pair.treatment_billed_tokens, false)
            } else {
                imputed_count += 1;
                (pair.scenario_budget_cap_tokens, true)
            };

            if pair.control_valid && pair.control_billed_tokens > 0 {
                let s_itt = (pair.control_billed_tokens as f64 - t_tokens as f64)
                    / (pair.control_billed_tokens as f64);
                itt_savings.push(s_itt);

                evaluated_itt.push(EvaluatedPair {
                    scenario_id: pair.scenario_id.clone(),
                    pair_index: pair.pair_index,
                    control_tokens: pair.control_billed_tokens,
                    treatment_tokens: t_tokens,
                    is_valid_protocol_pair: pair.control_valid && pair.treatment_valid,
                    savings_rate: Some(s_itt),
                    raw_difference: pair.control_billed_tokens as i64 - t_tokens as i64,
                    exclusion_reason: if was_imputed {
                        Some("Imputed at scenario budget cap (ITT sensitivity)".to_string())
                    } else {
                        None
                    },
                });
            } else {
                evaluated_itt.push(EvaluatedPair {
                    scenario_id: pair.scenario_id.clone(),
                    pair_index: pair.pair_index,
                    control_tokens: pair.control_billed_tokens,
                    treatment_tokens: t_tokens,
                    is_valid_protocol_pair: false,
                    savings_rate: None,
                    raw_difference: pair.control_billed_tokens as i64 - t_tokens as i64,
                    exclusion_reason: Some(
                        "Control invalid or zero baseline (ITT exclusion)".to_string(),
                    ),
                });
            }
        }

        let n = itt_savings.len();
        let (mean, sd, med, iqr) = if n >= 2 {
            let sum: f64 = itt_savings.iter().sum();
            let m = sum / n as f64;
            let var = itt_savings.iter().map(|s| (s - m).powi(2)).sum::<f64>() / (n - 1) as f64;
            let (med, iqr) = Self::compute_median_and_iqr(&itt_savings);
            (Some(m), Some(var.sqrt()), Some(med), Some(iqr))
        } else if n == 1 {
            (Some(itt_savings[0]), None, Some(itt_savings[0]), Some(0.0))
        } else {
            (None, None, None, None)
        };

        SensitivityAnalysisResult {
            total_itt_pairs: pairs.len(),
            itt_imputed_pairs_count: imputed_count,
            itt_mean_savings_rate: mean,
            itt_std_dev_savings_rate: sd,
            itt_median_savings_rate: med,
            itt_iqr_savings_rate: iqr,
            evaluated_itt_pairs: evaluated_itt,
        }
    }

    /// Apply Holm-Bonferroni step-down correction across secondary hypotheses to control FWER <= alpha.
    pub fn holm_bonferroni_adjust(
        hypotheses: &[SecondaryHypothesis],
        alpha: f64,
    ) -> Vec<HolmBonferroniResult> {
        let m = hypotheses.len();
        if m == 0 {
            return Vec::new();
        }

        // Partition into valid [0.0, 1.0] and invalid/NaN p-values
        let mut valid: Vec<(usize, &SecondaryHypothesis)> = Vec::new();
        let mut invalid: Vec<HolmBonferroniResult> = Vec::new();

        for (idx, hyp) in hypotheses.iter().enumerate() {
            if hyp.unadjusted_p_value.is_nan()
                || hyp.unadjusted_p_value < 0.0
                || hyp.unadjusted_p_value > 1.0
            {
                invalid.push(HolmBonferroniResult {
                    endpoint_name: hyp.endpoint_name.clone(),
                    unadjusted_p_value: hyp.unadjusted_p_value,
                    step_rank: 0,
                    adjusted_alpha_threshold: 0.0,
                    adjusted_p_value: 1.0,
                    is_significant: false,
                    observed_effect: hyp.observed_effect,
                });
            } else {
                valid.push((idx, hyp));
            }
        }

        // Sort valid hypotheses by unadjusted p-value in ascending order
        valid.sort_by(|a, b| {
            a.1.unadjusted_p_value
                .partial_cmp(&b.1.unadjusted_p_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let valid_m = valid.len();
        let mut results = Vec::with_capacity(m);
        let mut cumulative_max_p: f64 = 0.0;
        let mut reject_remaining = true;

        for (rank_idx, (_orig_idx, hyp)) in valid.iter().enumerate() {
            let step_rank = rank_idx + 1; // 1-indexed rank
            let divisor = (valid_m - rank_idx) as f64;
            let threshold = alpha / divisor;

            // Step-down rejection rule: once a hypothesis fails to be rejected, all subsequent hypotheses fail
            let is_sig = if reject_remaining && hyp.unadjusted_p_value <= threshold {
                true
            } else {
                reject_remaining = false;
                false
            };

            // Adjusted p-value: p_adj = min(1.0, max(p_adj_{k-1}, divisor * p_k))
            let raw_adj = divisor * hyp.unadjusted_p_value;
            let adj_p = (cumulative_max_p.max(raw_adj)).min(1.0);
            cumulative_max_p = adj_p;

            results.push(HolmBonferroniResult {
                endpoint_name: hyp.endpoint_name.clone(),
                unadjusted_p_value: hyp.unadjusted_p_value,
                step_rank,
                adjusted_alpha_threshold: threshold,
                adjusted_p_value: adj_p,
                is_significant: is_sig,
                observed_effect: hyp.observed_effect,
            });
        }

        results.extend(invalid);
        results
    }

    /// Derive confirmatory sample size using the iterative paired-t power algorithm.
    pub fn calculate_confirmatory_sample_size(
        pilot_sd: f64,
        mde: f64,
        alpha: f64,
        power: f64,
        attrition_buffer: f64,
    ) -> ConfirmatorySampleSizePlan {
        let pilot_n = 5;
        let inflated_sd = pilot_sd * PILOT_VARIANCE_UPPER_80_MULTIPLIER;
        let z_alpha = normal_quantile(1.0 - alpha / 2.0);
        let z_beta = normal_quantile(power);

        // Step 0: Normal approximation initialization
        let ratio_0 = mde / inflated_sd;
        let n0 = ((z_alpha + z_beta) / ratio_0).powi(2).ceil() as usize;

        // Iterative refinement with Student's t distribution
        let mut n_k = n0.max(2);
        let max_iterations = 100;
        let mut iteration = 0;

        while iteration < max_iterations {
            let df = (n_k - 1) as f64;
            let t_alpha = student_t_quantile(1.0 - alpha / 2.0, df);
            let t_beta = student_t_quantile(power, df);
            let n_next = ((t_alpha + t_beta) / ratio_0).powi(2).ceil() as usize;

            if n_next == n_k {
                break;
            }
            n_k = n_next.max(2);
            iteration += 1;
        }

        let final_n = (n_k as f64 * (1.0 + attrition_buffer)).ceil() as usize;

        // Build illustrative sensitivity matrix across standard SDs and MDEs
        let matrix_sds = [0.15, 0.20, 0.25, 0.30];
        let matrix_mdes = [0.15, 0.20, 0.25];
        let mut sensitivity_matrix = Vec::new();

        for s in matrix_sds {
            let s_upper = s * PILOT_VARIANCE_UPPER_80_MULTIPLIER;
            for m in matrix_mdes {
                let ratio = m / s_upper;
                let mut n_iter = ((z_alpha + z_beta) / ratio).powi(2).ceil() as usize;
                for _ in 0..50 {
                    let df = (n_iter - 1) as f64;
                    let t_a = student_t_quantile(1.0 - alpha / 2.0, df);
                    let t_b = student_t_quantile(power, df);
                    let next = ((t_a + t_b) / ratio).powi(2).ceil() as usize;
                    if next == n_iter {
                        break;
                    }
                    n_iter = next.max(2);
                }
                sensitivity_matrix.push(SampleSizeMatrixEntry {
                    assumed_sd: s,
                    upper_80_bound_sd: (s_upper * 1000.0).round() / 1000.0,
                    mde: m,
                    required_n: n_iter,
                    required_n_with_buffer: (n_iter as f64 * (1.0 + attrition_buffer)).ceil()
                        as usize,
                });
            }
        }

        ConfirmatorySampleSizePlan {
            pilot_n,
            pilot_measured_sd: pilot_sd,
            inflated_upper_80_sd: inflated_sd,
            target_mde: mde,
            target_alpha: alpha,
            target_power: power,
            initial_n0: n0,
            converged_n: n_k,
            attrition_buffer_percent: attrition_buffer * 100.0,
            recommended_sample_size: final_n,
            sensitivity_matrix,
        }
    }

    /// Compute median and interquartile range (IQR).
    fn compute_median_and_iqr(values: &[f64]) -> (f64, f64) {
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = sorted.len();
        let median = if n % 2 == 1 {
            sorted[n / 2]
        } else {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        };

        let q1 = Self::percentile_sorted(&sorted, 0.25);
        let q3 = Self::percentile_sorted(&sorted, 0.75);
        (median, q3 - q1)
    }

    fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
        let n = sorted.len();
        if n == 1 {
            return sorted[0];
        }
        let rank = p * (n - 1) as f64;
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        let weight = rank - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }

    /// Compute Wilcoxon signed-rank test statistic W and asymptotic p-value.
    ///
    /// # Methodological Assumptions & Epistemological Boundaries
    /// - **Symmetry Assumption:** The null hypothesis $H_0$ formally tests that the distribution
    ///   of paired differences is symmetric around zero. It is a test of the population median
    ///   only under the assumption that the underlying difference distribution is symmetric;
    ///   otherwise, it estimates the Hodges–Lehmann pseudomedian (median of Walsh averages).
    /// - **Small-Sample Bounds:** For small samples ($N \le 5$), the test uses an asymptotic normal
    ///   approximation with signed continuity correction. Under exact permutation theory with $N=5$,
    ///   the minimum possible two-tailed $p$-value is $2/32 = 0.0625$, meaning significance at
    ///   $\alpha = 0.05$ cannot be achieved on an exploratory $N=5$ cohort.
    fn compute_wilcoxon_signed_rank(values: &[f64]) -> (Option<f64>, Option<f64>) {
        // Compute differences against H0: symmetric difference around 0
        let non_zero: Vec<f64> = values.iter().copied().filter(|v| v.abs() > 1e-12).collect();
        let n = non_zero.len();
        if n < 2 {
            return (None, None);
        }

        // Sort by absolute value
        let mut indexed: Vec<(usize, f64, f64)> = non_zero
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v, v.abs()))
            .collect();
        indexed.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        // Assign ranks with average rank for ties and calculate tie penalty
        let mut ranks = vec![0.0; n];
        let mut tie_penalty = 0.0;
        let mut i = 0;
        while i < n {
            let mut j = i;
            while j < n - 1 && (indexed[j].2 - indexed[j + 1].2).abs() < 1e-12 {
                j += 1;
            }
            let tie_count = (j - i + 1) as f64;
            if tie_count > 1.0 {
                tie_penalty += (tie_count.powi(3) - tie_count) / 48.0;
            }
            let avg_rank = (i + j + 2) as f64 / 2.0; // 1-indexed average
            for k in i..=j {
                ranks[indexed[k].0] = avg_rank;
            }
            i = j + 1;
        }

        let mut w_plus = 0.0;
        let mut w_minus = 0.0;
        for (i, val) in non_zero.iter().enumerate() {
            if *val > 0.0 {
                w_plus += ranks[i];
            } else {
                w_minus += ranks[i];
            }
        }

        let _w = w_plus.min(w_minus);
        let n_f = n as f64;
        let mean_w = n_f * (n_f + 1.0) / 4.0;
        let unadjusted_var = n_f * (n_f + 1.0) * (2.0 * n_f + 1.0) / 24.0;
        let var_w = (unadjusted_var - tie_penalty).max(1e-15);
        let sd_w = var_w.sqrt();

        // Signed continuity correction: shrink towards mean
        let diff = w_plus - mean_w;
        let z = if sd_w > 0.0 {
            if diff.abs() > 0.5 {
                diff.signum() * (diff.abs() - 0.5) / sd_w
            } else {
                0.0
            }
        } else {
            0.0
        };

        let p_val = 2.0 * (1.0 - normal_cdf(z.abs()));
        (Some(w_plus), Some(p_val))
    }
}

/// Convenience wrapper for [`StatisticalEngine::evaluate_paired_savings`].
pub fn evaluate_paired_savings(pairs: &[MatchedPairInput]) -> PairedTokenSavingsStats {
    StatisticalEngine::evaluate_paired_savings(pairs)
}

/// Convenience wrapper for [`StatisticalEngine::evaluate_itt_sensitivity`].
pub fn evaluate_itt_sensitivity(pairs: &[MatchedPairInput]) -> SensitivityAnalysisResult {
    StatisticalEngine::evaluate_itt_sensitivity(pairs)
}

/// Convenience wrapper for [`StatisticalEngine::holm_bonferroni_adjust`].
pub fn holm_bonferroni_adjust(
    hypotheses: &[SecondaryHypothesis],
    alpha: f64,
) -> Vec<HolmBonferroniResult> {
    StatisticalEngine::holm_bonferroni_adjust(hypotheses, alpha)
}

/// Convenience wrapper for [`StatisticalEngine::calculate_confirmatory_sample_size`].
pub fn calculate_confirmatory_sample_size(
    pilot_sd: f64,
    mde: f64,
    alpha: f64,
    power: f64,
    attrition_buffer: f64,
) -> ConfirmatorySampleSizePlan {
    StatisticalEngine::calculate_confirmatory_sample_size(
        pilot_sd,
        mde,
        alpha,
        power,
        attrition_buffer,
    )
}
