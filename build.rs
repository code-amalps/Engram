use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

fn is_valid_hex(s: &str, min_len: usize, max_len: usize) -> bool {
    let len = s.len();
    len >= min_len && len <= max_len && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn hash_file_sha256(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("{:X}", hasher.finalize()))
}

fn collect_rs_files(dir: &Path, base: &Path, files: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, base, files);
            } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(rel) = path.strip_prefix(base) {
                    let normalized = rel.to_string_lossy().replace('\\', "/");
                    files.push(normalized);
                }
            }
        }
    }
}

fn discover_compilation_files(manifest_dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let root_candidates = [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        ".cargo/config.toml",
        ".cargo/config",
    ];
    for candidate in root_candidates {
        if manifest_dir.join(candidate).is_file() {
            files.push(candidate.to_string());
        }
    }
    let src_dir = manifest_dir.join("src");
    if src_dir.is_dir() {
        collect_rs_files(&src_dir, manifest_dir, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

fn compute_source_fingerprint(manifest_dir: &Path, files: &[String]) -> Option<String> {
    let mut hasher = Sha256::new();
    for rel_path in files {
        let full_path = manifest_dir.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = std::fs::read(&full_path).ok()?;
        let path_bytes = rel_path.as_bytes();
        hasher.update(&(path_bytes.len() as u32).to_be_bytes());
        hasher.update(path_bytes);
        hasher.update(&(content.len() as u64).to_be_bytes());
        hasher.update(&content);
    }
    Some(format!("{:X}", hasher.finalize()))
}

fn get_git_object_format(manifest_dir: &Path) -> (String, usize) {
    let mut cmd = Command::new("git");
    cmd.current_dir(manifest_dir);
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env("GIT_CEILING_DIRECTORIES", manifest_dir);
    cmd.args(["rev-parse", "--show-object-format"]);
    let fmt = cmd
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "sha1".to_string());

    let len = if fmt == "sha256" { 64 } else { 40 };
    (fmt, len)
}

fn is_git_object(manifest_dir: &Path, raw: &str) -> bool {
    let trimmed = raw.trim();
    let mut cmd = Command::new("git");
    cmd.current_dir(manifest_dir);
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env("GIT_CEILING_DIRECTORIES", manifest_dir);
    cmd.args(["rev-parse", "--verify", &format!("{}^{{object}}", trimmed)]);
    cmd.output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn resolve_git_commit(manifest_dir: &Path, raw: &str, expected_len: usize) -> Option<String> {
    let trimmed = raw.trim();
    if !is_valid_hex(trimmed, 7, expected_len) {
        return None;
    }
    let mut cmd = Command::new("git");
    cmd.current_dir(manifest_dir);
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env("GIT_CEILING_DIRECTORIES", manifest_dir);
    cmd.args(["rev-parse", "--verify", &format!("{}^{{commit}}", trimmed)]);
    let out = cmd.output().ok()?;
    if out.status.success() {
        let full = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if full.len() == expected_len && is_valid_hex(&full, expected_len, expected_len) {
            Some(full)
        } else {
            None
        }
    } else {
        None
    }
}

fn sanitize_flags_privacy(raw: &str) -> String {
    let mut sanitized = Vec::new();
    for part in raw.split_whitespace() {
        if part.contains(":\\")
            || part.contains(":/")
            || part.starts_with("/home/")
            || part.starts_with("/Users/")
        {
            if let Some(eq_idx) = part.find('=') {
                let prefix = &part[..=eq_idx];
                sanitized.push(format!("{}[REDACTED_PATH]", prefix));
            } else if let Some(colon_idx) = part.find(':') {
                if colon_idx > 1
                    && !part[..colon_idx].contains('\\')
                    && !part[..colon_idx].contains('/')
                {
                    let prefix = &part[..=colon_idx];
                    sanitized.push(format!("{}[REDACTED_PATH]", prefix));
                } else {
                    sanitized.push("[REDACTED_PATH]".to_string());
                }
            } else {
                sanitized.push("[REDACTED_PATH]".to_string());
            }
        } else {
            sanitized.push(part.to_string());
        }
    }
    sanitized.join(" ")
}

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));

    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.5.5".to_string());
    let raw_profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let raw_target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let schema_version = "2";

    let profile = raw_profile
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();
    let target_triple = raw_target
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();

    let raw_host = std::env::var("HOST").unwrap_or_else(|_| raw_target.clone());
    let host_triple = raw_host
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();

    let raw_opt = std::env::var("OPT_LEVEL").unwrap_or_else(|_| {
        if profile == "release" {
            "3".to_string()
        } else {
            "0".to_string()
        }
    });
    let opt_level = raw_opt
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();

    let debug_assertions = if let Ok(val) = std::env::var("CARGO_CFG_DEBUG_ASSERTIONS") {
        if val.trim().is_empty() {
            "true".to_string()
        } else {
            val.replace('\r', "").replace('\n', " ").trim().to_string()
        }
    } else if profile == "release" {
        "false".to_string()
    } else {
        "true".to_string()
    };

    let raw_debug = std::env::var("DEBUG").unwrap_or_else(|_| {
        if profile == "release" {
            "0".to_string()
        } else {
            "true".to_string()
        }
    });
    let debug_level = raw_debug
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();

    let raw_panic = std::env::var("CARGO_CFG_PANIC").unwrap_or_else(|_| "unwind".to_string());
    let panic_strategy = raw_panic
        .replace('\r', "")
        .replace('\n', " ")
        .trim()
        .to_string();

    let cargo_features = if let Ok(f_cfg) = std::env::var("CARGO_CFG_FEATURE") {
        let mut feats: Vec<&str> = f_cfg
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        feats.sort_unstable();
        feats.dedup();
        feats.join(",")
    } else {
        let mut feats: Vec<String> = std::env::vars()
            .filter_map(|(k, _)| {
                if let Some(suffix) = k.strip_prefix("CARGO_FEATURE_") {
                    Some(suffix.to_ascii_lowercase())
                } else {
                    None
                }
            })
            .collect();
        feats.sort();
        feats.dedup();
        feats.join(",")
    };

    let raw_rustflags = if let Ok(encoded) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        encoded.replace('\x1f', " ")
    } else if let Ok(rf) = std::env::var("RUSTFLAGS") {
        rf
    } else {
        std::env::var("CARGO_BUILD_RUSTFLAGS").unwrap_or_default()
    };
    let rustflags =
        sanitize_flags_privacy(&raw_rustflags.replace('\r', "").replace('\n', " ").trim());

    let lockfile_path = manifest_dir.join("Cargo.lock");
    let lockfile_sha256 = hash_file_sha256(&lockfile_path).unwrap_or_else(|| "unknown".to_string());

    let rustc_cmd_path = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let (rustc_version, rustc_commit, rustc_llvm) = {
        let mut cmd = Command::new(&rustc_cmd_path);
        cmd.arg("-Vv");
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                let mut v = "unknown".to_string();
                let mut commit = "unknown".to_string();
                let mut llvm = "unknown".to_string();
                for line in s.lines() {
                    if line.starts_with("rustc ") {
                        v = line.trim().to_string();
                    } else if let Some(c) = line.strip_prefix("commit-hash: ") {
                        commit = c.trim().to_string();
                    } else if let Some(l) = line.strip_prefix("LLVM version: ") {
                        llvm = l.trim().to_string();
                    }
                }
                (v, commit, llvm)
            } else {
                (
                    "unknown".to_string(),
                    "unknown".to_string(),
                    "unknown".to_string(),
                )
            }
        } else {
            (
                "unknown".to_string(),
                "unknown".to_string(),
                "unknown".to_string(),
            )
        }
    };

    // Discover Linker identity, SHA-256 and effective flags (Schema v2)
    let (linker_name, linker_sha256, linker_flags) = {
        let mut l_name = "unknown".to_string();
        let mut l_sha256 = "unknown".to_string();
        let l_flags = if rustflags.contains("/BREPRO") || rustflags.contains("link-arg=/BREPRO") {
            "/BREPRO".to_string()
        } else {
            "default".to_string()
        };

        let linker_file: Option<PathBuf> = if let Ok(custom_linker) = std::env::var("RUSTC_LINKER")
        {
            Some(PathBuf::from(custom_linker))
        } else if cfg!(windows) {
            let vswhere_path =
                r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe";
            let vs_root = if Path::new(vswhere_path).exists() {
                Command::new(vswhere_path)
                    .args(&["-latest", "-property", "installationPath"])
                    .output()
                    .ok()
                    .and_then(|out| {
                        if out.status.success() {
                            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                        } else {
                            None
                        }
                    })
            } else {
                None
            };

            if let Some(root) = vs_root {
                let msvc_tools_dir = Path::new(&root).join(r"VC\Tools\MSVC");
                if msvc_tools_dir.exists() {
                    let mut found = None;
                    if let Ok(entries) = std::fs::read_dir(msvc_tools_dir) {
                        for e in entries.flatten() {
                            let candidate = e.path().join(r"bin\Hostx64\x64\link.exe");
                            if candidate.exists() {
                                found = Some(candidate);
                                break;
                            }
                        }
                    }
                    found
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(lp) = linker_file {
            if lp.exists() {
                if let Some(h) = hash_file_sha256(&lp) {
                    l_sha256 = h;
                }
                if let Ok(out) = Command::new(&lp).arg("/?").output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    for line in s.lines() {
                        if line.contains("Microsoft (R) Incremental Linker") {
                            l_name = line.trim().to_string();
                            break;
                        }
                    }
                }
                if l_name == "unknown" {
                    l_name = "MSVC link.exe".to_string();
                }
            }
        }

        (l_name, l_sha256, l_flags)
    };

    // 1. Discover compilation input files & calculate local snapshot fingerprint
    let compilation_files = discover_compilation_files(&manifest_dir);
    let local_computed_fingerprint = compute_source_fingerprint(&manifest_dir, &compilation_files);

    // 2. Set up Cargo rebuild tracking for all compilation inputs and relevant environment variables
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    if manifest_dir.join(".cargo/config.toml").is_file() {
        println!("cargo:rerun-if-changed=.cargo/config.toml");
    }
    if manifest_dir.join(".cargo/config").is_file() {
        println!("cargo:rerun-if-changed=.cargo/config");
    }
    println!("cargo:rerun-if-changed=src");
    for f in &compilation_files {
        println!("cargo:rerun-if-changed={}", f);
    }
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_BUILD_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=RUSTC_WRAPPER");
    println!("cargo:rerun-if-env-changed=RUSTC_WORKSPACE_WRAPPER");
    println!("cargo:rerun-if-env-changed=CARGO_BUILD_RUSTC_WRAPPER");
    println!("cargo:rerun-if-env-changed=ENGRAM_GIT_COMMIT");
    println!("cargo:rerun-if-env-changed=ENGRAM_SOURCE_FINGERPRINT");
    println!("cargo:rerun-if-env-changed=ENGRAM_PROVENANCE_SOURCE");

    // 3. Inspect Git environment with boundary isolation
    let git_entry = manifest_dir.join(".git");
    let mut is_local_repo = git_entry.exists();

    if is_local_repo {
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/index");

        let mut top_cmd = Command::new("git");
        top_cmd.current_dir(&manifest_dir);
        top_cmd.env_remove("GIT_DIR");
        top_cmd.env_remove("GIT_WORK_TREE");
        top_cmd.env("GIT_CEILING_DIRECTORIES", &manifest_dir);
        top_cmd.args(["rev-parse", "--show-toplevel"]);

        let is_genuine = top_cmd
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    let s = String::from_utf8(out.stdout).ok()?;
                    let top_path = PathBuf::from(s.trim());
                    let canon_top = top_path.canonicalize().ok()?;
                    let canon_manifest = manifest_dir.canonicalize().ok()?;
                    Some(canon_top == canon_manifest)
                } else {
                    None
                }
            })
            .unwrap_or(false);

        if !is_genuine {
            is_local_repo = false;
        }
    }

    let (_obj_format, expected_commit_len) = if is_local_repo {
        get_git_object_format(&manifest_dir)
    } else {
        ("sha1".to_string(), 40)
    };

    // 4. Resolve Git dirty state: distinguish working-tree dirty from compilation-defining input dirty
    let (discovered_working_tree_dirty, discovered_compilation_dirty) = if is_local_repo {
        let mut status_cmd = Command::new("git");
        status_cmd.current_dir(&manifest_dir);
        status_cmd.env_remove("GIT_DIR");
        status_cmd.env_remove("GIT_WORK_TREE");
        status_cmd.env("GIT_CEILING_DIRECTORIES", &manifest_dir);
        status_cmd.args(["status", "--porcelain=v1"]);

        if let Ok(out) = status_cmd.output() {
            if out.status.success() {
                let out_str = String::from_utf8_lossy(&out.stdout);
                let wt_dirty = !out_str.trim().is_empty();
                let mut comp_dirty = false;
                for line in out_str.lines() {
                    if line.len() >= 4 {
                        let path_part = line[3..].trim().replace('\\', "/");
                        if compilation_files.iter().any(|cf| cf == &path_part) {
                            comp_dirty = true;
                            break;
                        }
                    }
                }
                (
                    if wt_dirty {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    },
                    if comp_dirty {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    },
                )
            } else {
                ("unknown".to_string(), "unknown".to_string())
            }
        } else {
            ("unknown".to_string(), "unknown".to_string())
        }
    } else {
        ("unknown".to_string(), "unknown".to_string())
    };

    // 5. Resolve source metadata
    let raw_commit = std::env::var("ENGRAM_GIT_COMMIT").ok();
    let raw_fingerprint = std::env::var("ENGRAM_SOURCE_FINGERPRINT").ok();
    let raw_provenance = std::env::var("ENGRAM_PROVENANCE_SOURCE").ok();

    let has_explicit_input =
        raw_commit.is_some() || raw_fingerprint.is_some() || raw_provenance.is_some();

    let (
        git_commit,
        git_dirty,
        compilation_dirty,
        working_tree_dirty,
        source_fingerprint,
        provenance_source,
    ) = if has_explicit_input {
        let mut valid = true;
        let mut locally_verified_commit = false;

        let commit_val = match raw_commit {
            Some(ref c) if !c.trim().is_empty() => {
                let trimmed = c.trim();
                if is_local_repo {
                    if let Some(resolved) =
                        resolve_git_commit(&manifest_dir, trimmed, expected_commit_len)
                    {
                        locally_verified_commit = true;
                        resolved
                    } else if is_git_object(&manifest_dir, trimmed) {
                        valid = false;
                        "invalid".to_string()
                    } else if trimmed.len() == expected_commit_len
                        && is_valid_hex(trimmed, expected_commit_len, expected_commit_len)
                    {
                        trimmed.to_string()
                    } else {
                        valid = false;
                        "invalid".to_string()
                    }
                } else {
                    if trimmed.len() == expected_commit_len
                        && is_valid_hex(trimmed, expected_commit_len, expected_commit_len)
                    {
                        trimmed.to_string()
                    } else {
                        valid = false;
                        "invalid".to_string()
                    }
                }
            }
            _ => "unknown".to_string(),
        };

        let fingerprint_val = match raw_fingerprint {
            Some(ref fp) if !fp.trim().is_empty() => {
                let trimmed = fp.trim();
                if is_valid_hex(trimmed, 64, 64) {
                    trimmed.to_uppercase()
                } else {
                    valid = false;
                    "invalid".to_string()
                }
            }
            _ => "unknown".to_string(),
        };

        let provenance_val = match raw_provenance {
            Some(ref p) if !p.trim().is_empty() => {
                let trimmed = p.trim();
                match trimmed {
                    "LocalGitRepository" | "ExplicitSnapshot" | "ExplicitOverride" | "Unknown" => {
                        if valid {
                            trimmed.to_string()
                        } else {
                            "Unknown".to_string()
                        }
                    }
                    _ => {
                        valid = false;
                        "Unknown".to_string()
                    }
                }
            }
            _ => {
                if !valid {
                    "Unknown".to_string()
                } else if raw_fingerprint.is_some() {
                    if let Some(ref local_fp) = local_computed_fingerprint {
                        if local_fp == &fingerprint_val {
                            if locally_verified_commit {
                                "LocalGitRepository".to_string()
                            } else {
                                "ExplicitSnapshot".to_string()
                            }
                        } else {
                            "ExplicitOverride".to_string()
                        }
                    } else {
                        "ExplicitSnapshot".to_string()
                    }
                } else if commit_val != "unknown" && commit_val != "invalid" {
                    if locally_verified_commit {
                        "LocalGitRepository".to_string()
                    } else {
                        "ExplicitSnapshot".to_string()
                    }
                } else {
                    "Unknown".to_string()
                }
            }
        };

        let (dirty_val, comp_dirty, wt_dirty) = if !valid {
            (
                "unknown".to_string(),
                "unknown".to_string(),
                "unknown".to_string(),
            )
        } else if commit_val != "unknown"
            && commit_val != "invalid"
            && fingerprint_val != "unknown"
            && fingerprint_val != "invalid"
        {
            if is_local_repo && raw_fingerprint.is_none() {
                (
                    discovered_working_tree_dirty.clone(),
                    discovered_compilation_dirty.clone(),
                    discovered_working_tree_dirty.clone(),
                )
            } else {
                (
                    "false".to_string(),
                    "false".to_string(),
                    "false".to_string(),
                )
            }
        } else {
            (
                "unknown".to_string(),
                "unknown".to_string(),
                "unknown".to_string(),
            )
        };

        (
            commit_val,
            dirty_val,
            comp_dirty,
            wt_dirty,
            fingerprint_val,
            provenance_val,
        )
    } else if is_local_repo {
        let mut commit_cmd = Command::new("git");
        commit_cmd.current_dir(&manifest_dir);
        commit_cmd.env_remove("GIT_DIR");
        commit_cmd.env_remove("GIT_WORK_TREE");
        commit_cmd.env("GIT_CEILING_DIRECTORIES", &manifest_dir);
        commit_cmd.args(["rev-parse", "HEAD"]);

        let discovered_commit = commit_cmd
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    String::from_utf8(out.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let fp = local_computed_fingerprint.unwrap_or_else(|| "unknown".to_string());
        let prov = if discovered_commit != "unknown" {
            "LocalGitRepository".to_string()
        } else {
            "Unknown".to_string()
        };

        (
            discovered_commit,
            discovered_working_tree_dirty.clone(),
            discovered_compilation_dirty,
            discovered_working_tree_dirty,
            fp,
            prov,
        )
    } else {
        (
            "unknown".to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            "unknown".to_string(),
            "Unknown".to_string(),
        )
    };

    // 6. Unambiguous length-prefixed canonical identity serialization (22 canonical fields in Schema v2)
    let canonical_fields = [
        ("schema_version", schema_version),
        ("package_version", &pkg_version),
        ("git_commit", &git_commit),
        ("git_dirty", &git_dirty),
        ("provenance_source", &provenance_source),
        ("source_fingerprint", &source_fingerprint),
        ("lockfile_sha256", &lockfile_sha256),
        ("build_profile", &profile),
        ("rustc_version", &rustc_version),
        ("rustc_commit", &rustc_commit),
        ("rustc_llvm", &rustc_llvm),
        ("linker_name", &linker_name),
        ("linker_sha256", &linker_sha256),
        ("linker_flags", &linker_flags),
        ("target_triple", &target_triple),
        ("host_triple", &host_triple),
        ("opt_level", &opt_level),
        ("debug_assertions", &debug_assertions),
        ("debug_level", &debug_level),
        ("panic_strategy", &panic_strategy),
        ("cargo_features", &cargo_features),
        ("rustflags", &rustflags),
    ];

    let mut canonical_identity_string = String::new();
    for (k, v) in canonical_fields {
        canonical_identity_string.push_str(&format!("{}:{}={}:{}\n", k.len(), k, v.len(), v));
    }

    let mut build_id_hasher = Sha256::new();
    build_id_hasher.update(canonical_identity_string.as_bytes());
    let build_id = format!("{:X}", build_id_hasher.finalize());

    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_COMMIT={}", git_commit);
    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_DIRTY={}", git_dirty);
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_COMPILATION_DIRTY={}",
        compilation_dirty
    );
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_WORKING_TREE_DIRTY={}",
        working_tree_dirty
    );
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_PROVENANCE_SOURCE={}",
        provenance_source
    );
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_SOURCE_FINGERPRINT={}",
        source_fingerprint
    );
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_LOCKFILE_SHA256={}",
        lockfile_sha256
    );
    println!("cargo:rustc-env=ENGRAM_BUILD_ID={}", build_id);
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC={}", rustc_version);
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC_COMMIT={}", rustc_commit);
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC_LLVM={}", rustc_llvm);
    println!("cargo:rustc-env=ENGRAM_BUILD_LINKER_NAME={}", linker_name);
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_LINKER_SHA256={}",
        linker_sha256
    );
    println!("cargo:rustc-env=ENGRAM_BUILD_LINKER_FLAGS={}", linker_flags);
    println!("cargo:rustc-env=ENGRAM_BUILD_PROFILE={}", profile);
    println!("cargo:rustc-env=ENGRAM_BUILD_TARGET={}", target_triple);
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_SCHEMA_VERSION={}",
        schema_version
    );
    println!("cargo:rustc-env=ENGRAM_BUILD_HOST={}", host_triple);
    println!("cargo:rustc-env=ENGRAM_BUILD_OPT_LEVEL={}", opt_level);
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_DEBUG_ASSERTIONS={}",
        debug_assertions
    );
    println!("cargo:rustc-env=ENGRAM_BUILD_DEBUG_LEVEL={}", debug_level);
    println!(
        "cargo:rustc-env=ENGRAM_BUILD_PANIC_STRATEGY={}",
        panic_strategy
    );
    println!("cargo:rustc-env=ENGRAM_BUILD_FEATURES={}", cargo_features);
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTFLAGS={}", rustflags);
}
