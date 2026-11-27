# Installing Engram

Engram is distributed as a pre-compiled Windows x64 binary and as source code that can be compiled on any platform supporting Rust 1.85+.

---

## 1. Primary Method: Verified Windows x64 Binary (Recommended)

For Windows x64 environments, download and verify the official release candidate archive:

### Step 1: Download Release Archive
Download `engram-preview-v1.5.5-rc2-windows-x64.zip` from the official GitHub Release:
- **Release Page:** [https://github.com/code-amalps/Engram/releases/tag/v1.5.5-rc2](https://github.com/code-amalps/Engram/releases/tag/v1.5.5-rc2)
- **Direct Asset URL:** `https://github.com/code-amalps/Engram/releases/download/v1.5.5-rc2/engram-preview-v1.5.5-rc2-windows-x64.zip`

### Step 2: Verify Cryptographic SHA-256 Checksum
Open PowerShell and verify the archive checksum before extracting:

```powershell
# Compute SHA-256 checksum of downloaded zip
(Get-FileHash -Algorithm SHA256 .\engram-preview-v1.5.5-rc2-windows-x64.zip).Hash
```

**Expected Archive Checksum:**
```text
92B4FE6F7358920F3650782EB43BE70D4A64D7927D25A2C26E412741CAC3751D
```

If the computed hash does not match, do not extract the archive.

### Step 3: Extract Package
Extract the zip package to your preferred tools directory (e.g., `C:\Tools\engram`):

```powershell
Expand-Archive -Path .\engram-preview-v1.5.5-rc2-windows-x64.zip -DestinationPath C:\Tools\engram
```

### Step 4: Verify Executable Checksum
Verify the extracted `bin\engram.exe` binary:

```powershell
(Get-FileHash -Algorithm SHA256 C:\Tools\engram\bin\engram.exe).Hash
```

**Expected Executable Checksum:**
```text
BB0AAA66A2F3ADEE3C8F2D5B91C3DA63FF041FEB98CFE08B1DD27B45F858D087
```

### Step 5: Add to PATH
Add the `bin` directory to your user or system `PATH`:

```powershell
# Add to User PATH via PowerShell
[Environment]::SetEnvironmentVariable(
    "Path",
    [Environment]::GetEnvironmentVariable("Path", "User") + ";C:\Tools\engram\bin",
    "User"
)
```

Restart your terminal session to load the updated `PATH`.

---

## 2. Package Contents

The official `engram-preview-v1.5.5-rc2-windows-x64.zip` package contains:

| File | Purpose |
|---|---|
| `bin/engram.exe` | Compiled, standalone Windows x64 binary (SHA-256: `BB0AAA...`) |
| `README.md` | Core product documentation and quickstart instructions |
| `LICENSE` | Full legal text of the GNU Affero General Public License v3.0 |
| `THIRD_PARTY_NOTICES.md` | Complete third-party license texts, notices, and attribution register |
| `RELEASE_NOTES.md` | Detailed changes, benchmark results, and verification records for RC2 |
| `package_manifest.json` | Machine-readable package metadata declaring `AGPL-3.0-or-later` |
| `SHA256SUMS.txt` | Cryptographic verification manifest for all packaged files |

---

## 3. Secondary Method: Building from Source

To build Engram from source on Windows, macOS, or Linux:

### Prerequisites
- **Rust Toolchain:** Rust 1.85+ (or 1.99+) with `cargo`. Install via [rustup.rs](https://rustup.rs/).
- **C/C++ Compiler:** Required for compiling embedded Tree-sitter C grammar libraries:
  - Windows: Visual Studio C++ Build Tools (MSVC)
  - Linux: `gcc` or `clang`, `build-essential`
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
- **Git:** Required for repository metadata queries.

### Build Steps

```bash
# 1. Clone repository
git clone https://github.com/code-amalps/Engram.git
cd Engram

# 2. Check compilation
cargo check

# 3. Run test suite
cargo test

# 4. Build optimized release binary
cargo build --release
```

The compiled binary will be located at `target/release/engram` (or `target/release/engram.exe` on Windows).

---

## 4. Post-Installation Verification

Confirm your installation is functional by querying the version and running diagnostic checks:

```powershell
# 1. Verify binary version output
engram --version
# Expected: engram 1.5.5-rc2 (or corresponding release version)

# 2. Run system doctor
engram doctor
# Expected: All system checks PASS
```\n