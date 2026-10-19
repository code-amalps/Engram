//! Benchmark report serialization, formatting, and epistemological disclosure
//! for Engram Phase 2B live and mock empirical evaluations.
//!
//! # Epistemological Standards & Methodological Invariants
//! 1. **Mandatory Pilot Disclaimer:** Every report artifact derived from an exploratory
//!    pilot ($N=5$) must visibly prominently carry the exploratory disclaimer:
//!    *"The N=5 pilot study is strictly exploratory and underpowered to prove statistical significance. Reported lifts, effect sizes, and medians are descriptive only."*
//! 2. **Complete Denominator Tracking:** The report explicitly accounts for 100% of attempted
//!    pairs, separating per-protocol valid pairs from zero-denominator exclusions and incomplete
//!    sessions (`token_savings_denominator: n of N`).
//! 3. **Primary vs Secondary Separation:** The pre-registered primary endpoint
//!    (per-pair ratio savings $S_i$) is formatted distinctly from raw token differences ($\Delta_i$)
//!    and other secondary endpoints.
//! 4. **Multiplicity Correction Disclosure:** Secondary endpoints are presented with unadjusted
//!    $p$-values, Holm-Bonferroni adjusted thresholds, adjusted $p$-values, and explicit FWER significance.
//! 5. **Sensitivity Analysis Disclosure:** ITT sensitivity analysis imputing incomplete sessions
//!    at $B_{\text{max}}$ is displayed side-by-side with per-protocol results to evaluate survivorship bias.
//! 6. **Full Provenance:** Every report includes engine revision, corpus digest, provider, model ID,
//!    temperature, prompt digest, and ISO 8601 timestamp.

use crate::empirical_live::stats::{
    ConfirmatorySampleSizePlan, HolmBonferroniResult, PairedTokenSavingsStats,
    SensitivityAnalysisResult,
};
use serde::{Deserialize, Serialize};

/// Mandatory disclaimer required on all exploratory pilot reports.
pub const EXPLORATORY_PILOT_DISCLAIMER: &str = "The N=5 pilot study is strictly exploratory and underpowered to prove statistical significance. Reported lifts, effect sizes, and medians are descriptive only.";

/// Provenance metadata recorded for live or mock empirical benchmark evaluations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkProvenance {
    pub evaluation_type: String,
    pub engine_version: String,
    pub engine_revision: String,
    pub corpus_digest: String,
    pub timestamp_utc: String,
    pub provider: String,
    pub model_id: String,
    pub temperature: f64,
    pub prompt_digest: String,
}

/// Complete empirical benchmark evaluation report for Engram Phase 2B.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveBenchmarkReport {
    pub provenance: BenchmarkProvenance,
    pub exploratory_disclaimer: String,
    pub total_scenarios_evaluated: usize,
    pub total_matched_pairs_attempted: usize,
    /// Explicit denominator tracking string: e.g. "4 of 5 pairs"
    pub token_savings_denominator: String,
    pub primary_stats: PairedTokenSavingsStats,
    pub secondary_holm_bonferroni: Vec<HolmBonferroniResult>,
    pub itt_sensitivity: SensitivityAnalysisResult,
    pub sample_size_plan: ConfirmatorySampleSizePlan,
}

impl LiveBenchmarkReport {
    /// Construct a new report ensuring mandatory disclaimer invariant is satisfied.
    pub fn new(
        provenance: BenchmarkProvenance,
        total_scenarios_evaluated: usize,
        primary_stats: PairedTokenSavingsStats,
        secondary_holm_bonferroni: Vec<HolmBonferroniResult>,
        itt_sensitivity: SensitivityAnalysisResult,
        sample_size_plan: ConfirmatorySampleSizePlan,
    ) -> Self {
        let total_pairs = primary_stats.total_pairs_attempted;
        let valid_pairs = primary_stats.valid_protocol_pairs_count;
        let token_savings_denominator = format!("{valid_pairs} of {total_pairs} pairs");

        Self {
            provenance,
            exploratory_disclaimer: EXPLORATORY_PILOT_DISCLAIMER.to_string(),
            total_scenarios_evaluated,
            total_matched_pairs_attempted: total_pairs,
            token_savings_denominator,
            primary_stats,
            secondary_holm_bonferroni,
            itt_sensitivity,
            sample_size_plan,
        }
    }

    /// Serialize report to formatted JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize report from JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Render human-readable, publication-grade Markdown report.
    pub fn to_markdown(&self) -> String {
        let mut md = String::with_capacity(8192);

        // Header & Disclaimer
        md.push_str("# Engram Live Benchmark Report — Empirical Evaluation\n\n");
        md.push_str("> [!WARNING]\n");
        md.push_str("> **Epistemological Disclaimer (Exploratory Pilot):**\n");
        md.push_str(&format!("> {}\n\n", self.exploratory_disclaimer));

        // Provenance Table
        md.push_str("## 1. Provenance & Execution Context\n\n");
        md.push_str("| Attribute | Value |\n");
        md.push_str("|:---|:---|\n");
        md.push_str(&format!(
            "| **Evaluation Type** | `{}` |\n",
            self.provenance.evaluation_type
        ));
        md.push_str(&format!(
            "| **Engine Version** | `{}` |\n",
            self.provenance.engine_version
        ));
        md.push_str(&format!(
            "| **Engine Revision** | `{}` |\n",
            self.provenance.engine_revision
        ));
        md.push_str(&format!(
            "| **Corpus Digest** | `{}` |\n",
            self.provenance.corpus_digest
        ));
        md.push_str(&format!(
            "| **Timestamp (UTC)** | `{}` |\n",
            self.provenance.timestamp_utc
        ));
        md.push_str(&format!(
            "| **Provider** | `{}` |\n",
            self.provenance.provider
        ));
        md.push_str(&format!(
            "| **Model ID** | `{}` |\n",
            self.provenance.model_id
        ));
        md.push_str(&format!(
            "| **Temperature** | `{}` |\n",
            self.provenance.temperature
        ));
        md.push_str(&format!(
            "| **Prompt Digest** | `{}` |\n",
            self.provenance.prompt_digest
        ));
        md.push_str(&format!(
            "| **Scenarios Evaluated** | `{}` |\n",
            self.total_scenarios_evaluated
        ));
        md.push_str(&format!(
            "| **Total Matched Pairs** | `{}` |\n",
            self.total_matched_pairs_attempted
        ));
        md.push_str(&format!(
            "| **Token Savings Denominator** | `{}` |\n\n",
            self.token_savings_denominator
        ));

        // Primary Registered Endpoint
        md.push_str("## 2. Pre-Registered Primary Outcome\n\n");
        md.push_str("### Per-Pair Billed-Token Savings Rate ($S_i$)\n\n");
        md.push_str("$$S_i = \\frac{C_i - T_i}{C_i}$$\n\n");
        md.push_str(&format!(
            "*Evaluated across eligible valid matched pairs (`{}`). Pairs with zero Control tokens ($C_i=0$) or incomplete sessions are strictly handled per pre-registration protocol.*\n\n",
            self.token_savings_denominator
        ));

        md.push_str("| Metric | Estimate | Interpretation |\n");
        md.push_str("|:---|:---:|:---|\n");

        let mean_str = self
            .primary_stats
            .mean_savings_rate
            .map(|m| format!("{:.2}%", m * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **Mean Savings Rate (\\bar{{S}})** | `{mean_str}` | Primary continuous estimator |\n"
        ));

        let sd_str = self
            .primary_stats
            .std_dev_savings_rate
            .map(|s| format!("{:.4}", s))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **Sample Standard Deviation ($s_S$)** | `{sd_str}` | Dispersion across valid pairs |\n"));

        let med_str = self
            .primary_stats
            .median_savings_rate
            .map(|m| format!("{:.2}%", m * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **Median Savings Rate** | `{med_str}` | Non-parametric central tendency |\n"
        ));

        let iqr_str = self
            .primary_stats
            .iqr_savings_rate
            .map(|iqr| format!("{:.2}%", iqr * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **Interquartile Range (IQR)** | `{iqr_str}` | Middle 50% spread |\n"
        ));

        let t_stat_str = self
            .primary_stats
            .t_statistic
            .map(|t| format!("{:.4}", t))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **One-Sample $t$-Statistic** | `{t_stat_str}` | $H_0: \\mu_S \\le 0$ vs $H_1: \\mu_S > 0$ |\n"));

        let p_one_str = self
            .primary_stats
            .t_test_p_value_one_tailed
            .map(|p| format!("{:.4}", p))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **$t$-Test $p$-Value (One-Tailed)** | `{p_one_str}` | Directional savings significance |\n"));

        let p_two_str = self
            .primary_stats
            .t_test_p_value_two_tailed
            .map(|p| format!("{:.4}", p))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **$t$-Test $p$-Value (Two-Tailed)** | `{p_two_str}` | Non-directional significance |\n"));

        let d_str = self
            .primary_stats
            .cohens_d
            .map(|d| format!("{:.4}", d))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **Standardized Effect Size (Cohen's $d$)** | `{d_str}` | $d_S = \\bar{{S}} / s_S$ |\n"));

        let w_str = self
            .primary_stats
            .wilcoxon_w
            .map(|w| format!("{:.1}", w))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **Wilcoxon Signed-Rank ($W$)** | `{w_str}` | Rank-sum statistic ($H_0$: symmetric differences about 0) |\n"));

        let w_p_str = self
            .primary_stats
            .wilcoxon_p_value
            .map(|p| format!("{:.4}", p))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **Wilcoxon $p$-Value** | `{w_p_str}` | Asymptotic continuity-corrected signed-rank $p$-value |\n\n"));
        md.push_str("*Note: The Wilcoxon signed-rank test evaluates the Hodges–Lehmann pseudomedian under the assumption of a symmetric difference distribution. At N=5, normal approximations are exploratory only; exact two-tailed significance at alpha=0.05 is mathematically unattainable (exact p_min = 0.0625).*\n\n");

        // Evaluated Pairs Detail
        md.push_str("### Matched Pairs Breakdown\n\n");
        md.push_str("| Pair Index | Scenario ID | Control Tokens ($C_i$) | Treatment Tokens ($T_i$) | Savings ($S_i$) | Raw Diff ($\\Delta_i$) | Protocol Status |\n");
        md.push_str("|:---:|:---|:---:|:---:|:---:|:---:|:---|\n");
        for p in &self.primary_stats.evaluated_pairs {
            let s_str = p
                .savings_rate
                .map(|s| format!("{:.2}%", s * 100.0))
                .unwrap_or_else(|| "—".to_string());
            let status = if p.is_valid_protocol_pair {
                "Valid".to_string()
            } else {
                format!(
                    "Excluded ({})",
                    p.exclusion_reason.as_deref().unwrap_or("Unknown")
                )
            };
            md.push_str(&format!(
                "| {} | `{}` | {} | {} | {} | {} | {} |\n",
                p.pair_index,
                p.scenario_id,
                p.control_tokens,
                p.treatment_tokens,
                s_str,
                p.raw_difference,
                status
            ));
        }
        md.push('\n');

        // Secondary Registered Endpoints & Holm-Bonferroni
        md.push_str(
            "## 3. Secondary Endpoints & Holm-Bonferroni Multiple Comparison Correction\n\n",
        );
        md.push_str("> [!NOTE]\n");
        md.push_str("> **Methodological Boundary:** Raw token differences ($\\Delta_i = C_i - T_i$) and task metrics are registered as secondary endpoints and are **never** conflated with the ratio savings rate ($S_i$). To prevent false discoveries from multiplicity, all secondary endpoints are adjusted via the step-down Holm-Bonferroni procedure to control $\\text{FWER} \\le 0.05$.\n\n");

        md.push_str("### Raw Paired Token Difference ($\\Delta_i = C_i - T_i$)\n\n");
        md.push_str("| Metric | Estimate |\n");
        md.push_str("|:---|:---:|\n");
        md.push_str(&format!(
            "| **Mean Raw Tokens Saved (\\bar{{\\Delta}})** | `{:.1}` tokens |\n",
            self.primary_stats.mean_raw_difference
        ));
        md.push_str(&format!(
            "| **Sample Standard Deviation ($s_\\Delta$)** | `{:.1}` tokens |\n",
            self.primary_stats.std_dev_raw_difference
        ));
        let dz_str = self
            .primary_stats
            .paired_cohens_dz
            .map(|dz| format!("{:.4}", dz))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!("| **Paired Cohen's $d_z$** | `{dz_str}` |\n\n"));

        md.push_str(
            "### Holm-Bonferroni Step-Down Multiplicity Adjustments ($\\alpha = 0.05$)\n\n",
        );
        if self.secondary_holm_bonferroni.is_empty() {
            md.push_str("*No secondary hypothesis tests registered.*\n\n");
        } else {
            md.push_str("| Step Rank | Secondary Endpoint | Observed Effect | Unadjusted $p$-Value | Threshold ($\\alpha / (m - k + 1)$) | Adjusted $p$-Value | Significant ($\\text{FWER} \\le 0.05$) |\n");
            md.push_str("|:---:|:---|:---:|:---:|:---:|:---:|:---:|\n");
            for r in &self.secondary_holm_bonferroni {
                let sig_badge = if r.is_significant { "**YES**" } else { "No" };
                md.push_str(&format!(
                    "| {} | {} | {:.4} | {:.4} | {:.5} | {:.4} | {} |\n",
                    r.step_rank,
                    r.endpoint_name,
                    r.observed_effect,
                    r.unadjusted_p_value,
                    r.adjusted_alpha_threshold,
                    r.adjusted_p_value,
                    sig_badge
                ));
            }
            md.push('\n');
        }

        // Sensitivity Analysis (ITT)
        md.push_str("## 4. Intent-To-Treat (ITT) Sensitivity Analysis\n\n");
        md.push_str("> [!IMPORTANT]\n");
        md.push_str("> **Survivorship Bias Guard:** Incomplete sessions are penalized at the scenario maximum budget cap ($B_{\\text{max}}$). This prevents agent aborts or timeouts from artificially inflating reported efficiency.\n\n");

        md.push_str("| Sensitivity Metric | Value |\n");
        md.push_str("|:---|:---:|\n");
        md.push_str(&format!(
            "| **Total ITT Pairs** | `{}` |\n",
            self.itt_sensitivity.total_itt_pairs
        ));
        md.push_str(&format!(
            "| **Imputed Incomplete Pairs** | `{}` |\n",
            self.itt_sensitivity.itt_imputed_pairs_count
        ));

        let itt_m_str = self
            .itt_sensitivity
            .itt_mean_savings_rate
            .map(|m| format!("{:.2}%", m * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **ITT Mean Savings Rate (\\bar{{S}}_{{\\text{{ITT}}}})** | `{itt_m_str}` |\n"
        ));

        let itt_sd_str = self
            .itt_sensitivity
            .itt_std_dev_savings_rate
            .map(|s| format!("{:.4}", s))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **ITT Sample Standard Deviation** | `{itt_sd_str}` |\n"
        ));

        let itt_med_str = self
            .itt_sensitivity
            .itt_median_savings_rate
            .map(|m| format!("{:.2}%", m * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **ITT Median Savings Rate** | `{itt_med_str}` |\n"
        ));

        let itt_iqr_str = self
            .itt_sensitivity
            .itt_iqr_savings_rate
            .map(|iqr| format!("{:.2}%", iqr * 100.0))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| **ITT Interquartile Range (IQR)** | `{itt_iqr_str}` |\n\n"
        ));

        // Confirmatory Sample Size Planning
        md.push_str("## 5. Confirmatory Study Sample Size Planning\n\n");
        md.push_str("Calculated using the iterative paired-$t$ power algorithm with upper 80% $\\chi^2$ pilot variance inflation and $+15\\%$ infrastructure attrition buffer:\n\n");
        md.push_str(&format!(
            "- **Pilot Sample Size ($N_{{\\text{{pilot}}}}$):** `{}`\n",
            self.sample_size_plan.pilot_n
        ));
        md.push_str(&format!(
            "- **Measured Pilot Standard Deviation ($s_{{S, \\text{{pilot}}}}$):** `{:.4}`\n",
            self.sample_size_plan.pilot_measured_sd
        ));
        md.push_str(&format!("- **Inflated Upper 80% Standard Deviation ($s_{{S, \\text{{upper}}}}$):** `{:.4}` ($\\approx 1.55745 \\times s_S$)\n", self.sample_size_plan.inflated_upper_80_sd));
        md.push_str(&format!(
            "- **Target MDE:** `{:.1}%`\n",
            self.sample_size_plan.target_mde * 100.0
        ));
        md.push_str(&format!(
            "- **Target Significance Level ($\\alpha$):** `{}` (two-tailed)\n",
            self.sample_size_plan.target_alpha
        ));
        md.push_str(&format!(
            "- **Target Power ($1 - \\beta$):** `{:.0}%`\n",
            self.sample_size_plan.target_power * 100.0
        ));
        md.push_str(&format!(
            "- **Normal Initial Sizing ($N_0$):** `{}`\n",
            self.sample_size_plan.initial_n0
        ));
        md.push_str(&format!(
            "- **Converged $t$-Distribution Sizing ($N_k$):** `{}`\n",
            self.sample_size_plan.converged_n
        ));
        md.push_str(&format!(
            "- **Attrition Buffer:** `+{:.0}%`\n",
            self.sample_size_plan.attrition_buffer_percent
        ));
        md.push_str(&format!("- **Recommended Confirmatory Pairs ($N_{{\\text{{recommended}}}}$):** `**{} pairs**`\n\n", self.sample_size_plan.recommended_sample_size));

        md.push_str("### Illustrative Sensitivity Planning Matrix\n\n");
        md.push_str("| Assumed $s_S$ | $s_{S, \\text{upper}}$ (80% Bound) | MDE = 15% | MDE = 20% (Target) | MDE = 25% |\n");
        md.push_str("|:---:|:---:|:---:|:---:|:---:|\n");

        // Group matrix entries by assumed_sd
        let mut by_sd: std::collections::BTreeMap<
            u64,
            Vec<&crate::empirical_live::stats::SampleSizeMatrixEntry>,
        > = std::collections::BTreeMap::new();
        for entry in &self.sample_size_plan.sensitivity_matrix {
            let key = (entry.assumed_sd * 1000.0).round() as u64;
            by_sd.entry(key).or_default().push(entry);
        }

        for (_key, entries) in by_sd {
            if let Some(first) = entries.first() {
                let sd_val = first.assumed_sd;
                let upper_val = first.upper_80_bound_sd;
                let n_15 = entries
                    .iter()
                    .find(|e| (e.mde - 0.15).abs() < 1e-4)
                    .map(|e| format!("N = {}", e.required_n))
                    .unwrap_or_else(|| "—".to_string());
                let n_20 = entries
                    .iter()
                    .find(|e| (e.mde - 0.20).abs() < 1e-4)
                    .map(|e| format!("N = {}", e.required_n))
                    .unwrap_or_else(|| "—".to_string());
                let n_25 = entries
                    .iter()
                    .find(|e| (e.mde - 0.25).abs() < 1e-4)
                    .map(|e| format!("N = {}", e.required_n))
                    .unwrap_or_else(|| "—".to_string());

                md.push_str(&format!(
                    "| **$s_S = {:.2}$** | `{:.3}` | `{}` | `{}` | `{}` |\n",
                    sd_val, upper_val, n_15, n_20, n_25
                ));
            }
        }
        md.push('\n');

        md
    }
}
