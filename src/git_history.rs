//! Git History Intelligence: statistical co-change mining with noise filtering and evidence formatting.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::storage::{db, now, root_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoChangeSignal {
    pub source_file: String,
    pub target_file: String,
    pub support: usize,
    pub sample_size: usize,
    pub conditional_probability: f64,
    pub reverse_probability: f64,
    pub lift: f64,
    pub window: usize,
    pub advisory_message: String,
}

/// Check if a file is considered noise (lockfiles, generated assets, build artifacts).
pub fn is_noise_file(path: &str) -> bool {
    let p = path.replace('\\', "/").to_ascii_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);

    // Lockfiles
    if name == "cargo.lock"
        || name == "package-lock.json"
        || name == "yarn.lock"
        || name == "pnpm-lock.yaml"
        || name == "composer.lock"
        || name == "gemfile.lock"
        || name == "poetry.lock"
    {
        return true;
    }

    // Minified or generated
    if name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.contains(".generated.")
        || name.ends_with(".d.ts.map")
    {
        return true;
    }

    // Target, build, dist directories
    if p.starts_with("target/")
        || p.starts_with("dist/")
        || p.starts_with("build/")
        || p.starts_with(".git/")
        || p.starts_with("node_modules/")
    {
        return true;
    }

    false
}

/// Initialize the cochange table in SQLite.
pub fn init_cochange_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cochange_edges (
            source_file TEXT NOT NULL,
            target_file TEXT NOT NULL,
            support INTEGER NOT NULL,
            sample_size INTEGER NOT NULL,
            cond_prob REAL NOT NULL,
            rev_prob REAL NOT NULL,
            lift REAL NOT NULL,
            window_size INTEGER NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (source_file, target_file)
        )",
        [],
    )?;
    Ok(())
}

/// Extract commit batches of modified files from `git log`.
pub fn extract_commit_file_batches(root: &Path, window: usize) -> Result<(usize, Vec<Vec<String>>)> {
    let output = Command::new("git")
        .args([
            "log",
            &format!("-n{window}"),
            "--no-merges",
            "--name-only",
            "--format=COMMIT_SEP:%H",
        ])
        .current_dir(root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok((0, Vec::new())),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut batches = Vec::new();
    let mut current_files: HashSet<String> = HashSet::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("COMMIT_SEP:") {
            if !current_files.is_empty() {
                // Safeguard against mass refactor commits touching > 40 files
                if current_files.len() <= 40 {
                    batches.push(current_files.into_iter().collect());
                }
                current_files = HashSet::new();
            }
        } else {
            let normalized = line.replace('\\', "/");
            if !is_noise_file(&normalized) {
                current_files.insert(normalized);
            }
        }
    }

    if !current_files.is_empty() && current_files.len() <= 40 {
        batches.push(current_files.into_iter().collect());
    }

    let total = batches.len();
    Ok((total, batches))
}

/// Mine Git co-changes and save into local SQLite database.
pub fn mine_and_persist_cochanges(root: &Path, window: usize) -> Result<usize> {
    let root = root_path(root)?;
    let (total_commits, batches) = extract_commit_file_batches(&root, window)?;
    if batches.is_empty() {
        return Ok(0);
    }

    // Count single file occurrences
    let mut file_commit_counts: HashMap<String, usize> = HashMap::new();
    // Count pair occurrences: (file_a, file_b) -> support
    let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();

    for batch in &batches {
        for file in batch {
            *file_commit_counts.entry(file.clone()).or_insert(0) += 1;
        }

        for i in 0..batch.len() {
            for j in 0..batch.len() {
                if i != j {
                    let a = &batch[i];
                    let b = &batch[j];
                    *pair_counts.entry((a.clone(), b.clone())).or_insert(0) += 1;
                }
            }
        }
    }

    let conn = db(&root)?;
    init_cochange_table(&conn)?;

    let mut edges_inserted = 0;
    let now_ts = now();

    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM cochange_edges", [])?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO cochange_edges (
                source_file, target_file, support, sample_size, cond_prob, rev_prob, lift, window_size, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;

        for ((a, b), support) in pair_counts {
            // Statistical safeguard: minimum support of 2 commits
            if support < 2 {
                continue;
            }

            let sample_size = *file_commit_counts.get(&a).unwrap_or(&1);
            let b_total = *file_commit_counts.get(&b).unwrap_or(&1);

            let cond_prob = support as f64 / sample_size as f64;
            let rev_prob = support as f64 / b_total as f64;

            // Lift: P(A & B) / (P(A) * P(B)) = (support / N) / ((sample_size / N) * (b_total / N))
            let lift = if sample_size > 0 && b_total > 0 && total_commits > 0 {
                (support as f64 * total_commits as f64) / (sample_size as f64 * b_total as f64)
            } else {
                1.0
            };

            // Only keep statistically meaningful signals (cond_prob >= 0.25 and lift >= 1.2)
            if cond_prob >= 0.25 && lift >= 1.2 {
                stmt.execute(params![
                    a,
                    b,
                    support as i64,
                    sample_size as i64,
                    cond_prob,
                    rev_prob,
                    lift,
                    window as i64,
                    now_ts
                ])?;
                edges_inserted += 1;
            }
        }
    }

    tx.commit()?;
    Ok(edges_inserted)
}

/// Query co-change signals for a given source file.
pub fn get_cochanges_for_file(
    root: &Path,
    source_file: &str,
    min_prob: f64,
) -> Result<Vec<CoChangeSignal>> {
    let root = root_path(root)?;
    let norm_source = source_file.replace('\\', "/");
    let conn = db(&root)?;
    init_cochange_table(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT target_file, support, sample_size, cond_prob, rev_prob, lift, window_size
         FROM cochange_edges
         WHERE source_file = ?1 AND cond_prob >= ?2
         ORDER BY cond_prob DESC, support DESC
         LIMIT 10",
    )?;

    let rows = stmt.query_map(params![norm_source, min_prob], |row| {
        let target: String = row.get(0)?;
        let support: usize = row.get(1)?;
        let sample_size: usize = row.get(2)?;
        let cond_prob: f64 = row.get(3)?;
        let rev_prob: f64 = row.get(4)?;
        let lift: f64 = row.get(5)?;
        let window: usize = row.get(6)?;

        let pct = (cond_prob * 100.0).round() as u32;
        let msg = format!(
            "Historical signal: '{}' changed in {}% of historical commits modifying '{}' (support: {}/{}, lift: {:.1}).",
            target, pct, norm_source, support, sample_size, lift
        );

        Ok(CoChangeSignal {
            source_file: norm_source.clone(),
            target_file: target,
            support,
            sample_size,
            conditional_probability: cond_prob,
            reverse_probability: rev_prob,
            lift,
            window,
            advisory_message: msg,
        })
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}
