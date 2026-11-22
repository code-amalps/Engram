#!/usr/bin/env python3
"""
Engram Clean-Checkout Validation Script
Automates the clean-worktree checkout, offline build, metadata audit,
and regression test execution to establish a verifiable baseline.
"""

import subprocess
import os
import sys
import shutil
import hashlib
import json
import time

def main():
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    temp_worktree = os.path.join(os.environ.get('LOCALAPPDATA', os.environ.get('TEMP', '/tmp')), 'engram_clean_val_' + str(int(time.time())))
    
    print("=" * 60)
    print("Engram Clean-Checkout Validation Harness")
    print(f"Source Repository: {repo_root}")
    print(f"Validation Worktree: {temp_worktree}")
    print("=" * 60)

    try:
        # 1. Verify source repository status
        print("\n[Step 1/5] Verifying repository status...")
        head_commit = subprocess.check_output(['git', '-C', repo_root, 'rev-parse', 'HEAD'], text=True).strip()
        print(f"  Current HEAD commit: {head_commit}")

        # 2. Create clean worktree
        print("\n[Step 2/5] Creating clean isolated git worktree...")
        subprocess.check_call(['git', '-C', repo_root, 'worktree', 'add', '--force', temp_worktree, head_commit])
        
        status_out = subprocess.check_output(['git', '-C', temp_worktree, 'status', '--porcelain'], text=True).strip()
        if status_out:
            print(f"  ERROR: Worktree is not clean:\n{status_out}")
            return 1
        print("  Worktree confirmed 100% clean (zero uncommitted changes).")

        # 3. Compile in locked offline mode
        print("\n[Step 3/5] Building release binary in locked offline mode...")
        build_cmd = ['cargo', 'build', '--release', '--locked', '--offline']
        subprocess.check_call(build_cmd, cwd=temp_worktree)
        print("  Build completed successfully.")

        bin_name = 'engram.exe' if sys.platform == 'win32' else 'engram'
        bin_path = os.path.join(temp_worktree, 'target', 'release', bin_name)
        if not os.path.exists(bin_path):
            print(f"  ERROR: Binary not found at {bin_path}")
            return 1

        with open(bin_path, 'rb') as f:
            bin_sha = hashlib.sha256(f.read()).hexdigest().upper()
        bin_size = os.path.getsize(bin_path)
        print(f"  Binary size: {bin_size} bytes")
        print(f"  Binary SHA-256: {bin_sha}")

        # 4. Verify embedded metadata
        print("\n[Step 4/5] Auditing embedded binary metadata...")
        ver_out = subprocess.check_output([bin_path, '--version'], text=True).strip()
        print(f"  Output of {bin_name} --version:\n{ver_out}")
        
        if 'dirty: true' in ver_out:
            print("  ERROR: Binary reports dirty: true! Expected clean build.")
            return 1
        if head_commit[:7] not in ver_out and 'commit:' not in ver_out:
            print(f"  WARNING: Expected commit prefix not found in version output.")

        # 5. Run tests
        print("\n[Step 5/5] Executing test suite in offline mode...")
        test_cmd = ['cargo', 'test', '--release', '--locked', '--offline']
        subprocess.check_call(test_cmd, cwd=temp_worktree)
        print("  All tests passed successfully.")

        print("\n" + "=" * 60)
        print("VALIDATION SUCCESS: Clean-checkout baseline established.")
        print(f"HEAD Commit: {head_commit}")
        print(f"Binary SHA-256: {bin_sha}")
        print("=" * 60)
        return 0

    finally:
        print("\nCleaning up validation worktree...")
        try:
            subprocess.run(['git', '-C', repo_root, 'worktree', 'remove', '--force', temp_worktree], capture_output=True)
            if os.path.exists(temp_worktree):
                shutil.rmtree(temp_worktree, ignore_errors=True)
            print("Cleanup completed.")
        except Exception as e:
            print(f"Cleanup note: {e}")

if __name__ == '__main__':
    sys.exit(main())
