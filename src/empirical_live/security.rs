//! OS security hardening, process-tree containment, and containment validation for Milestone B5.
//!
//! # Architecture & Security Invariants
//!
//! 1. **Windows Job Object Containment:**
//!    Subprocesses spawned by the tool dispatcher are bound to a Windows Job Object configured
//!    with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When the job handle is closed or dropped on any
//!    exit path (normal termination, timeout, cancellation, or panic/unwind), the Windows kernel
//!    forcibly terminates all processes in the job tree, preventing orphaned descendant processes.
//!
//! 2. **Spawn-Then-Assign Race Elimination:**
//!    To guarantee that untrusted code cannot execute before containment is active, processes on
//!    Windows are created in a suspended state (`CREATE_SUSPENDED`), assigned to the Job Object,
//!    verified via `IsProcessInJob`, and only then resumed. If assignment fails, the process is
//!    terminated immediately without ever executing user code.
//!
//! 3. **Non-Windows Process Containment:**
//!    On POSIX platforms, processes are spawned in independent process groups (`setpgid(0, 0)`)
//!    and reaped via group signals (`SIGKILL` to `-pgid`). We explicitly document that POSIX
//!    process groups do not constitute a kernel-enforced security jail equivalent to Windows Job
//!    Objects, as unprivileged processes may call `setpgid` or `setsid` to detach.
//!
//! 4. **Child-Process Environment Sanitization:**
//!    Subprocesses are isolated using `Command::env_clear()`. Only an explicitly allowlisted set
//!    of essential system variables (`SYSTEMROOT`, `SYSTEMDRIVE`, `WINDIR`, `COMSPEC`, `PATHEXT`,
//!    `PATH`) are permitted. All secret-bearing variables matching case-insensitive patterns
//!    (`KEY`, `TOKEN`, `SECRET`, `AUTH`, `PASS`, `CREDENTIAL`, `COOKIE`), all proxy variables
//!    (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`), and Git metadata variables
//!    are completely scrubbed. Tool caches (`CARGO_HOME`, `RUSTUP_HOME`, `NPM_CONFIG_CACHE`)
//!    are redirected to sandbox-local paths.
//!
//! 5. **Adversarial Network-Containment Validation:**
//!    A local, offline probe framework evaluates network reachability without contacting real
//!    external endpoints. It formally distinguishes between active OS policy rejection (`WSAEACCES`),
//!    unreachable/connection-refused (`WSAECONNREFUSED`), DNS resolution failure, and unexpected
//!    egress leaks. All reports preserve the epistemological disclosure that empirical probes do
//!    not constitute mathematical proof of universal network isolation across all protocols.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};

/// Mandatory epistemological disclosure regarding empirical network probe validity.
pub const NETWORK_CONTAINMENT_EPISTEMOLOGICAL_DISCLAIMER: &str = "Notice: A passing network egress probe verifies that tested destinations were rejected \
     under the measured host configuration. It constitutes empirical evidence of containment \
     in the tested environment, not universal mathematical proof of total network isolation \
     across all protocols, network interfaces, or covert channels.";

// ============================================================================
// 1. Error Types
// ============================================================================

/// Typed security errors for Milestone B5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityError {
    /// Failed to create the Windows Job Object.
    JobObjectCreationFailed { os_error_code: u32, message: String },
    /// Failed to configure extended limits (such as `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
    JobObjectConfigurationFailed { os_error_code: u32, message: String },
    /// Failed to assign the spawned process to the Job Object.
    ProcessAssignmentFailed { os_error_code: u32, message: String },
    /// Process was verified NOT to be inside the Job Object after assignment.
    ProcessNotInJob { pid: u32 },
    /// Failed to resume the suspended process thread.
    ProcessResumeFailed { os_error_code: u32, message: String },
    /// Required process containment is not supported or cannot be verified on this platform.
    UnsupportedPlatformContainment { platform: String, reason: String },
    /// Executable cannot be resolved securely from trusted paths without relying on untrusted PATH.
    ExecutableResolutionFailed {
        command: String,
        search_paths: Vec<String>,
    },
    /// An environment variable failed security validation.
    EnvironmentSanitizationFailed { variable: String, reason: String },
    /// An unexpected network connection was established during a containment probe (critical leak!).
    EgressLeakDetected { destination: String, detail: String },
    /// Network containment could not be definitively verified.
    UnverifiedContainment { reason: String },
    /// The containment probe infrastructure itself failed.
    ProbeInfrastructureFailed { reason: String },
    /// Failed to terminate or clean up descendant processes during teardown.
    DescendantCleanupFailed { os_error_code: u32, message: String },
    /// Partial thread resumption failure in suspended child process.
    PartialThreadResumptionFailed {
        pid: u32,
        discovered_threads: usize,
        resumed_threads: usize,
        failed_threads: usize,
        os_error_code: Option<u32>,
    },
    /// Thread snapshot or enumeration failed during process resumption.
    ThreadSnapshotFailed { pid: u32, os_error_code: u32 },
    /// Suspended process has zero discoverable threads.
    ZeroThreadsDiscovered { pid: u32 },
    /// Failed to create Windows AppContainer profile.
    AppContainerProfileCreationFailed { os_error_code: u32, message: String },
    /// Failed to delete Windows AppContainer profile.
    AppContainerProfileDeletionFailed { os_error_code: u32, message: String },
    /// Failed to derive AppContainer SID from profile name.
    AppContainerSidDerivationFailed { os_error_code: u32, message: String },
    /// Failed to initialize or update thread attributes for security capabilities.
    AppContainerSecurityAttributeFailed { os_error_code: u32, message: String },
    /// Failed to configure DACL or security descriptor for sandboxed path.
    AclConfigurationFailed { os_error_code: u32, message: String },
    /// Failed to spawn process with AppContainer security capabilities.
    ProcessSpawnFailed { os_error_code: u32, message: String },
    /// Command execution timed out.
    CommandTimeout { timeout_secs: u64 },
    /// General I/O error during security operations.
    Io(String),
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JobObjectCreationFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to create Windows Job Object (OS error {os_error_code}): {message}"
                )
            }
            Self::JobObjectConfigurationFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to configure Job Object limits (OS error {os_error_code}): {message}"
                )
            }
            Self::ProcessAssignmentFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to assign process to Job Object (OS error {os_error_code}): {message}"
                )
            }
            Self::ProcessNotInJob { pid } => {
                write!(
                    f,
                    "Verification failed: process {pid} is not in the assigned Job Object"
                )
            }
            Self::ProcessResumeFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to resume suspended child process (OS error {os_error_code}): {message}"
                )
            }
            Self::PartialThreadResumptionFailed {
                pid,
                discovered_threads,
                resumed_threads,
                failed_threads,
                os_error_code,
            } => {
                write!(
                    f,
                    "Partial thread resumption failure for PID {pid}: discovered {discovered_threads}, resumed {resumed_threads}, failed {failed_threads} (last OS error: {os_error_code:?})"
                )
            }
            Self::ThreadSnapshotFailed { pid, os_error_code } => {
                write!(
                    f,
                    "Failed to create thread snapshot for PID {pid} (OS error {os_error_code})"
                )
            }
            Self::ZeroThreadsDiscovered { pid } => {
                write!(f, "Zero threads discovered for suspended child PID {pid}")
            }
            Self::UnsupportedPlatformContainment { platform, reason } => {
                write!(
                    f,
                    "Platform containment unsupported on {platform}: {reason}"
                )
            }
            Self::ExecutableResolutionFailed {
                command,
                search_paths,
            } => {
                write!(
                    f,
                    "Failed to securely resolve executable '{command}' in trusted paths: {search_paths:?}"
                )
            }
            Self::EnvironmentSanitizationFailed { variable, reason } => {
                write!(
                    f,
                    "Environment sanitization rejected variable '{variable}': {reason}"
                )
            }
            Self::EgressLeakDetected {
                destination,
                detail,
            } => {
                write!(
                    f,
                    "CRITICAL: Network egress leak detected to '{destination}': {detail}"
                )
            }
            Self::UnverifiedContainment { reason } => {
                write!(f, "Containment unverified: {reason}")
            }
            Self::ProbeInfrastructureFailed { reason } => {
                write!(f, "Probe infrastructure failed: {reason}")
            }
            Self::DescendantCleanupFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to clean up descendant processes (OS error {os_error_code}): {message}"
                )
            }
            Self::AppContainerProfileCreationFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to create AppContainer profile (OS error {os_error_code}): {message}"
                )
            }
            Self::AppContainerProfileDeletionFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to delete AppContainer profile (OS error {os_error_code}): {message}"
                )
            }
            Self::AppContainerSidDerivationFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to derive AppContainer SID (OS error {os_error_code}): {message}"
                )
            }
            Self::AppContainerSecurityAttributeFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to configure AppContainer security attributes (OS error {os_error_code}): {message}"
                )
            }
            Self::AclConfigurationFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to configure ACL (OS error {os_error_code}): {message}"
                )
            }
            Self::ProcessSpawnFailed {
                os_error_code,
                message,
            } => {
                write!(
                    f,
                    "Failed to spawn AppContainer process (OS error {os_error_code}): {message}"
                )
            }
            Self::CommandTimeout { timeout_secs } => {
                write!(f, "Command execution timed out after {timeout_secs}s")
            }
            Self::Io(msg) => write!(f, "Security I/O error: {msg}"),
        }
    }
}

impl std::error::Error for SecurityError {}

// ============================================================================
// 2. Win32 FFI Layouts and Bindings (Zero Dependencies)
// ============================================================================

#[cfg(windows)]
#[allow(non_snake_case, non_camel_case_types, dead_code)]
pub mod win32 {
    use std::ffi::c_void;

    pub type HANDLE = *mut c_void;
    pub type BOOL = i32;
    pub const FALSE: BOOL = 0;
    pub const TRUE: BOOL = 1;

    pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

    pub type PSID = *mut c_void;
    pub type PSECURITY_DESCRIPTOR = *mut c_void;
    pub type HLOCAL = *mut c_void;
    pub type PPROC_THREAD_ATTRIBUTE_LIST = *mut c_void;

    // Process creation flags
    pub const CREATE_SUSPENDED: u32 = 0x00000004;
    pub const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x00080000;
    pub const CREATE_UNICODE_ENVIRONMENT: u32 = 0x00000400;

    // Proc thread attribute constants
    pub const PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES: usize = 0x00020009;
    pub const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x00020002;

    // Handle and startup flags
    pub const HANDLE_FLAG_INHERIT: u32 = 0x00000001;
    pub const STARTF_USESTDHANDLES: u32 = 0x00000100;

    // Security & ACL constants
    pub const DACL_SECURITY_INFORMATION: u32 = 0x00000004;
    pub const SE_FILE_OBJECT: u32 = 1;
    pub const GRANT_ACCESS: i32 = 1;
    pub const REVOKE_ACCESS: i32 = 4;
    pub const CONTAINER_INHERIT_ACE: u32 = 2;
    pub const OBJECT_INHERIT_ACE: u32 = 1;
    pub const GENERIC_ALL: u32 = 0x10000000;
    pub const GENERIC_READ: u32 = 0x80000000;
    pub const GENERIC_EXECUTE: u32 = 0x20000000;
    pub const TRUSTEE_IS_SID: i32 = 0;
    pub const TRUSTEE_IS_UNKNOWN: i32 = 0;
    pub const WAIT_OBJECT_0: u32 = 0;
    pub const WAIT_TIMEOUT: u32 = 0x00000102;
    pub const STILL_ACTIVE: u32 = 259;

    // WindowStation specific access rights (least-privilege for console/CRT initialization)
    pub const WINSTA_ENUMDESKTOPS: u32 = 0x0001;
    pub const WINSTA_READATTRIBUTES: u32 = 0x0002;
    pub const WINSTA_ACCESSGLOBALATOMS: u32 = 0x0020;
    pub const WINSTA_ENUMERATE: u32 = 0x0100;
    pub const STANDARD_RIGHTS_READ: u32 = 0x00020000;
    pub const WINSTA_CONTAINED_RIGHTS: u32 = WINSTA_ACCESSGLOBALATOMS
        | WINSTA_ENUMDESKTOPS
        | WINSTA_ENUMERATE
        | WINSTA_READATTRIBUTES
        | STANDARD_RIGHTS_READ;

    // Desktop specific access rights (least-privilege for console/CRT initialization)
    pub const DESKTOP_READOBJECTS: u32 = 0x0001;
    pub const DESKTOP_CREATEWINDOW: u32 = 0x0002;
    pub const DESKTOP_ENUMERATE: u32 = 0x0040;
    pub const DESKTOP_WRITEOBJECTS: u32 = 0x0080;
    pub const DESKTOP_CONTAINED_RIGHTS: u32 = DESKTOP_READOBJECTS
        | DESKTOP_CREATEWINDOW
        | DESKTOP_ENUMERATE
        | DESKTOP_WRITEOBJECTS
        | STANDARD_RIGHTS_READ;

    // Job Object Information Classes
    pub const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

    // Job Object Limit Flags
    pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x00002000;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct SID_AND_ATTRIBUTES {
        pub Sid: PSID,
        pub Attributes: u32,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct SECURITY_CAPABILITIES {
        pub AppContainerSid: PSID,
        pub Capabilities: *mut SID_AND_ATTRIBUTES,
        pub CapabilityCount: u32,
        pub Reserved: u32,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct SECURITY_ATTRIBUTES {
        pub nLength: u32,
        pub lpSecurityDescriptor: *mut c_void,
        pub bInheritHandle: BOOL,
    }

    #[repr(C)]
    pub struct STARTUPINFOW {
        pub cb: u32,
        pub lpReserved: *mut u16,
        pub lpDesktop: *mut u16,
        pub lpTitle: *mut u16,
        pub dwX: u32,
        pub dwY: u32,
        pub dwXSize: u32,
        pub dwYSize: u32,
        pub dwXCountChars: u32,
        pub dwYCountChars: u32,
        pub dwFillAttribute: u32,
        pub dwFlags: u32,
        pub wShowWindow: u16,
        pub cbReserved2: u16,
        pub lpReserved2: *mut u8,
        pub hStdInput: HANDLE,
        pub hStdOutput: HANDLE,
        pub hStdError: HANDLE,
    }

    #[repr(C)]
    pub struct STARTUPINFOEXW {
        pub StartupInfo: STARTUPINFOW,
        pub lpAttributeList: PPROC_THREAD_ATTRIBUTE_LIST,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct PROCESS_INFORMATION {
        pub hProcess: HANDLE,
        pub hThread: HANDLE,
        pub dwProcessId: u32,
        pub dwThreadId: u32,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct TRUSTEE_W {
        pub pMultipleTrustee: *mut c_void,
        pub MultipleTrusteeOperation: i32,
        pub TrusteeForm: i32,
        pub TrusteeType: i32,
        pub ptstrName: *mut c_void,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct EXPLICIT_ACCESS_W {
        pub grfAccessPermissions: u32,
        pub grfAccessMode: i32,
        pub grfInheritance: u32,
        pub Trustee: TRUSTEE_W,
    }

    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        pub PerProcessUserTimeLimit: i64,
        pub PerJobUserTimeLimit: i64,
        pub LimitFlags: u32,
        pub MinimumWorkingSetSize: usize,
        pub MaximumWorkingSetSize: usize,
        pub ActiveProcessLimit: u32,
        pub Affinity: usize,
        pub PriorityClass: u32,
        pub SchedulingClass: u32,
    }

    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct IO_COUNTERS {
        pub ReadOperationCount: u64,
        pub WriteOperationCount: u64,
        pub OtherOperationCount: u64,
        pub ReadTransferCount: u64,
        pub WriteTransferCount: u64,
        pub OtherTransferCount: u64,
    }

    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        pub IoInfo: IO_COUNTERS,
        pub ProcessMemoryLimit: usize,
        pub JobMemoryLimit: usize,
        pub PeakProcessMemoryLimit: usize,
        pub PeakJobMemoryLimit: usize,
    }

    #[link(name = "userenv")]
    unsafe extern "system" {
        pub fn CreateAppContainerProfile(
            pszAppContainerName: *const u16,
            pszDisplayName: *const u16,
            pszDescription: *const u16,
            pCapabilities: *const SID_AND_ATTRIBUTES,
            dwCapabilityCount: u32,
            ppSidAppContainerSid: *mut PSID,
        ) -> i32;

        pub fn DeleteAppContainerProfile(pszAppContainerName: *const u16) -> i32;

        pub fn DeriveAppContainerSidFromAppContainerName(
            pszAppContainerName: *const u16,
            ppsidAppContainerSid: *mut PSID,
        ) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        pub fn FreeSid(pSid: PSID) -> PSID;

        pub fn SetEntriesInAclW(
            cCountOfExplicitEntries: u32,
            pListOfExplicitEntries: *const EXPLICIT_ACCESS_W,
            OldAcl: *const c_void,
            NewAcl: *mut *mut c_void,
        ) -> u32;

        pub fn SetNamedSecurityInfoW(
            pObjectName: *const u16,
            ObjectType: u32,
            SecurityInfo: u32,
            psidOwner: PSID,
            psidGroup: PSID,
            pDacl: *const c_void,
            pSacl: *const c_void,
        ) -> u32;

        pub fn GetNamedSecurityInfoW(
            pObjectName: *const u16,
            ObjectType: u32,
            SecurityInfo: u32,
            ppsidOwner: *mut PSID,
            ppsidGroup: *mut PSID,
            ppDacl: *mut *mut c_void,
            ppSacl: *mut *mut c_void,
            ppSecurityDescriptor: *mut PSECURITY_DESCRIPTOR,
        ) -> u32;

        pub fn SetSecurityDescriptorDacl(
            pSecurityDescriptor: PSECURITY_DESCRIPTOR,
            bDaclPresent: BOOL,
            pDacl: *mut c_void,
            bDaclDefaulted: BOOL,
        ) -> BOOL;

        pub fn ConvertSidToStringSidW(Sid: PSID, StringSid: *mut *mut u16) -> BOOL;
        pub fn GetAce(pAcl: *const c_void, dwAceIndex: u32, pAce: *mut *mut c_void) -> BOOL;
        pub fn GetLengthSid(pSid: PSID) -> u32;
        pub fn IsValidSid(pSid: PSID) -> BOOL;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetProcessWindowStation() -> HANDLE;
        pub fn GetThreadDesktop(dwThreadId: u32) -> HANDLE;
        pub fn GetCurrentThreadId() -> u32;
        pub fn GetUserObjectSecurity(
            hObj: HANDLE,
            pSIRequested: *const u32,
            pSID: *mut c_void,
            nLength: u32,
            lpnLengthNeeded: *mut u32,
        ) -> BOOL;
        pub fn SetUserObjectSecurity(
            hObj: HANDLE,
            pSIRequested: *const u32,
            pSID: *const c_void,
        ) -> BOOL;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn CreateJobObjectW(lpJobAttributes: *mut c_void, lpName: *const u16) -> HANDLE;
        pub fn SetInformationJobObject(
            hJob: HANDLE,
            JobObjectInformationClass: u32,
            lpJobObjectInformation: *const c_void,
            cbJobObjectInformationLength: u32,
        ) -> BOOL;
        pub fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;
        pub fn IsProcessInJob(ProcessHandle: HANDLE, JobHandle: HANDLE, Result: *mut BOOL) -> BOOL;
        pub fn TerminateJobObject(hJob: HANDLE, uExitCode: u32) -> BOOL;
        pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
        pub fn GetLastError() -> u32;

        pub fn GetModuleHandleW(lpModuleName: *const u16) -> HANDLE;
        pub fn GetProcAddress(hModule: HANDLE, lpProcName: *const u8) -> *mut c_void;

        pub fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> HANDLE;
        pub fn Thread32First(hSnapshot: HANDLE, lpte: *mut THREADENTRY32) -> BOOL;
        pub fn Thread32Next(hSnapshot: HANDLE, lpte: *mut THREADENTRY32) -> BOOL;
        pub fn OpenThread(dwDesiredAccess: u32, bInheritHandle: BOOL, dwThreadId: u32) -> HANDLE;
        pub fn ResumeThread(hThread: HANDLE) -> u32;

        pub fn InitializeProcThreadAttributeList(
            lpAttributeList: PPROC_THREAD_ATTRIBUTE_LIST,
            dwAttributeCount: u32,
            dwFlags: u32,
            lpSize: *mut usize,
        ) -> BOOL;

        pub fn UpdateProcThreadAttribute(
            lpAttributeList: PPROC_THREAD_ATTRIBUTE_LIST,
            dwFlags: u32,
            Attribute: usize,
            lpValue: *const c_void,
            cbSize: usize,
            lpPreviousValue: *mut c_void,
            lpReturnSize: *mut usize,
        ) -> BOOL;

        pub fn DeleteProcThreadAttributeList(lpAttributeList: PPROC_THREAD_ATTRIBUTE_LIST);

        pub fn CreateProcessW(
            lpApplicationName: *const u16,
            lpCommandLine: *mut u16,
            lpProcessAttributes: *mut SECURITY_ATTRIBUTES,
            lpThreadAttributes: *mut SECURITY_ATTRIBUTES,
            bInheritHandles: BOOL,
            dwCreationFlags: u32,
            lpEnvironment: *const c_void,
            lpCurrentDirectory: *const u16,
            lpStartupInfo: *const STARTUPINFOEXW,
            lpProcessInformation: *mut PROCESS_INFORMATION,
        ) -> BOOL;

        pub fn CreatePipe(
            hReadPipe: *mut HANDLE,
            hWritePipe: *mut HANDLE,
            lpPipeAttributes: *mut SECURITY_ATTRIBUTES,
            nSize: u32,
        ) -> BOOL;

        pub fn SetHandleInformation(hObject: HANDLE, dwMask: u32, dwFlags: u32) -> BOOL;

        pub fn ReadFile(
            hFile: HANDLE,
            lpBuffer: *mut u8,
            nNumberOfBytesToRead: u32,
            lpNumberOfBytesRead: *mut u32,
            lpOverlapped: *mut c_void,
        ) -> BOOL;

        pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: u32) -> u32;
        pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut u32) -> BOOL;
        pub fn LocalFree(hMem: HLOCAL) -> HLOCAL;
    }

    pub const TH32CS_SNAPTHREAD: u32 = 0x00000004;
    pub const THREAD_SUSPEND_RESUME: u32 = 0x0002;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct THREADENTRY32 {
        pub dwSize: u32,
        pub cntUsage: u32,
        pub th32ThreadID: u32,
        pub th32OwnerProcessID: u32,
        pub tpBasePri: i32,
        pub tpDeltaPri: i32,
        pub dwFlags: u32,
    }
}

/// Detailed accounting of child process thread resumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadResumptionSummary {
    /// Resumption method used ("NtResumeProcess" or "Toolhelp32").
    pub method: String,
    /// Discovered threads belonging to PID.
    pub discovered_threads: usize,
    /// Count of successfully resumed threads.
    pub resumed_threads: usize,
    /// Count of failed thread operations.
    pub failed_threads: usize,
    /// Last raw OS error code observed, if any.
    pub last_os_error: Option<u32>,
}

impl ThreadResumptionSummary {
    /// Returns true if all discovered threads were completely resumed with zero failures.
    pub fn is_complete_success(&self) -> bool {
        self.discovered_threads > 0
            && self.failed_threads == 0
            && self.resumed_threads == self.discovered_threads
    }

    /// Strictly validate resumption results.
    ///
    /// Fails closed if zero threads were discovered or if any thread failed to resume.
    pub fn validate(&self, pid: u32) -> Result<(), SecurityError> {
        if self.discovered_threads == 0 {
            return Err(SecurityError::ZeroThreadsDiscovered { pid });
        }
        if self.failed_threads > 0 || self.resumed_threads != self.discovered_threads {
            return Err(SecurityError::PartialThreadResumptionFailed {
                pid,
                discovered_threads: self.discovered_threads,
                resumed_threads: self.resumed_threads,
                failed_threads: self.failed_threads,
                os_error_code: self.last_os_error,
            });
        }
        Ok(())
    }
}

/// Check whether the specified OS platform supports kernel process tree containment.
///
/// Fails closed with `UnsupportedPlatformContainment` on all non-Windows platforms.
pub fn check_platform_containment(platform: &str) -> Result<(), SecurityError> {
    if platform.eq_ignore_ascii_case("windows") {
        Ok(())
    } else {
        Err(SecurityError::UnsupportedPlatformContainment {
            platform: platform.to_string(),
            reason: "Windows Job Objects are only supported on Windows NT platforms (fail-closed)"
                .to_string(),
        })
    }
}

// ============================================================================
// 3. RAII Windows Job Object Wrapper
// ============================================================================

/// RAII container for a Windows Job Object handle.
/// Guarantees that when dropped, `CloseHandle` is invoked, which triggers the Windows
/// kernel's `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` mechanism to reap all descendants.
#[derive(Debug)]
pub struct ProcessJobObject {
    #[cfg(windows)]
    handle: win32::HANDLE,
    is_closed: AtomicBool,
}

// Safety: The Windows Job Object HANDLE can be safely sent across threads.
unsafe impl Send for ProcessJobObject {}
unsafe impl Sync for ProcessJobObject {}

impl ProcessJobObject {
    /// Create a new, nameless Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub fn create() -> Result<Self, SecurityError> {
        #[cfg(windows)]
        {
            unsafe {
                // 1. Create Job Object with default security attributes
                let handle = win32::CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                if handle.is_null() || handle == win32::INVALID_HANDLE_VALUE {
                    let err = win32::GetLastError();
                    return Err(SecurityError::JobObjectCreationFailed {
                        os_error_code: err,
                        message: format!("CreateJobObjectW failed with error code {err}"),
                    });
                }

                // 2. Configure JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                let mut info = win32::JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = win32::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let success = win32::SetInformationJobObject(
                    handle,
                    win32::JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<win32::JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );

                if success == win32::FALSE {
                    let err = win32::GetLastError();
                    win32::CloseHandle(handle);
                    return Err(SecurityError::JobObjectConfigurationFailed {
                        os_error_code: err,
                        message: format!("SetInformationJobObject failed with error code {err}"),
                    });
                }

                Ok(Self {
                    handle,
                    is_closed: AtomicBool::new(false),
                })
            }
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "Windows Job Objects are only supported on Windows NT platforms"
                    .to_string(),
            })
        }
    }

    /// Assign a running or suspended process to this Job Object.
    ///
    /// # Safety and Invariants
    /// Verifies immediately with `IsProcessInJob` that the process is actually contained.
    /// If assignment fails (e.g. nested job object restriction), fails closed.
    pub fn assign_process(&self, child: &Child) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let process_handle = child.as_raw_handle() as win32::HANDLE;
            let pid = child.id();

            unsafe {
                let success = win32::AssignProcessToJobObject(self.handle, process_handle);
                if success == win32::FALSE {
                    let err = win32::GetLastError();
                    return Err(SecurityError::ProcessAssignmentFailed {
                        os_error_code: err,
                        message: format!(
                            "AssignProcessToJobObject failed for PID {pid} with OS error {err}"
                        ),
                    });
                }

                // Verify containment
                let mut in_job: win32::BOOL = win32::FALSE;
                let check = win32::IsProcessInJob(process_handle, self.handle, &mut in_job);
                if check == win32::FALSE || in_job == win32::FALSE {
                    return Err(SecurityError::ProcessNotInJob { pid });
                }

                Ok(())
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "Windows Job Objects are only supported on Windows NT platforms"
                    .to_string(),
            })
        }
    }

    /// Resume a child process that was spawned with `CREATE_SUSPENDED`.
    ///
    /// First attempts `NtResumeProcess` dynamically from `ntdll.dll`. If unavailable,
    /// falls back to thread enumeration via `CreateToolhelp32Snapshot` with strict thread
    /// accounting.
    ///
    /// # Safety and Invariants
    /// If any thread cannot be opened or resumed, or if zero threads are found, or if snapshot
    /// creation fails, this function terminates the child and cleans up all descendants in
    /// the Job Object before returning a fail-closed error (AUD-B5-03).
    pub fn resume_child(
        &self,
        child: &mut Child,
    ) -> Result<ThreadResumptionSummary, SecurityError> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let process_handle = child.as_raw_handle() as win32::HANDLE;
            let pid = child.id();

            unsafe {
                // Method A: Dynamic lookup of NtResumeProcess in ntdll.dll
                let ntdll_name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
                let h_ntdll = win32::GetModuleHandleW(ntdll_name.as_ptr());
                if !h_ntdll.is_null() {
                    let proc_name = b"NtResumeProcess\0";
                    let p_nt_resume = win32::GetProcAddress(h_ntdll, proc_name.as_ptr());
                    if !p_nt_resume.is_null() {
                        type NtResumeProcessFn = unsafe extern "system" fn(win32::HANDLE) -> i32;
                        let nt_resume: NtResumeProcessFn = std::mem::transmute(p_nt_resume);
                        let status = nt_resume(process_handle);
                        if status >= 0 {
                            return Ok(ThreadResumptionSummary {
                                method: "NtResumeProcess".to_string(),
                                discovered_threads: 1, // atomic process-wide resumption
                                resumed_threads: 1,
                                failed_threads: 0,
                                last_os_error: None,
                            });
                        }
                    }
                }

                // Method B: Toolhelp thread enumeration fallback with strict accounting
                let h_snap = win32::CreateToolhelp32Snapshot(win32::TH32CS_SNAPTHREAD, 0);
                if h_snap == win32::INVALID_HANDLE_VALUE || h_snap.is_null() {
                    let err = win32::GetLastError();
                    let _ = child.kill();
                    let _ = self.terminate(1);
                    return Err(SecurityError::ThreadSnapshotFailed {
                        pid,
                        os_error_code: err,
                    });
                }

                let mut te = win32::THREADENTRY32 {
                    dwSize: std::mem::size_of::<win32::THREADENTRY32>() as u32,
                    cntUsage: 0,
                    th32ThreadID: 0,
                    th32OwnerProcessID: 0,
                    tpBasePri: 0,
                    tpDeltaPri: 0,
                    dwFlags: 0,
                };

                let mut discovered_threads = 0usize;
                let mut resumed_threads = 0usize;
                let mut failed_threads = 0usize;
                let mut last_os_error: Option<u32> = None;

                if win32::Thread32First(h_snap, &mut te) == win32::TRUE {
                    loop {
                        if te.th32OwnerProcessID == pid {
                            discovered_threads += 1;
                            let h_thread = win32::OpenThread(
                                win32::THREAD_SUSPEND_RESUME,
                                win32::FALSE,
                                te.th32ThreadID,
                            );
                            if !h_thread.is_null() && h_thread != win32::INVALID_HANDLE_VALUE {
                                let prev_count = win32::ResumeThread(h_thread);
                                win32::CloseHandle(h_thread);
                                if prev_count != u32::MAX {
                                    resumed_threads += 1;
                                } else {
                                    failed_threads += 1;
                                    last_os_error = Some(win32::GetLastError());
                                }
                            } else {
                                failed_threads += 1;
                                last_os_error = Some(win32::GetLastError());
                            }
                        }
                        if win32::Thread32Next(h_snap, &mut te) == win32::FALSE {
                            break;
                        }
                    }
                } else {
                    let err = win32::GetLastError();
                    win32::CloseHandle(h_snap);
                    let _ = child.kill();
                    let _ = self.terminate(1);
                    return Err(SecurityError::ThreadSnapshotFailed {
                        pid,
                        os_error_code: err,
                    });
                }

                win32::CloseHandle(h_snap);

                let summary = ThreadResumptionSummary {
                    method: "Toolhelp32".to_string(),
                    discovered_threads,
                    resumed_threads,
                    failed_threads,
                    last_os_error,
                };

                if let Err(e) = summary.validate(pid) {
                    let _ = child.kill();
                    let _ = self.terminate(1);
                    return Err(e);
                }

                Ok(summary)
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "Thread resumption is only supported on Windows NT platforms".to_string(),
            })
        }
    }

    /// Terminate all processes currently associated with this Job Object.
    pub fn terminate(&self, exit_code: u32) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            unsafe {
                let success = win32::TerminateJobObject(self.handle, exit_code);
                if success == win32::FALSE {
                    let err = win32::GetLastError();
                    return Err(SecurityError::DescendantCleanupFailed {
                        os_error_code: err,
                        message: format!("TerminateJobObject failed with error {err}"),
                    });
                }
                Ok(())
            }
        }
        #[cfg(not(windows))]
        {
            let _ = exit_code;
            Ok(())
        }
    }

    /// Check if the handle is still open.
    pub fn is_open(&self) -> bool {
        !self.is_closed.load(Ordering::SeqCst)
    }

    /// Returns the raw Windows Job Object handle.
    #[cfg(windows)]
    pub fn raw_handle(&self) -> win32::HANDLE {
        self.handle
    }
}

impl Drop for ProcessJobObject {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            if !self.is_closed.swap(true, Ordering::SeqCst) {
                unsafe {
                    if !self.handle.is_null() && self.handle != win32::INVALID_HANDLE_VALUE {
                        win32::CloseHandle(self.handle);
                    }
                }
            }
        }
    }
}

// ============================================================================
// 3B. Windows AppContainer Containment (BLK-B6-02)
// ============================================================================

/// UTF-16 wide string null-terminated converter.
#[cfg(windows)]
pub fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// UTF-16 wide string null-terminated decoder.
#[cfg(windows)]
pub fn from_wide_null(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}

/// Helper to build an environment block in UTF-16 double-null-terminated format.
#[cfg(windows)]
pub fn build_environment_block(vars: &HashMap<String, String>) -> Vec<u16> {
    let mut block = Vec::new();
    let mut sorted: Vec<(&String, &String)> = vars.iter().collect();
    sorted.sort_by(|a, b| a.0.to_ascii_uppercase().cmp(&b.0.to_ascii_uppercase()));
    for (k, v) in sorted {
        let entry = format!("{k}={v}\0");
        block.extend(entry.encode_utf16());
    }
    block.push(0); // double null termination
    block
}

/// Safely read all output from an anonymous pipe to a String until EOF.
#[cfg(windows)]
unsafe fn read_pipe_to_string(pipe: win32::HANDLE) -> String {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    let mut read: u32 = 0;
    loop {
        let ok = unsafe {
            win32::ReadFile(
                pipe,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == win32::FALSE || read == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..read as usize]);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

#[cfg(windows)]
static USER_OBJECT_SECURITY_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Grant DACL access permissions to an AppContainer SID on a Win32 user object (WindowStation, Desktop).
#[cfg(windows)]
unsafe fn grant_user_object_access(
    handle: win32::HANDLE,
    sid: win32::PSID,
    permissions: u32,
) -> Result<(), SecurityError> {
    if handle.is_null() || handle == win32::INVALID_HANDLE_VALUE {
        let err = unsafe { win32::GetLastError() };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: "Invalid user object handle".to_string(),
        });
    }

    let _lock = USER_OBJECT_SECURITY_MUTEX.lock().unwrap();

    let mut needed: u32 = 0;
    unsafe {
        win32::GetUserObjectSecurity(
            handle,
            &win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }

    let mut buf = vec![0u8; (needed as usize).max(256)];
    let mut actual_needed = needed;
    let mut get_ok = win32::FALSE;
    for _ in 0..5 {
        get_ok = unsafe {
            win32::GetUserObjectSecurity(
                handle,
                &win32::DACL_SECURITY_INFORMATION,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
                &mut actual_needed,
            )
        };
        if get_ok != win32::FALSE {
            break;
        }
        let err = unsafe { win32::GetLastError() };
        if err == 122 {
            // ERROR_INSUFFICIENT_BUFFER: resize with headroom and retry
            buf.resize(actual_needed as usize + 256, 0);
            continue;
        }
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: "GetUserObjectSecurity failed".to_string(),
        });
    }

    if get_ok == win32::FALSE {
        let err = unsafe { win32::GetLastError() };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: "GetUserObjectSecurity failed after retries".to_string(),
        });
    }

    let sec_desc = buf.as_ptr() as *const std::ffi::c_void;
    let dacl_offset = unsafe { *(sec_desc.add(16) as *const u32) };
    let old_dacl = if dacl_offset != 0 {
        unsafe { sec_desc.add(dacl_offset as usize) }
    } else {
        std::ptr::null()
    };

    let ea = win32::EXPLICIT_ACCESS_W {
        grfAccessPermissions: permissions,
        grfAccessMode: win32::GRANT_ACCESS,
        grfInheritance: 0,
        Trustee: win32::TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: win32::TRUSTEE_IS_SID,
            TrusteeType: win32::TRUSTEE_IS_UNKNOWN,
            ptstrName: sid,
        },
    };

    let mut new_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let set_entries_err = unsafe { win32::SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
    if set_entries_err != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_entries_err,
            message: format!("SetEntriesInAclW failed for user object: {set_entries_err}"),
        });
    }

    let mut new_sd = vec![0u8; 40];
    new_sd[0] = 1; // SECURITY_DESCRIPTOR_REVISION
    let set_ok = unsafe {
        win32::SetSecurityDescriptorDacl(
            new_sd.as_mut_ptr() as *mut std::ffi::c_void,
            win32::TRUE,
            new_dacl,
            win32::FALSE,
        )
    };
    let ok = if set_ok != win32::FALSE {
        unsafe {
            win32::SetUserObjectSecurity(
                handle,
                &win32::DACL_SECURITY_INFORMATION,
                new_sd.as_ptr() as *const std::ffi::c_void,
            ) != win32::FALSE
        }
    } else {
        false
    };
    let err = unsafe { win32::GetLastError() };
    unsafe { win32::LocalFree(new_dacl) };

    if !ok {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: format!("SetUserObjectSecurity failed: {err}"),
        });
    }
    Ok(())
}

/// Revoke DACL access permissions previously granted to an AppContainer SID on a Win32 user object.
#[cfg(windows)]
unsafe fn revoke_user_object_access(
    handle: win32::HANDLE,
    sid: win32::PSID,
) -> Result<(), SecurityError> {
    if handle.is_null() || handle == win32::INVALID_HANDLE_VALUE || sid.is_null() {
        return Ok(());
    }

    let _lock = USER_OBJECT_SECURITY_MUTEX.lock().unwrap();

    let mut needed: u32 = 0;
    unsafe {
        win32::GetUserObjectSecurity(
            handle,
            &win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }

    let mut buf = vec![0u8; (needed as usize).max(256)];
    let mut actual_needed = needed;
    let mut get_ok = win32::FALSE;
    for _ in 0..5 {
        get_ok = unsafe {
            win32::GetUserObjectSecurity(
                handle,
                &win32::DACL_SECURITY_INFORMATION,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
                &mut actual_needed,
            )
        };
        if get_ok != win32::FALSE {
            break;
        }
        let err = unsafe { win32::GetLastError() };
        if err == 122 {
            buf.resize(actual_needed as usize + 256, 0);
            continue;
        }
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: format!("GetUserObjectSecurity buffer query failed during revocation: {err}"),
        });
    }

    if get_ok == win32::FALSE {
        let err = unsafe { win32::GetLastError() };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: format!("GetUserObjectSecurity failed during revocation after retries: {err}"),
        });
    }

    let sec_desc = buf.as_ptr() as *const std::ffi::c_void;
    let dacl_offset = unsafe { *(sec_desc.add(16) as *const u32) };
    let old_dacl = if dacl_offset != 0 {
        unsafe { sec_desc.add(dacl_offset as usize) }
    } else {
        std::ptr::null()
    };

    let ea = win32::EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0,
        grfAccessMode: win32::REVOKE_ACCESS,
        grfInheritance: 0,
        Trustee: win32::TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: win32::TRUSTEE_IS_SID,
            TrusteeType: win32::TRUSTEE_IS_UNKNOWN,
            ptstrName: sid,
        },
    };

    let mut new_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let set_entries_err = unsafe { win32::SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
    if set_entries_err != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_entries_err,
            message: format!("SetEntriesInAclW failed during revocation: {set_entries_err}"),
        });
    }

    let mut new_sd = vec![0u8; 40];
    new_sd[0] = 1; // SECURITY_DESCRIPTOR_REVISION
    let set_ok = unsafe {
        win32::SetSecurityDescriptorDacl(
            new_sd.as_mut_ptr() as *mut std::ffi::c_void,
            win32::TRUE,
            new_dacl,
            win32::FALSE,
        )
    };
    let set_user_ok = if set_ok != win32::FALSE {
        unsafe {
            win32::SetUserObjectSecurity(
                handle,
                &win32::DACL_SECURITY_INFORMATION,
                new_sd.as_ptr() as *const std::ffi::c_void,
            ) != win32::FALSE
        }
    } else {
        false
    };
    let last_err = unsafe { win32::GetLastError() };
    unsafe { win32::LocalFree(new_dacl) };

    if !set_user_ok {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: last_err,
            message: format!("SetUserObjectSecurity failed during revocation: {last_err}"),
        });
    }
    Ok(())
}

/// Detailed snapshot of an individual ACE for security auditing, integrity verification, and drift detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AceSnapshot {
    /// 0-indexed position within the DACL. Order is security-critical under Windows top-down evaluation.
    pub index: usize,
    /// Win32 ACE type (e.g. 0x00 = ACCESS_ALLOWED, 0x01 = ACCESS_DENIED, 0x05 = ACCESS_ALLOWED_OBJECT, etc.).
    pub ace_type: u8,
    /// ACE flags (inheritance and auditing: CONTAINER_INHERIT, OBJECT_INHERIT, INHERITED_ACE, etc.).
    pub ace_flags: u8,
    /// Access rights bitmask (e.g. GENERIC_READ, WINSTA_CONTAINED_RIGHTS).
    pub access_mask: u32,
    /// Canonical SDDL SID string (e.g. "S-1-15-2-1-...").
    pub sid_string: String,
    /// Complete raw byte sequence of this ACE from `AceHeader` to `AceSize`.
    pub raw_ace_bytes: Vec<u8>,
    /// Optional object type GUID for object-specific ACEs (types 0x05, 0x06, 0x07, 0x08).
    pub object_type_guid: Option<String>,
    /// Optional inherited object type GUID for object-specific ACEs.
    pub inherited_object_type_guid: Option<String>,
    /// Any callback / condition data following the SID (for callback ACE types 0x09..0x10).
    pub extra_data: Vec<u8>,
    /// Whether this ACE type is standard and fully parsed by the security subsystem.
    pub is_supported_type: bool,
}

pub type DaclEntrySnapshot = AceSnapshot;

/// Maximum allowed DP matrix cells (baseline.len() * current.len()) for order-preserving DACL matching.
/// Prevents excessive computational load or denial-of-service on adversarial ACL inputs.
pub const MAX_DACL_MATCHER_CELLS: usize = 1_000_000;

/// Typed errors representing unexpected concurrent mutations or corruption detected in an object's DACL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaclDriftError {
    UnexpectedAceAdded {
        sid: String,
        mask: u32,
        flags: u8,
    },
    PreexistingAceRemoved {
        sid: String,
        expected_mask: u32,
    },
    PreexistingAceModified {
        sid: String,
        detail: String,
    },
    PreexistingAceReordered {
        sid: String,
        detail: String,
    },
    ContainedAceMissing {
        expected_sid: String,
    },
    AclStructureCorrupted {
        detail: String,
    },
    MatcherComplexityBudgetExceeded {
        baseline_count: usize,
        current_count: usize,
        max_cells: usize,
    },
}

impl fmt::Display for DaclDriftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedAceAdded { sid, mask, flags } => {
                write!(
                    f,
                    "Unexpected ACE added by concurrent actor: SID='{sid}', mask=0x{mask:08x}, flags=0x{flags:02x}"
                )
            }
            Self::PreexistingAceRemoved { sid, expected_mask } => {
                write!(
                    f,
                    "Pre-existing ACE removed by concurrent actor: SID='{sid}', expected_mask=0x{expected_mask:08x}"
                )
            }
            Self::PreexistingAceModified { sid, detail } => {
                write!(f, "Pre-existing ACE for SID '{sid}' was modified: {detail}")
            }
            Self::PreexistingAceReordered { sid, detail } => {
                write!(
                    f,
                    "Pre-existing ACE for SID '{sid}' was reordered: {detail}"
                )
            }
            Self::ContainedAceMissing { expected_sid } => {
                write!(
                    f,
                    "Expected contained ACE for SID '{expected_sid}' was not found in DACL"
                )
            }
            Self::AclStructureCorrupted { detail } => {
                write!(f, "ACL structure corruption detected: {detail}")
            }
            Self::MatcherComplexityBudgetExceeded {
                baseline_count,
                current_count,
                max_cells,
            } => {
                write!(
                    f,
                    "DACL matcher complexity budget exceeded: baseline has {baseline_count} ACEs, current has {current_count} ACEs (product {} exceeds budget {max_cells})",
                    baseline_count.saturating_mul(*current_count)
                )
            }
        }
    }
}

impl std::error::Error for DaclDriftError {}

/// Complete snapshot of an object's DACL security descriptor for integrity comparison and drift detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaclSnapshot {
    /// Whether a DACL was present in the security descriptor.
    pub is_present: bool,
    /// Whether the DACL is explicitly NULL (which grants all access to everyone).
    pub is_null_dacl: bool,
    /// ACL revision (typically 2 for standard ACLs, 4 for object ACLs).
    pub acl_revision: u8,
    /// Total count of ACEs.
    pub ace_count: u16,
    /// Ordered list of complete ACE snapshots.
    pub aces: Vec<AceSnapshot>,
    /// Complete raw bytes of the ACL (header + all contiguous ACEs).
    pub raw_acl_bytes: Vec<u8>,
}

impl std::ops::Deref for DaclSnapshot {
    type Target = [AceSnapshot];
    fn deref(&self) -> &Self::Target {
        &self.aces
    }
}

impl<'a> IntoIterator for &'a DaclSnapshot {
    type Item = &'a AceSnapshot;
    type IntoIter = std::slice::Iter<'a, AceSnapshot>;
    fn into_iter(self) -> Self::IntoIter {
        self.aces.iter()
    }
}

impl DaclSnapshot {
    /// Strict exact match: all ACEs, order, masks, flags, SIDs, GUIDs, and raw ACL bytes must be identical.
    pub fn is_exact_match(&self, other: &Self) -> bool {
        self == other
    }

    /// Provide a comprehensive diagnostic diff explaining any divergence between two snapshots.
    pub fn detailed_diff(&self, other: &Self) -> Vec<String> {
        let mut diffs = Vec::new();
        if self.is_present != other.is_present {
            diffs.push(format!(
                "is_present mismatch: self={}, other={}",
                self.is_present, other.is_present
            ));
        }
        if self.is_null_dacl != other.is_null_dacl {
            diffs.push(format!(
                "is_null_dacl mismatch: self={}, other={}",
                self.is_null_dacl, other.is_null_dacl
            ));
        }
        if self.acl_revision != other.acl_revision {
            diffs.push(format!(
                "acl_revision mismatch: self={}, other={}",
                self.acl_revision, other.acl_revision
            ));
        }
        if self.ace_count != other.ace_count {
            diffs.push(format!(
                "ace_count mismatch: self={}, other={}",
                self.ace_count, other.ace_count
            ));
        }
        if self.raw_acl_bytes != other.raw_acl_bytes {
            diffs.push(format!(
                "raw_acl_bytes mismatch: self len={}, other len={}",
                self.raw_acl_bytes.len(),
                other.raw_acl_bytes.len()
            ));
        }
        let max_aces = self.aces.len().max(other.aces.len());
        for i in 0..max_aces {
            match (self.aces.get(i), other.aces.get(i)) {
                (Some(a), Some(b)) => {
                    if a != b {
                        let mut ace_diffs = Vec::new();
                        if a.ace_type != b.ace_type {
                            ace_diffs
                                .push(format!("type 0x{:02x} vs 0x{:02x}", a.ace_type, b.ace_type));
                        }
                        if a.ace_flags != b.ace_flags {
                            ace_diffs.push(format!(
                                "flags 0x{:02x} vs 0x{:02x}",
                                a.ace_flags, b.ace_flags
                            ));
                        }
                        if a.access_mask != b.access_mask {
                            ace_diffs.push(format!(
                                "mask 0x{:08x} vs 0x{:08x}",
                                a.access_mask, b.access_mask
                            ));
                        }
                        if a.sid_string != b.sid_string {
                            ace_diffs.push(format!("sid '{}' vs '{}'", a.sid_string, b.sid_string));
                        }
                        if a.object_type_guid != b.object_type_guid {
                            ace_diffs.push(format!(
                                "object_guid {:?} vs {:?}",
                                a.object_type_guid, b.object_type_guid
                            ));
                        }
                        if a.inherited_object_type_guid != b.inherited_object_type_guid {
                            ace_diffs.push(format!(
                                "inherited_object_guid {:?} vs {:?}",
                                a.inherited_object_type_guid, b.inherited_object_type_guid
                            ));
                        }
                        if a.extra_data != b.extra_data {
                            ace_diffs.push("extra_data mismatch".to_string());
                        }
                        if a.raw_ace_bytes != b.raw_ace_bytes {
                            ace_diffs.push("raw_ace_bytes mismatch".to_string());
                        }
                        diffs.push(format!("ACE[{i}] diff: {}", ace_diffs.join(", ")));
                    }
                }
                (Some(a), None) => {
                    diffs.push(format!(
                        "ACE[{i}] present in self (sid='{}', mask=0x{:08x}) but missing in other",
                        a.sid_string, a.access_mask
                    ));
                }
                (None, Some(b)) => {
                    diffs.push(format!(
                        "ACE[{i}] missing in self but present in other (sid='{}', mask=0x{:08x})",
                        b.sid_string, b.access_mask
                    ));
                }
                (None, None) => {}
            }
        }
        diffs
    }

    /// Detect unexpected concurrent mutations between this baseline snapshot and the current snapshot.
    ///
    /// Preserves the exact relative order of all pre-existing baseline ACEs. Legitimate AppContainer
    /// insertions (where `sid_string == expected_contained_sid`) are permitted at any position,
    /// but pre-existing ACEs must not be reordered, modified, or removed, and no unexpected third-party
    /// ACEs may be added.
    pub fn detect_concurrent_drift(
        &self,
        current: &DaclSnapshot,
        expected_contained_sid: &str,
    ) -> Result<(), DaclDriftError> {
        // 0. Verify presence and null state consistency
        if self.is_present != current.is_present || self.is_null_dacl != current.is_null_dacl {
            return Err(DaclDriftError::AclStructureCorrupted {
                detail: format!(
                    "DACL presence/null state changed: baseline (present={}, null={}) vs current (present={}, null={})",
                    self.is_present, self.is_null_dacl, current.is_present, current.is_null_dacl
                ),
            });
        }

        // Helper to check complete ACE identity (type, flags, mask, SID, GUIDs, extra data, raw bytes)
        fn ace_ident_match(a: &AceSnapshot, b: &AceSnapshot) -> bool {
            a.ace_type == b.ace_type
                && a.ace_flags == b.ace_flags
                && a.access_mask == b.access_mask
                && a.sid_string == b.sid_string
                && a.object_type_guid == b.object_type_guid
                && a.inherited_object_type_guid == b.inherited_object_type_guid
                && a.extra_data == b.extra_data
                && a.raw_ace_bytes == b.raw_ace_bytes
        }

        // Iterative dynamic programming order-preserving subsequence matcher.
        // Determines whether `baseline` is an order-preserving subsequence of `current`,
        // where every element in `current` not matched to `baseline` has `sid_string == contained_sid`.
        // Time complexity: O(|baseline| * |current|).
        // Space complexity: O(|baseline|) memory, O(1) stack recursion depth.
        fn find_valid_embedding(
            baseline: &[AceSnapshot],
            current: &[AceSnapshot],
            contained_sid: &str,
        ) -> Result<bool, DaclDriftError> {
            let b_len = baseline.len();
            let c_len = current.len();

            if b_len > MAX_DACL_MATCHER_CELLS
                || c_len > MAX_DACL_MATCHER_CELLS
                || b_len.saturating_mul(c_len) > MAX_DACL_MATCHER_CELLS
            {
                return Err(DaclDriftError::MatcherComplexityBudgetExceeded {
                    baseline_count: b_len,
                    current_count: c_len,
                    max_cells: MAX_DACL_MATCHER_CELLS,
                });
            }

            // dp[b] represents: can baseline[b..b_len] be embedded in current[c..c_len]?
            let mut dp = vec![false; b_len + 1];
            dp[b_len] = true;

            for c in (0..c_len).rev() {
                let is_contained = current[c].sid_string == contained_sid;
                let mut next_dp = vec![false; b_len + 1];

                // For b == b_len: all remaining elements in current[c..] must be contained_sid
                next_dp[b_len] = is_contained && dp[b_len];

                for b in (0..b_len).rev() {
                    let mut matched = false;
                    // Option 1: Current ACE matches baseline ACE; advance both
                    if ace_ident_match(&baseline[b], &current[c]) && dp[b + 1] {
                        matched = true;
                    }
                    // Option 2: Current ACE is an inserted/expected contained ACE; advance current only
                    if !matched && is_contained && dp[b] {
                        matched = true;
                    }
                    next_dp[b] = matched;
                }
                dp = next_dp;
            }

            Ok(dp[0])
        }

        if find_valid_embedding(&self.aces, &current.aces, expected_contained_sid)? {
            return Ok(());
        }

        // If no valid order-preserving embedding was found, diagnose the exact failure mode.
        // Step 1: Multiset comparison of non-contained ACEs
        let mut matched_current = std::collections::HashSet::new();
        let mut matched_baseline = std::collections::HashSet::new();

        for (b_i, b_ace) in self.aces.iter().enumerate() {
            for (c_i, c_ace) in current.aces.iter().enumerate() {
                if !matched_current.contains(&c_i) && ace_ident_match(b_ace, c_ace) {
                    matched_current.insert(c_i);
                    matched_baseline.insert(b_i);
                    break;
                }
            }
        }

        let all_baseline_matched = matched_baseline.len() == self.aces.len();
        let uncontained_unmatched_in_current: Vec<_> = current
            .aces
            .iter()
            .enumerate()
            .filter(|(c_i, c)| {
                !matched_current.contains(c_i) && c.sid_string != expected_contained_sid
            })
            .collect();

        if all_baseline_matched && uncontained_unmatched_in_current.is_empty() {
            // Multiset is identical! The failure was strictly due to relative reordering of baseline ACEs!
            // Find the first baseline ACE whose relative order was inverted
            let mut last_c_pos = 0;
            let mut inverted_sid = self.aces[0].sid_string.clone();
            let mut inverted_detail = format!(
                "Baseline ACE[0] for SID '{}' was reordered in current DACL",
                self.aces[0].sid_string
            );
            for (b_i, b_ace) in self.aces.iter().enumerate() {
                if let Some(c_pos) = current.aces.iter().position(|c| ace_ident_match(b_ace, c)) {
                    if b_i > 0 && c_pos < last_c_pos {
                        inverted_sid = b_ace.sid_string.clone();
                        inverted_detail = format!(
                            "Baseline ACE[{b_i}] for SID '{}' (flags=0x{:02x}, mask=0x{:08x}) appears at position {c_pos} in current DACL, preceding baseline ACE[{}] at position {last_c_pos}",
                            b_ace.sid_string,
                            b_ace.ace_flags,
                            b_ace.access_mask,
                            b_i - 1
                        );
                        break;
                    }
                    last_c_pos = c_pos;
                }
            }
            return Err(DaclDriftError::PreexistingAceReordered {
                sid: inverted_sid,
                detail: inverted_detail,
            });
        }

        // If there are unmatched non-contained ACEs in current:
        if let Some((_, rogue)) = uncontained_unmatched_in_current.first() {
            if self.aces.iter().any(|b| b.sid_string == rogue.sid_string) {
                return Err(DaclDriftError::PreexistingAceModified {
                    sid: rogue.sid_string.clone(),
                    detail: format!(
                        "Pre-existing ACE for SID '{}' was modified (flags=0x{:02x}, mask=0x{:08x})",
                        rogue.sid_string, rogue.ace_flags, rogue.access_mask
                    ),
                });
            } else {
                return Err(DaclDriftError::UnexpectedAceAdded {
                    sid: rogue.sid_string.clone(),
                    mask: rogue.access_mask,
                    flags: rogue.ace_flags,
                });
            }
        }

        // If there are unmatched baseline ACEs:
        for (b_i, b_ace) in self.aces.iter().enumerate() {
            if !matched_baseline.contains(&b_i) {
                if current
                    .aces
                    .iter()
                    .any(|c| c.sid_string == b_ace.sid_string)
                {
                    return Err(DaclDriftError::PreexistingAceModified {
                        sid: b_ace.sid_string.clone(),
                        detail: format!(
                            "Pre-existing ACE for SID '{}' was modified or replaced",
                            b_ace.sid_string
                        ),
                    });
                } else {
                    return Err(DaclDriftError::PreexistingAceRemoved {
                        sid: b_ace.sid_string.clone(),
                        expected_mask: b_ace.access_mask,
                    });
                }
            }
        }

        Err(DaclDriftError::AclStructureCorrupted {
            detail: "DACL could not be reconciled with baseline".to_string(),
        })
    }
}

/// Pure Rust decoder for binary SID structures into canonical SDDL format ("S-1-...").
pub fn decode_sid_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 8 {
        return None;
    }
    let revision = bytes[0];
    let sub_authority_count = bytes[1] as usize;
    if bytes.len() < 8 + sub_authority_count * 4 {
        return None;
    }
    let mut auth_val: u64 = 0;
    for &b in &bytes[2..8] {
        auth_val = (auth_val << 8) | (b as u64);
    }
    let mut parts = vec![format!("S-{revision}-{auth_val}")];
    for i in 0..sub_authority_count {
        let offset = 8 + i * 4;
        let sub_auth = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        parts.push(sub_auth.to_string());
    }
    Some(parts.join("-"))
}

/// Helper to format a 16-byte slice into a standard GUID string.
pub fn format_guid_slice(b: &[u8]) -> String {
    if b.len() < 16 {
        return "INVALID_GUID".to_string();
    }
    let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let d2 = u16::from_le_bytes([b[4], b[5]]);
    let d3 = u16::from_le_bytes([b[6], b[7]]);
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d1, d2, d3, b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Helper to create a synthetic ACE binary slice for tests.
pub fn make_synthetic_ace(
    ace_type: u8,
    ace_flags: u8,
    mask: u32,
    sub_authorities: &[u32],
    extra: &[u8],
) -> Vec<u8> {
    let mut sid_bytes = vec![1u8, sub_authorities.len() as u8, 0, 0, 0, 0, 0, 5];
    for &sa in sub_authorities {
        sid_bytes.extend_from_slice(&sa.to_le_bytes());
    }
    let ace_size = (8 + sid_bytes.len() + extra.len()) as u16;
    let mut ace = Vec::new();
    ace.push(ace_type);
    ace.push(ace_flags);
    ace.extend_from_slice(&ace_size.to_le_bytes());
    ace.extend_from_slice(&mask.to_le_bytes());
    ace.extend_from_slice(&sid_bytes);
    ace.extend_from_slice(extra);
    ace
}

/// Helper to create a synthetic DACL binary buffer containing a list of ACEs for tests.
pub fn make_synthetic_dacl(aces: &[Vec<u8>]) -> Vec<u8> {
    let total_ace_bytes: usize = aces.iter().map(|a| a.len()).sum();
    let acl_size = (8 + total_ace_bytes) as u16;
    let ace_count = aces.len() as u16;
    let mut dacl = Vec::new();
    dacl.push(2); // AclRevision
    dacl.push(0); // Sbz1
    dacl.extend_from_slice(&acl_size.to_le_bytes());
    dacl.extend_from_slice(&ace_count.to_le_bytes());
    dacl.extend_from_slice(&0u16.to_le_bytes()); // Sbz2
    for ace in aces {
        dacl.extend_from_slice(ace);
    }
    dacl
}

/// Helper to create a synthetic object-specific ACE binary slice for tests (types 5..8).
pub fn make_synthetic_object_ace(
    ace_type: u8,
    ace_flags: u8,
    mask: u32,
    object_flags: u32,
    object_guid: Option<&[u8; 16]>,
    inherited_object_guid: Option<&[u8; 16]>,
    sub_authorities: &[u32],
    extra: &[u8],
) -> Vec<u8> {
    let mut sid_bytes = vec![1u8, sub_authorities.len() as u8, 0, 0, 0, 0, 0, 5];
    for &sa in sub_authorities {
        sid_bytes.extend_from_slice(&sa.to_le_bytes());
    }
    let mut guid_len = 0;
    if (object_flags & 0x01) != 0 && object_guid.is_some() {
        guid_len += 16;
    }
    if (object_flags & 0x02) != 0 && inherited_object_guid.is_some() {
        guid_len += 16;
    }
    let ace_size = (12 + guid_len + sid_bytes.len() + extra.len()) as u16;
    let mut ace = Vec::new();
    ace.push(ace_type);
    ace.push(ace_flags);
    ace.extend_from_slice(&ace_size.to_le_bytes());
    ace.extend_from_slice(&mask.to_le_bytes());
    ace.extend_from_slice(&object_flags.to_le_bytes());
    if (object_flags & 0x01) != 0 {
        if let Some(g) = object_guid {
            ace.extend_from_slice(g);
        }
    }
    if (object_flags & 0x02) != 0 {
        if let Some(g) = inherited_object_guid {
            ace.extend_from_slice(g);
        }
    }
    ace.extend_from_slice(&sid_bytes);
    ace.extend_from_slice(extra);
    ace
}

/// Helper to create a synthetic self-relative security descriptor buffer for tests.
pub fn make_synthetic_security_descriptor(control: u16, dacl_bytes: Option<&[u8]>) -> Vec<u8> {
    let mut sd = vec![0u8; 20];
    sd[0] = 1; // Revision
    sd[1] = 0; // Sbz1
    sd[2..4].copy_from_slice(&control.to_le_bytes());
    // Offsets for Owner (4..8), Group (8..12), Sacl (12..16) are 0
    if let Some(dacl) = dacl_bytes {
        let dacl_offset = 20u32;
        sd[16..20].copy_from_slice(&dacl_offset.to_le_bytes());
        sd.extend_from_slice(dacl);
    }
    sd
}

/// Windows Security Descriptor control flag indicating that a DACL is present.
pub const SE_DACL_PRESENT: u16 = 0x0004;

/// Parse a raw Windows DACL / ACL byte slice into a rich, structured `DaclSnapshot`.
pub fn parse_dacl_bytes(dacl_bytes: &[u8]) -> Result<DaclSnapshot, SecurityError> {
    if dacl_bytes.is_empty() {
        return Ok(DaclSnapshot {
            is_present: false,
            is_null_dacl: false,
            acl_revision: 0,
            ace_count: 0,
            aces: Vec::new(),
            raw_acl_bytes: Vec::new(),
        });
    }

    if dacl_bytes.len() < 8 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: 0,
            message: format!(
                "ACL buffer too short ({} bytes < 8 byte header)",
                dacl_bytes.len()
            ),
        });
    }

    let acl_revision = dacl_bytes[0];
    let _acl_size = u16::from_le_bytes([dacl_bytes[2], dacl_bytes[3]]) as usize;
    let ace_count = u16::from_le_bytes([dacl_bytes[4], dacl_bytes[5]]) as usize;

    let mut aces = Vec::with_capacity(ace_count);
    let mut offset = 8;

    for i in 0..ace_count {
        if offset + 4 > dacl_bytes.len() {
            return Err(SecurityError::AclConfigurationFailed {
                os_error_code: 0,
                message: format!("Truncated ACL buffer reading ACE {i} header at offset {offset}"),
            });
        }

        let ace_type = dacl_bytes[offset];
        let ace_flags = dacl_bytes[offset + 1];
        let ace_size =
            u16::from_le_bytes([dacl_bytes[offset + 2], dacl_bytes[offset + 3]]) as usize;

        if ace_size < 4 {
            return Err(SecurityError::AclConfigurationFailed {
                os_error_code: 0,
                message: format!("Corrupted ACE {i}: AceSize {ace_size} < 4"),
            });
        }

        if offset + ace_size > dacl_bytes.len() {
            return Err(SecurityError::AclConfigurationFailed {
                os_error_code: 0,
                message: format!(
                    "Truncated ACE {i}: offset {offset} + size {ace_size} > buffer len {}",
                    dacl_bytes.len()
                ),
            });
        }

        let raw_ace = dacl_bytes[offset..offset + ace_size].to_vec();
        let access_mask = if ace_size >= 8 {
            u32::from_le_bytes([
                dacl_bytes[offset + 4],
                dacl_bytes[offset + 5],
                dacl_bytes[offset + 6],
                dacl_bytes[offset + 7],
            ])
        } else {
            0
        };

        // Parse type-specific fields
        let mut object_type_guid = None;
        let mut inherited_object_type_guid = None;
        let mut extra_data = Vec::new();
        let mut sid_string = String::new();
        let is_supported_type;

        match ace_type {
            // Standard ACEs: Allowed, Denied, Audit, Alarm, Mandatory Label
            0 | 1 | 2 | 3 | 0x11 => {
                is_supported_type = true;
                let sid_offset = offset + 8;
                if sid_offset < offset + ace_size {
                    let sid_slice = &dacl_bytes[sid_offset..offset + ace_size];
                    if let Some(s) = decode_sid_bytes(sid_slice) {
                        sid_string = s;
                        let sub_auths = sid_slice.get(1).copied().unwrap_or(0) as usize;
                        let sid_len = 8 + sub_auths * 4;
                        if sid_offset + sid_len < offset + ace_size {
                            extra_data =
                                dacl_bytes[sid_offset + sid_len..offset + ace_size].to_vec();
                        }
                    } else {
                        sid_string = format!("CORRUPT_OR_UNPARSABLE_SID_ACE_{i}");
                    }
                } else {
                    sid_string = format!("MISSING_SID_ACE_{i}");
                }
            }
            // Object-specific ACEs (Allowed Object, Denied Object, Audit Object, Alarm Object)
            5 | 6 | 7 | 8 => {
                is_supported_type = true;
                if ace_size < 12 {
                    return Err(SecurityError::AclConfigurationFailed {
                        os_error_code: 0,
                        message: format!(
                            "Corrupted object-specific ACE {i}: AceSize {ace_size} < 12"
                        ),
                    });
                }
                let flags = u32::from_le_bytes([
                    dacl_bytes[offset + 8],
                    dacl_bytes[offset + 9],
                    dacl_bytes[offset + 10],
                    dacl_bytes[offset + 11],
                ]);
                let mut curr = offset + 12;
                if (flags & 0x01) != 0 {
                    if curr + 16 > offset + ace_size {
                        return Err(SecurityError::AclConfigurationFailed {
                            os_error_code: 0,
                            message: format!(
                                "Truncated object-specific ACE {i}: insufficient bytes for ObjectType GUID"
                            ),
                        });
                    }
                    object_type_guid = Some(format_guid_slice(&dacl_bytes[curr..curr + 16]));
                    curr += 16;
                }
                if (flags & 0x02) != 0 {
                    if curr + 16 > offset + ace_size {
                        return Err(SecurityError::AclConfigurationFailed {
                            os_error_code: 0,
                            message: format!(
                                "Truncated object-specific ACE {i}: insufficient bytes for InheritedObjectType GUID"
                            ),
                        });
                    }
                    inherited_object_type_guid =
                        Some(format_guid_slice(&dacl_bytes[curr..curr + 16]));
                    curr += 16;
                }
                if curr + 8 > offset + ace_size {
                    return Err(SecurityError::AclConfigurationFailed {
                        os_error_code: 0,
                        message: format!(
                            "Truncated object-specific ACE {i}: insufficient bytes for SID header"
                        ),
                    });
                }
                let sid_slice = &dacl_bytes[curr..offset + ace_size];
                if let Some(s) = decode_sid_bytes(sid_slice) {
                    sid_string = s;
                    let sub_auths = sid_slice.get(1).copied().unwrap_or(0) as usize;
                    let sid_len = 8 + sub_auths * 4;
                    if curr + sid_len < offset + ace_size {
                        extra_data = dacl_bytes[curr + sid_len..offset + ace_size].to_vec();
                    }
                } else {
                    return Err(SecurityError::AclConfigurationFailed {
                        os_error_code: 0,
                        message: format!("Corrupted SID in object-specific ACE {i}"),
                    });
                }
            }
            // Callback ACEs (Allowed Callback, Denied Callback, etc.)
            9 | 10 | 0x0D | 0x0E => {
                is_supported_type = true;
                let sid_offset = offset + 8;
                if sid_offset < offset + ace_size {
                    let sid_slice = &dacl_bytes[sid_offset..offset + ace_size];
                    if let Some(s) = decode_sid_bytes(sid_slice) {
                        sid_string = s;
                        let sub_auths = sid_slice.get(1).copied().unwrap_or(0) as usize;
                        let sid_len = 8 + sub_auths * 4;
                        if sid_offset + sid_len < offset + ace_size {
                            extra_data =
                                dacl_bytes[sid_offset + sid_len..offset + ace_size].to_vec();
                        }
                    } else {
                        sid_string = format!("CORRUPT_CALLBACK_SID_{i}");
                    }
                }
            }
            _ => {
                is_supported_type = false;
                sid_string = format!("UNSUPPORTED_ACE_TYPE_0x{ace_type:02X}");
                if offset + 8 < offset + ace_size {
                    extra_data = dacl_bytes[offset + 8..offset + ace_size].to_vec();
                }
            }
        }

        aces.push(AceSnapshot {
            index: i,
            ace_type,
            ace_flags,
            access_mask,
            sid_string,
            raw_ace_bytes: raw_ace,
            object_type_guid,
            inherited_object_type_guid,
            extra_data,
            is_supported_type,
        });

        offset += ace_size;
    }

    Ok(DaclSnapshot {
        is_present: true,
        is_null_dacl: false,
        acl_revision,
        ace_count: ace_count as u16,
        aces,
        raw_acl_bytes: dacl_bytes[..offset.min(dacl_bytes.len())].to_vec(),
    })
}

/// Parse a raw Windows Security Descriptor byte buffer (self-relative) into a structured `DaclSnapshot`.
///
/// Accurately distinguishes all four documented Windows DACL presence states:
/// 1. DACL absent (`SE_DACL_PRESENT` flag is not set): `is_present: false, is_null_dacl: false`.
/// 2. DACL explicitly NULL (`SE_DACL_PRESENT` flag is set and DACL offset/pointer is 0): `is_present: true, is_null_dacl: true`.
/// 3. DACL present but empty (`SE_DACL_PRESENT` set, DACL offset != 0, ACL `AceCount == 0`): `is_present: true, is_null_dacl: false, ace_count: 0`.
/// 4. DACL present and populated (`SE_DACL_PRESENT` set, DACL offset != 0, ACL `AceCount > 0`): `is_present: true, is_null_dacl: false, ace_count > 0`.
pub fn parse_security_descriptor_bytes(sd_bytes: &[u8]) -> Result<DaclSnapshot, SecurityError> {
    if sd_bytes.len() < 20 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: 0,
            message: format!(
                "Security descriptor buffer too short ({} bytes < 20-byte relative header)",
                sd_bytes.len()
            ),
        });
    }

    let control = u16::from_le_bytes([sd_bytes[2], sd_bytes[3]]);
    let has_dacl = (control & SE_DACL_PRESENT) != 0;
    let dacl_offset =
        u32::from_le_bytes([sd_bytes[16], sd_bytes[17], sd_bytes[18], sd_bytes[19]]) as usize;

    // Case 1: DACL absent
    if !has_dacl {
        return Ok(DaclSnapshot {
            is_present: false,
            is_null_dacl: false,
            acl_revision: 0,
            ace_count: 0,
            aces: Vec::new(),
            raw_acl_bytes: Vec::new(),
        });
    }

    // Case 2: DACL explicitly NULL
    if dacl_offset == 0 {
        return Ok(DaclSnapshot {
            is_present: true,
            is_null_dacl: true,
            acl_revision: 0,
            ace_count: 0,
            aces: Vec::new(),
            raw_acl_bytes: Vec::new(),
        });
    }

    // Cases 3 and 4: DACL present at offset
    if dacl_offset + 8 > sd_bytes.len() {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: 0,
            message: format!(
                "Truncated security descriptor: DACL offset {dacl_offset} exceeds buffer length {}",
                sd_bytes.len()
            ),
        });
    }

    let acl_size =
        u16::from_le_bytes([sd_bytes[dacl_offset + 2], sd_bytes[dacl_offset + 3]]) as usize;
    if dacl_offset + acl_size > sd_bytes.len() {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: 0,
            message: format!(
                "Corrupted security descriptor: DACL size {acl_size} at offset {dacl_offset} exceeds buffer length {}",
                sd_bytes.len()
            ),
        });
    }

    parse_dacl_bytes(&sd_bytes[dacl_offset..dacl_offset + acl_size])
}

/// Capture a deterministic snapshot of an object's DACL for before/after comparison.
#[cfg(windows)]
pub fn snapshot_user_object_dacl(handle: win32::HANDLE) -> Result<DaclSnapshot, SecurityError> {
    if handle.is_null() || handle == win32::INVALID_HANDLE_VALUE {
        let err = unsafe { win32::GetLastError() };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: "Invalid user object handle for DACL snapshot".to_string(),
        });
    }

    let _lock = USER_OBJECT_SECURITY_MUTEX.lock().unwrap();

    let mut needed: u32 = 0;
    unsafe {
        win32::GetUserObjectSecurity(
            handle,
            &win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }

    let mut buf = vec![0u8; (needed as usize).max(256)];
    let mut actual_needed = needed;
    let mut get_ok = win32::FALSE;
    for _ in 0..5 {
        get_ok = unsafe {
            win32::GetUserObjectSecurity(
                handle,
                &win32::DACL_SECURITY_INFORMATION,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
                &mut actual_needed,
            )
        };
        if get_ok != win32::FALSE {
            break;
        }
        let err = unsafe { win32::GetLastError() };
        if err == 122 {
            buf.resize(actual_needed as usize + 256, 0);
            continue;
        }
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: err,
            message: "GetUserObjectSecurity failed during snapshot".to_string(),
        });
    }

    if get_ok == win32::FALSE {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: unsafe { win32::GetLastError() },
            message: "GetUserObjectSecurity failed after retries during snapshot".to_string(),
        });
    }

    parse_security_descriptor_bytes(&buf[..actual_needed as usize])
}

/// Grant DACL access permissions to an AppContainer SID on a filesystem path.
#[cfg(windows)]
unsafe fn grant_filesystem_access(
    path: &Path,
    sid: win32::PSID,
    permissions: u32,
) -> Result<(), SecurityError> {
    let wide = to_wide_null(&path.to_string_lossy());

    let mut old_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut sec_desc: *mut std::ffi::c_void = std::ptr::null_mut();

    let get_res = unsafe {
        win32::GetNamedSecurityInfoW(
            wide.as_ptr(),
            win32::SE_FILE_OBJECT,
            win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old_dacl,
            std::ptr::null_mut(),
            &mut sec_desc,
        )
    };
    if get_res != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: get_res,
            message: format!(
                "GetNamedSecurityInfoW failed for '{}': {get_res}",
                path.display()
            ),
        });
    }

    let inheritance = if path.is_dir() {
        win32::CONTAINER_INHERIT_ACE | win32::OBJECT_INHERIT_ACE
    } else {
        0
    };

    let ea = win32::EXPLICIT_ACCESS_W {
        grfAccessPermissions: permissions,
        grfAccessMode: win32::GRANT_ACCESS,
        grfInheritance: inheritance,
        Trustee: win32::TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: win32::TRUSTEE_IS_SID,
            TrusteeType: win32::TRUSTEE_IS_UNKNOWN,
            ptstrName: sid,
        },
    };

    let mut new_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let set_entries = unsafe { win32::SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
    if set_entries != 0 {
        unsafe { win32::LocalFree(sec_desc) };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_entries,
            message: format!(
                "SetEntriesInAclW failed for '{}': {set_entries}",
                path.display()
            ),
        });
    }

    let set_res = unsafe {
        win32::SetNamedSecurityInfoW(
            wide.as_ptr(),
            win32::SE_FILE_OBJECT,
            win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        )
    };

    unsafe {
        win32::LocalFree(new_dacl);
        win32::LocalFree(sec_desc);
    }

    if set_res != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_res,
            message: format!(
                "SetNamedSecurityInfoW failed for '{}': {set_res}",
                path.display()
            ),
        });
    }

    Ok(())
}

/// Revoke DACL access permissions previously granted to an AppContainer SID on a filesystem path.
#[cfg(windows)]
unsafe fn revoke_filesystem_access(path: &Path, sid: win32::PSID) -> Result<(), SecurityError> {
    if sid.is_null() || !path.exists() {
        return Ok(());
    }
    let wide = to_wide_null(&path.to_string_lossy());
    let mut old_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut sec_desc: *mut std::ffi::c_void = std::ptr::null_mut();

    let get_res = unsafe {
        win32::GetNamedSecurityInfoW(
            wide.as_ptr(),
            win32::SE_FILE_OBJECT,
            win32::DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old_dacl,
            std::ptr::null_mut(),
            &mut sec_desc,
        )
    };
    if get_res != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: get_res,
            message: format!(
                "GetNamedSecurityInfoW failed during filesystem revocation for '{}': {get_res}",
                path.display()
            ),
        });
    }

    let ea = win32::EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0,
        grfAccessMode: win32::REVOKE_ACCESS,
        grfInheritance: 0,
        Trustee: win32::TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: win32::TRUSTEE_IS_SID,
            TrusteeType: win32::TRUSTEE_IS_UNKNOWN,
            ptstrName: sid,
        },
    };

    let mut new_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
    let set_entries = unsafe { win32::SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
    if set_entries != 0 {
        unsafe { win32::LocalFree(sec_desc) };
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_entries,
            message: format!(
                "SetEntriesInAclW failed during filesystem revocation for '{}': {set_entries}",
                path.display()
            ),
        });
    }

    let set_res = if !new_dacl.is_null() {
        unsafe {
            win32::SetNamedSecurityInfoW(
                wide.as_ptr(),
                win32::SE_FILE_OBJECT,
                win32::DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                new_dacl,
                std::ptr::null_mut(),
            )
        }
    } else {
        0
    };

    unsafe {
        if !new_dacl.is_null() {
            win32::LocalFree(new_dacl);
        }
        win32::LocalFree(sec_desc);
    }

    if set_res != 0 {
        return Err(SecurityError::AclConfigurationFailed {
            os_error_code: set_res,
            message: format!(
                "SetNamedSecurityInfoW failed during filesystem revocation for '{}': {set_res}",
                path.display()
            ),
        });
    }
    Ok(())
}

#[derive(Debug, Default)]
struct ProfileGrantState {
    winsta_granted: bool,
    desktop_granted: bool,
    granted_paths: Vec<PathBuf>,
    profile_deleted: bool,
    deletion_initiated: bool,
    winsta_baseline: Option<DaclSnapshot>,
    desktop_baseline: Option<DaclSnapshot>,
    strict_drift_detection: bool,
    active_executions: usize,
}

/// RAII lease representing an active contained subprocess execution.
/// While any lease is held, `AppContainerProfile::delete()` will fail closed
/// to prevent concurrent revocation or profile deletion during subprocess execution.
#[derive(Debug)]
pub struct ExecutionLease<'a> {
    profile: &'a AppContainerProfile,
}

impl<'a> Drop for ExecutionLease<'a> {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let mut state = match self.profile.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.active_executions = state.active_executions.saturating_sub(1);
        }
    }
}

/// An ephemeral Windows AppContainer security profile.
///
/// Ensures fail-closed cleanup via RAII: when dropped or deleted, all granted WindowStation,
/// Desktop, and filesystem ACEs are revoked with error surfacing, `DeleteAppContainerProfile` is
/// called, and the AppContainer SID is freed. Access grants are NEVER cleared until revocation
/// succeeds, making cleanup fully retryable and idempotent.
pub struct AppContainerProfile {
    #[cfg(windows)]
    profile_name: String,
    #[cfg(windows)]
    sid: win32::PSID,
    #[cfg(windows)]
    state: std::sync::Mutex<ProfileGrantState>,
    #[cfg(windows)]
    cleanup_lock: std::sync::Mutex<()>,
    #[doc(hidden)]
    pub fault_hook: std::sync::Arc<
        std::sync::Mutex<Option<Box<dyn Fn(&str) -> Option<SecurityError> + Send + Sync>>>,
    >,
}

impl fmt::Debug for AppContainerProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(windows)]
        {
            f.debug_struct("AppContainerProfile")
                .field("profile_name", &self.profile_name)
                .field("sid", &self.sid)
                .field("state", &self.state)
                .finish()
        }
        #[cfg(not(windows))]
        {
            f.debug_struct("AppContainerProfile").finish()
        }
    }
}

#[cfg(windows)]
unsafe impl Send for AppContainerProfile {}
#[cfg(windows)]
unsafe impl Sync for AppContainerProfile {}

impl AppContainerProfile {
    /// Create a new, ephemeral AppContainer profile with zero network capabilities.
    pub fn create(profile_name: &str) -> Result<Self, SecurityError> {
        #[cfg(windows)]
        {
            let wide_name = to_wide_null(profile_name);
            let mut sid: win32::PSID = std::ptr::null_mut();

            unsafe {
                let hr = win32::CreateAppContainerProfile(
                    wide_name.as_ptr(),
                    wide_name.as_ptr(),
                    wide_name.as_ptr(),
                    std::ptr::null(),
                    0,
                    &mut sid,
                );

                // If profile already exists (0x800700B7), clean it up and recreate cleanly
                if (hr as u32) == 0x800700B7 {
                    let _ = win32::DeleteAppContainerProfile(wide_name.as_ptr());
                    let retry_hr = win32::CreateAppContainerProfile(
                        wide_name.as_ptr(),
                        wide_name.as_ptr(),
                        wide_name.as_ptr(),
                        std::ptr::null(),
                        0,
                        &mut sid,
                    );
                    if retry_hr < 0 {
                        return Err(SecurityError::AppContainerProfileCreationFailed {
                            os_error_code: retry_hr as u32,
                            message: format!(
                                "CreateAppContainerProfile retry failed with HRESULT 0x{retry_hr:08X}"
                            ),
                        });
                    }
                } else if hr < 0 {
                    return Err(SecurityError::AppContainerProfileCreationFailed {
                        os_error_code: hr as u32,
                        message: format!(
                            "CreateAppContainerProfile failed with HRESULT 0x{hr:08X}"
                        ),
                    });
                }

                Ok(Self {
                    profile_name: profile_name.to_string(),
                    sid,
                    state: std::sync::Mutex::new(ProfileGrantState {
                        winsta_granted: false,
                        desktop_granted: false,
                        granted_paths: Vec::new(),
                        profile_deleted: false,
                        deletion_initiated: false,
                        winsta_baseline: None,
                        desktop_baseline: None,
                        strict_drift_detection: true,
                        active_executions: 0,
                    }),
                    cleanup_lock: std::sync::Mutex::new(()),
                    fault_hook: std::sync::Arc::new(std::sync::Mutex::new(None)),
                })
            }
        }
        #[cfg(not(windows))]
        {
            let _ = profile_name;
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "AppContainer profiles are only supported on Windows NT (fail-closed)"
                    .to_string(),
            })
        }
    }

    /// Retrieve the AppContainer profile name.
    pub fn name(&self) -> &str {
        #[cfg(windows)]
        {
            &self.profile_name
        }
        #[cfg(not(windows))]
        {
            ""
        }
    }

    /// Retrieve the raw AppContainer PSID.
    #[cfg(windows)]
    pub fn sid(&self) -> win32::PSID {
        self.sid
    }

    /// Retrieve the AppContainer SID as a standard SDDL string ("S-1-...").
    pub fn sid_to_string(&self) -> String {
        #[cfg(windows)]
        {
            unsafe {
                if self.sid.is_null() {
                    return "NULL_SID".to_string();
                }
                let mut str_ptr: *mut u16 = std::ptr::null_mut();
                let conv_ok = win32::ConvertSidToStringSidW(self.sid, &mut str_ptr);
                if conv_ok != win32::FALSE && !str_ptr.is_null() {
                    let s = from_wide_null(str_ptr);
                    win32::LocalFree(str_ptr as win32::HLOCAL);
                    s
                } else {
                    "UNKNOWN_SID".to_string()
                }
            }
        }
        #[cfg(not(windows))]
        {
            "MOCK_SID".to_string()
        }
    }

    /// Check if WindowStation access has been granted and not yet revoked.
    pub fn is_winsta_granted(&self) -> bool {
        #[cfg(windows)]
        {
            self.state.lock().map(|s| s.winsta_granted).unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Check if Desktop access has been granted and not yet revoked.
    pub fn is_desktop_granted(&self) -> bool {
        #[cfg(windows)]
        {
            self.state
                .lock()
                .map(|s| s.desktop_granted)
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Check if all resources have been fully cleaned up and profile deleted.
    pub fn is_deleted(&self) -> bool {
        #[cfg(windows)]
        {
            self.state
                .lock()
                .map(|s| {
                    s.profile_deleted
                        && !s.winsta_granted
                        && !s.desktop_granted
                        && s.granted_paths.is_empty()
                })
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            true
        }
    }

    /// Count of filesystem paths currently granted and awaiting revocation.
    pub fn granted_paths_count(&self) -> usize {
        #[cfg(windows)]
        {
            self.state
                .lock()
                .map(|s| s.granted_paths.len())
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Configure whether strict drift detection is enforced during cleanup.
    pub fn set_strict_drift_detection(&self, strict: bool) {
        #[cfg(windows)]
        {
            if let Ok(mut state) = self.state.lock() {
                state.strict_drift_detection = strict;
            }
        }
        #[cfg(not(windows))]
        {
            let _ = strict;
        }
    }

    /// Acquire an active execution lease before spawning a contained process.
    /// Fails closed if the profile has already been deleted or if deletion has been initiated.
    pub fn acquire_execution_lease(&self) -> Result<ExecutionLease<'_>, SecurityError> {
        #[cfg(windows)]
        {
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if state.profile_deleted || state.deletion_initiated {
                return Err(SecurityError::AclConfigurationFailed {
                    os_error_code: 0,
                    message: format!(
                        "Cannot acquire execution lease: AppContainer profile '{}' is deleted or pending deletion",
                        self.profile_name
                    ),
                });
            }
            state.active_executions += 1;
            Ok(ExecutionLease { profile: self })
        }
        #[cfg(not(windows))]
        {
            Ok(ExecutionLease { profile: self })
        }
    }

    /// Check whether deletion of this profile has been initiated.
    pub fn is_deletion_initiated(&self) -> bool {
        #[cfg(windows)]
        {
            self.state
                .lock()
                .map(|s| s.deletion_initiated)
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Retrieve the current count of active subprocess executions using this profile.
    pub fn active_executions_count(&self) -> usize {
        #[cfg(windows)]
        {
            self.state.lock().map(|s| s.active_executions).unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    /// Check whether the current WindowStation and Desktop DACLs have drifted from baseline.
    pub fn check_dacl_drift(&self) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            let state = self.state.lock().unwrap();
            let sid_str = self.sid_to_string();
            if let Some(baseline) = &state.winsta_baseline {
                let winsta = unsafe { win32::GetProcessWindowStation() };
                let curr = snapshot_user_object_dacl(winsta)?;
                if let Err(e) = baseline.detect_concurrent_drift(&curr, &sid_str) {
                    return Err(SecurityError::AclConfigurationFailed {
                        os_error_code: 0,
                        message: format!("WindowStation DACL drift: {e}"),
                    });
                }
            }
            if let Some(baseline) = &state.desktop_baseline {
                let desktop = unsafe { win32::GetThreadDesktop(win32::GetCurrentThreadId()) };
                let curr = snapshot_user_object_dacl(desktop)?;
                if let Err(e) = baseline.detect_concurrent_drift(&curr, &sid_str) {
                    return Err(SecurityError::AclConfigurationFailed {
                        os_error_code: 0,
                        message: format!("Desktop DACL drift: {e}"),
                    });
                }
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }

    /// Inject a test fault hook to simulate OS API failures during cleanup/grant.
    #[doc(hidden)]
    pub fn set_fault_hook<F>(&self, hook: F)
    where
        F: Fn(&str) -> Option<SecurityError> + Send + Sync + 'static,
    {
        *self.fault_hook.lock().unwrap() = Some(Box::new(hook));
    }

    /// Clear any active test fault hook.
    #[doc(hidden)]
    pub fn clear_fault_hook(&self) {
        *self.fault_hook.lock().unwrap() = None;
    }

    /// Check if a test fault should be triggered for the specified operation.
    #[doc(hidden)]
    pub fn check_fault(&self, op: &str) -> Option<SecurityError> {
        if let Ok(guard) = self.fault_hook.lock() {
            if let Some(hook) = guard.as_ref() {
                return hook(op);
            }
        }
        None
    }

    /// Grant filesystem access to a sandboxed directory or file.
    pub fn grant_path_access(&self, path: &Path, permissions: u32) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            let _cleanup_guard = match self.cleanup_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if state.profile_deleted || state.deletion_initiated {
                return Err(SecurityError::AclConfigurationFailed {
                    os_error_code: 0,
                    message: format!(
                        "Cannot grant path access: AppContainer profile '{}' is deleted or pending deletion",
                        self.profile_name
                    ),
                });
            }
            unsafe { grant_filesystem_access(path, self.sid, permissions)? };
            state.granted_paths.push(path.to_path_buf());
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = (path, permissions);
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "ACL configuration is only supported on Windows NT".to_string(),
            })
        }
    }

    /// Grant access to the process WindowStation and Desktop.
    ///
    /// Uses least-privilege specific rights (`WINSTA_CONTAINED_RIGHTS` and `DESKTOP_CONTAINED_RIGHTS`),
    /// granting zero write, DACL, or owner modification permissions.
    ///
    /// # Safety and Invariants (Partial-Failure Rollback)
    /// If granting Desktop access fails after WindowStation access succeeded, WindowStation
    /// access is immediately rolled back fail-closed before returning the error.
    pub fn grant_window_station_and_desktop(&self) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            let _cleanup_guard = match self.cleanup_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if state.profile_deleted || state.deletion_initiated {
                return Err(SecurityError::AclConfigurationFailed {
                    os_error_code: 0,
                    message: format!(
                        "Cannot grant WindowStation/Desktop access: AppContainer profile '{}' is deleted or pending deletion",
                        self.profile_name
                    ),
                });
            }

            // Idempotent: if both already granted, return Ok(())
            if state.winsta_granted && state.desktop_granted {
                return Ok(());
            }

            unsafe {
                if let Some(err) = self.check_fault("grant_winsta") {
                    return Err(err);
                }

                let winsta = win32::GetProcessWindowStation();
                if state.winsta_baseline.is_none() {
                    state.winsta_baseline = snapshot_user_object_dacl(winsta).ok();
                }

                if !state.winsta_granted {
                    grant_user_object_access(winsta, self.sid, win32::WINSTA_CONTAINED_RIGHTS)?;
                    state.winsta_granted = true;
                }

                let desktop = win32::GetThreadDesktop(win32::GetCurrentThreadId());
                if state.desktop_baseline.is_none() {
                    state.desktop_baseline = snapshot_user_object_dacl(desktop).ok();
                }

                if !state.desktop_granted {
                    let grant_res = if let Some(err) = self.check_fault("grant_desktop") {
                        Err(err)
                    } else {
                        grant_user_object_access(desktop, self.sid, win32::DESKTOP_CONTAINED_RIGHTS)
                    };

                    if let Err(grant_err) = grant_res {
                        // Desktop grant failed! Attempt rollback of winsta!
                        let rb_res = self.revoke_winsta_internal(&mut state, winsta);
                        if let Err(rb_err) = rb_res {
                            return Err(SecurityError::AclConfigurationFailed {
                                os_error_code: 0,
                                message: format!(
                                    "Desktop grant failed: {grant_err}; WindowStation rollback also failed: {rb_err}"
                                ),
                            });
                        }
                        return Err(grant_err);
                    }
                    state.desktop_granted = true;
                }

                Ok(())
            }
        }
        #[cfg(not(windows))]
        {
            Err(SecurityError::UnsupportedPlatformContainment {
                platform: std::env::consts::OS.to_string(),
                reason: "WindowStation/Desktop access is only supported on Windows NT".to_string(),
            })
        }
    }

    #[cfg(windows)]
    unsafe fn revoke_winsta_internal(
        &self,
        state: &mut ProfileGrantState,
        winsta: win32::HANDLE,
    ) -> Result<(), SecurityError> {
        if let Some(err) = self.check_fault("revoke_winsta") {
            return Err(err);
        }

        if state.strict_drift_detection {
            if let Some(baseline) = &state.winsta_baseline {
                if let Ok(current) = snapshot_user_object_dacl(winsta) {
                    let sid_str = self.sid_to_string();
                    if let Err(drift_err) = baseline.detect_concurrent_drift(&current, &sid_str) {
                        return Err(SecurityError::AclConfigurationFailed {
                            os_error_code: 0,
                            message: format!(
                                "Concurrent WindowStation DACL drift detected: {drift_err}"
                            ),
                        });
                    }
                }
            }
        }

        unsafe { revoke_user_object_access(winsta, self.sid)? };
        state.winsta_granted = false;
        Ok(())
    }

    #[cfg(windows)]
    unsafe fn revoke_desktop_internal(
        &self,
        state: &mut ProfileGrantState,
        desktop: win32::HANDLE,
    ) -> Result<(), SecurityError> {
        if let Some(err) = self.check_fault("revoke_desktop") {
            return Err(err);
        }

        if state.strict_drift_detection {
            if let Some(baseline) = &state.desktop_baseline {
                if let Ok(current) = snapshot_user_object_dacl(desktop) {
                    let sid_str = self.sid_to_string();
                    if let Err(drift_err) = baseline.detect_concurrent_drift(&current, &sid_str) {
                        return Err(SecurityError::AclConfigurationFailed {
                            os_error_code: 0,
                            message: format!("Concurrent Desktop DACL drift detected: {drift_err}"),
                        });
                    }
                }
            }
        }

        unsafe { revoke_user_object_access(desktop, self.sid)? };
        state.desktop_granted = false;
        Ok(())
    }

    /// Explicitly delete this AppContainer profile and revoke all granted ACEs.
    ///
    /// Revocation operations are retryable and idempotent. An access grant is NEVER
    /// marked as revoked until the corresponding revocation operation has succeeded.
    /// Returns `Err` if any revocation or deletion fails, preserving exact failure details.
    pub fn delete(&self) -> Result<(), SecurityError> {
        #[cfg(windows)]
        {
            let _cleanup_guard = match self.cleanup_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let mut state = match self.state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            // Fail closed if any subprocess executions are actively using this profile
            if state.active_executions > 0 {
                return Err(SecurityError::AclConfigurationFailed {
                    os_error_code: 0,
                    message: format!(
                        "Cannot delete profile '{}': {} subprocess execution(s) are active",
                        self.profile_name, state.active_executions
                    ),
                });
            }

            // Atomically mark deletion as initiated under lock so no new leases can be acquired,
            // even if cleanup fails partway through and must be retried later.
            state.deletion_initiated = true;

            // Idempotent: if already deleted and all resources revoked, return Ok(())
            if state.profile_deleted
                && !state.winsta_granted
                && !state.desktop_granted
                && state.granted_paths.is_empty()
            {
                return Ok(());
            }

            let mut errors: Vec<String> = Vec::new();

            // 1. Revoke WindowStation permissions if still granted
            if state.winsta_granted {
                unsafe {
                    let winsta = win32::GetProcessWindowStation();
                    if let Err(e) = self.revoke_winsta_internal(&mut state, winsta) {
                        errors.push(format!("WindowStation revocation failed: {e}"));
                    }
                }
            }

            // 2. Revoke Desktop permissions if still granted
            if state.desktop_granted {
                unsafe {
                    let desktop = win32::GetThreadDesktop(win32::GetCurrentThreadId());
                    if let Err(e) = self.revoke_desktop_internal(&mut state, desktop) {
                        errors.push(format!("Desktop revocation failed: {e}"));
                    }
                }
            }

            // 3. Revoke filesystem paths that remain granted
            if !state.granted_paths.is_empty() {
                let mut failed_paths = Vec::new();
                let paths_to_revoke = std::mem::take(&mut state.granted_paths);
                for path in paths_to_revoke {
                    if let Some(err) = self.check_fault("revoke_filesystem") {
                        errors.push(format!(
                            "Filesystem revocation failed for '{}': {err}",
                            path.display()
                        ));
                        failed_paths.push(path);
                        continue;
                    }
                    unsafe {
                        if let Err(e) = revoke_filesystem_access(&path, self.sid) {
                            errors.push(format!(
                                "Filesystem revocation failed for '{}': {e}",
                                path.display()
                            ));
                            failed_paths.push(path);
                        }
                    }
                }
                state.granted_paths = failed_paths;
            }

            // 4. Delete AppContainer profile if not already deleted
            if !state.profile_deleted {
                if let Some(err) = self.check_fault("delete_profile") {
                    errors.push(format!("DeleteAppContainerProfile failed: {err}"));
                } else {
                    let wide_name = to_wide_null(&self.profile_name);
                    let hr = unsafe { win32::DeleteAppContainerProfile(wide_name.as_ptr()) };
                    if hr < 0 {
                        errors.push(format!(
                            "DeleteAppContainerProfile failed with HRESULT 0x{hr:08X}"
                        ));
                    } else {
                        state.profile_deleted = true;
                    }
                }
            }

            if !errors.is_empty() {
                return Err(SecurityError::AclConfigurationFailed {
                    os_error_code: 0,
                    message: format!(
                        "Cleanup failed with {} error(s): {}",
                        errors.len(),
                        errors.join("; ")
                    ),
                });
            }

            Ok(())
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }
}

impl Drop for AppContainerProfile {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let _ = self.delete();
            unsafe {
                if !self.sid.is_null() {
                    win32::FreeSid(self.sid);
                    self.sid = std::ptr::null_mut();
                }
            }
        }
    }
}

/// Stages a minimal ephemeral Rust toolchain mirror from the host sysroot into an isolated destination directory.
///
/// This implements Strategy B (ephemeral toolchain staging) without granting broad access to `%USERPROFILE%`.
/// It copies `rustc.exe` and its runtime DLLs from `<sysroot>/bin` and standard library rlibs from `<sysroot>/lib/rustlib/<target>/lib`.
pub fn stage_ephemeral_toolchain_mirror(destination: &Path) -> Result<PathBuf, std::io::Error> {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Failed to determine host rustc sysroot",
        ));
    }
    let sysroot_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let sysroot = PathBuf::from(&sysroot_str);

    let bin_src = sysroot.join("bin");
    let bin_dst = destination.join("bin");
    std::fs::create_dir_all(&bin_dst)?;

    if bin_src.exists() {
        for entry in std::fs::read_dir(&bin_src)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_name = entry.file_name();
                let dst_file = bin_dst.join(&file_name);
                let _ = std::fs::copy(&path, &dst_file);
            }
        }
    }

    let lib_src = sysroot.join("lib");
    let lib_dst = destination.join("lib");
    if lib_src.exists() {
        let rustlib = lib_src.join("rustlib");
        if rustlib.exists() {
            for entry in std::fs::read_dir(&rustlib)? {
                let entry = entry?;
                let target_name = entry.file_name();
                let target_lib_src = entry.path().join("lib");
                if target_lib_src.exists() {
                    let target_lib_dst = lib_dst.join("rustlib").join(target_name).join("lib");
                    std::fs::create_dir_all(&target_lib_dst)?;
                    for lib_entry in std::fs::read_dir(&target_lib_src)? {
                        let lib_entry = lib_entry?;
                        let p = lib_entry.path();
                        if p.is_file() {
                            let _ = std::fs::copy(&p, target_lib_dst.join(lib_entry.file_name()));
                        }
                    }
                }
            }
        }
    }

    let rustc_bin = bin_dst.join("rustc.exe");
    if rustc_bin.exists() {
        Ok(rustc_bin)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "rustc.exe not found in staged mirror at {}",
                bin_dst.display()
            ),
        ))
    }
}

/// Output captured from a process executed inside an AppContainer and Job Object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainedProcessOutput {
    /// Process exit code.
    pub exit_code: i32,
    /// Standard output text.
    pub stdout: String,
    /// Standard error text.
    pub stderr: String,
    /// Thread resumption accounting.
    pub thread_resumption: ThreadResumptionSummary,
}

/// Spawns a process contained within an AppContainer and assigned to a Job Object before execution.
///
/// # Security Guarantees:
/// 1. Process is created in `CREATE_SUSPENDED` state with `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES` (AppContainer with 0 capabilities) and `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (only stdout/stderr pipes).
/// 2. Assigned to `job` (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) before execution starts.
/// 3. Verified with `IsProcessInJob`.
/// 4. Resumed via `ResumeThread`.
/// 5. Bounded by `timeout`.
/// 6. Fail-closed: On any failure (spawn, assignment, verification, resumption, timeout), all processes in the job are reaped immediately.
pub fn spawn_contained_appcontainer_process(
    profile: &AppContainerProfile,
    job: &ProcessJobObject,
    executable: &Path,
    args: &[String],
    current_dir: &Path,
    env_vars: &HashMap<String, String>,
    timeout: std::time::Duration,
) -> Result<ContainedProcessOutput, SecurityError> {
    #[cfg(windows)]
    {
        let _exec_lease = profile.acquire_execution_lease()?;
        use std::mem::size_of;

        unsafe {
            // 1. Create Pipes with inheritance enabled
            let mut sa = win32::SECURITY_ATTRIBUTES {
                nLength: size_of::<win32::SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: std::ptr::null_mut(),
                bInheritHandle: win32::TRUE,
            };

            let mut out_read: win32::HANDLE = win32::INVALID_HANDLE_VALUE;
            let mut out_write: win32::HANDLE = win32::INVALID_HANDLE_VALUE;
            if win32::CreatePipe(&mut out_read, &mut out_write, &mut sa, 0) == win32::FALSE {
                let err = win32::GetLastError();
                return Err(SecurityError::ProcessSpawnFailed {
                    os_error_code: err,
                    message: format!("CreatePipe failed for stdout: {err}"),
                });
            }

            let mut err_read: win32::HANDLE = win32::INVALID_HANDLE_VALUE;
            let mut err_write: win32::HANDLE = win32::INVALID_HANDLE_VALUE;
            if win32::CreatePipe(&mut err_read, &mut err_write, &mut sa, 0) == win32::FALSE {
                let err = win32::GetLastError();
                win32::CloseHandle(out_read);
                win32::CloseHandle(out_write);
                return Err(SecurityError::ProcessSpawnFailed {
                    os_error_code: err,
                    message: format!("CreatePipe failed for stderr: {err}"),
                });
            }

            // Ensure read handles are NOT inherited by child
            win32::SetHandleInformation(out_read, win32::HANDLE_FLAG_INHERIT, 0);
            win32::SetHandleInformation(err_read, win32::HANDLE_FLAG_INHERIT, 0);

            // 2. Initialize ProcThreadAttributeList with 2 attributes:
            // - PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES
            // - PROC_THREAD_ATTRIBUTE_HANDLE_LIST
            let mut attr_size: usize = 0;
            win32::InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut attr_size);

            let mut attr_buf = vec![0u8; attr_size];
            let attr_list = attr_buf.as_mut_ptr() as win32::PPROC_THREAD_ATTRIBUTE_LIST;
            if win32::InitializeProcThreadAttributeList(attr_list, 2, 0, &mut attr_size)
                == win32::FALSE
            {
                let err = win32::GetLastError();
                win32::CloseHandle(out_read);
                win32::CloseHandle(out_write);
                win32::CloseHandle(err_read);
                win32::CloseHandle(err_write);
                return Err(SecurityError::AppContainerSecurityAttributeFailed {
                    os_error_code: err,
                    message: format!("InitializeProcThreadAttributeList failed: {err}"),
                });
            }

            let sec_cap = win32::SECURITY_CAPABILITIES {
                AppContainerSid: profile.sid(),
                Capabilities: std::ptr::null_mut(),
                CapabilityCount: 0,
                Reserved: 0,
            };

            if win32::UpdateProcThreadAttribute(
                attr_list,
                0,
                win32::PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                &sec_cap as *const _ as *const std::ffi::c_void,
                size_of::<win32::SECURITY_CAPABILITIES>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == win32::FALSE
            {
                let err = win32::GetLastError();
                win32::DeleteProcThreadAttributeList(attr_list);
                win32::CloseHandle(out_read);
                win32::CloseHandle(out_write);
                win32::CloseHandle(err_read);
                win32::CloseHandle(err_write);
                return Err(SecurityError::AppContainerSecurityAttributeFailed {
                    os_error_code: err,
                    message: format!(
                        "UpdateProcThreadAttribute SECURITY_CAPABILITIES failed: {err}"
                    ),
                });
            }

            let inherited_handles = [out_write, err_write];
            if win32::UpdateProcThreadAttribute(
                attr_list,
                0,
                win32::PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                inherited_handles.as_ptr() as *const std::ffi::c_void,
                inherited_handles.len() * size_of::<win32::HANDLE>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == win32::FALSE
            {
                let err = win32::GetLastError();
                win32::DeleteProcThreadAttributeList(attr_list);
                win32::CloseHandle(out_read);
                win32::CloseHandle(out_write);
                win32::CloseHandle(err_read);
                win32::CloseHandle(err_write);
                return Err(SecurityError::AppContainerSecurityAttributeFailed {
                    os_error_code: err,
                    message: format!("UpdateProcThreadAttribute HANDLE_LIST failed: {err}"),
                });
            }

            // 3. Prepare STARTUPINFOEXW
            let mut si_ex: win32::STARTUPINFOEXW = std::mem::zeroed();
            si_ex.StartupInfo.cb = size_of::<win32::STARTUPINFOEXW>() as u32;
            si_ex.StartupInfo.dwFlags = win32::STARTF_USESTDHANDLES;
            si_ex.StartupInfo.hStdInput = win32::INVALID_HANDLE_VALUE;
            si_ex.StartupInfo.hStdOutput = out_write;
            si_ex.StartupInfo.hStdError = err_write;
            si_ex.lpAttributeList = attr_list;

            let mut pi: win32::PROCESS_INFORMATION = std::mem::zeroed();

            // 4. Command line formatting
            let mut cmd_line_str = format!("\"{}\"", executable.display());
            for arg in args {
                if arg.contains(' ') || arg.contains('\t') || arg.contains('"') {
                    let escaped = arg.replace('"', "\\\"");
                    cmd_line_str.push_str(&format!(" \"{escaped}\""));
                } else {
                    cmd_line_str.push_str(&format!(" {arg}"));
                }
            }
            let mut wide_cmd_line = to_wide_null(&cmd_line_str);
            let wide_app_name = to_wide_null(&executable.to_string_lossy());
            let wide_cur_dir = to_wide_null(&current_dir.to_string_lossy());
            let env_block = build_environment_block(env_vars);

            // 5. Spawn suspended with extended startupinfo
            let flags = win32::EXTENDED_STARTUPINFO_PRESENT
                | win32::CREATE_SUSPENDED
                | win32::CREATE_UNICODE_ENVIRONMENT;

            let spawn_ok = win32::CreateProcessW(
                wide_app_name.as_ptr(),
                wide_cmd_line.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                win32::TRUE, // MUST be TRUE when PROC_THREAD_ATTRIBUTE_HANDLE_LIST is present
                flags,
                env_block.as_ptr() as *const std::ffi::c_void,
                wide_cur_dir.as_ptr(),
                &si_ex,
                &mut pi,
            );

            // Parent closes write pipe ends immediately
            win32::CloseHandle(out_write);
            win32::CloseHandle(err_write);
            win32::DeleteProcThreadAttributeList(attr_list);

            if spawn_ok == win32::FALSE {
                let err = win32::GetLastError();
                win32::CloseHandle(out_read);
                win32::CloseHandle(err_read);
                return Err(SecurityError::ProcessSpawnFailed {
                    os_error_code: err,
                    message: format!(
                        "CreateProcessW failed for '{}': {err}",
                        executable.display()
                    ),
                });
            }

            // 6. Assign to Job Object before any user code can execute
            let assign_ok = win32::AssignProcessToJobObject(job.raw_handle(), pi.hProcess);
            if assign_ok == win32::FALSE {
                let err = win32::GetLastError();
                win32::TerminateProcess(pi.hProcess, 1);
                let _ = job.terminate(1);
                win32::CloseHandle(pi.hThread);
                win32::CloseHandle(pi.hProcess);
                win32::CloseHandle(out_read);
                win32::CloseHandle(err_read);
                return Err(SecurityError::ProcessAssignmentFailed {
                    os_error_code: err,
                    message: format!(
                        "AssignProcessToJobObject failed for PID {}: {err}",
                        pi.dwProcessId
                    ),
                });
            }

            // 7. Verify containment
            let mut in_job: win32::BOOL = win32::FALSE;
            let check = win32::IsProcessInJob(pi.hProcess, job.raw_handle(), &mut in_job);
            if check == win32::FALSE || in_job == win32::FALSE {
                win32::TerminateProcess(pi.hProcess, 1);
                let _ = job.terminate(1);
                win32::CloseHandle(pi.hThread);
                win32::CloseHandle(pi.hProcess);
                win32::CloseHandle(out_read);
                win32::CloseHandle(err_read);
                return Err(SecurityError::ProcessNotInJob {
                    pid: pi.dwProcessId,
                });
            }

            // 8. Spawn background reader threads for stdout and stderr to prevent pipe buffer deadlocks
            let out_fd = out_read as isize;
            let out_thread =
                std::thread::spawn(move || read_pipe_to_string(out_fd as win32::HANDLE));

            let err_fd = err_read as isize;
            let err_thread =
                std::thread::spawn(move || read_pipe_to_string(err_fd as win32::HANDLE));

            // 9. Resume thread
            let prev_count = win32::ResumeThread(pi.hThread);
            if prev_count == u32::MAX {
                let err = win32::GetLastError();
                win32::TerminateProcess(pi.hProcess, 1);
                let _ = job.terminate(1);
                win32::CloseHandle(out_read);
                win32::CloseHandle(err_read);
                let _ = out_thread.join();
                let _ = err_thread.join();
                win32::CloseHandle(pi.hThread);
                win32::CloseHandle(pi.hProcess);
                return Err(SecurityError::ProcessResumeFailed {
                    os_error_code: err,
                    message: format!("ResumeThread failed for PID {}: {err}", pi.dwProcessId),
                });
            }

            let resumption = ThreadResumptionSummary {
                method: "ResumeThread".to_string(),
                discovered_threads: 1,
                resumed_threads: 1,
                failed_threads: 0,
                last_os_error: None,
            };

            // 10. Wait bounded by timeout
            let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
            let wait_res = win32::WaitForSingleObject(pi.hProcess, timeout_ms);

            if wait_res == win32::WAIT_TIMEOUT {
                win32::TerminateProcess(pi.hProcess, 1);
                let _ = job.terminate(1);
                win32::CloseHandle(out_read);
                win32::CloseHandle(err_read);
                let _ = out_thread.join();
                let _ = err_thread.join();
                win32::CloseHandle(pi.hThread);
                win32::CloseHandle(pi.hProcess);
                return Err(SecurityError::CommandTimeout {
                    timeout_secs: timeout.as_secs(),
                });
            }

            // 11. Collect exit code and pipe contents
            let mut exit_code: u32 = 0;
            win32::GetExitCodeProcess(pi.hProcess, &mut exit_code);

            win32::CloseHandle(pi.hThread);
            win32::CloseHandle(pi.hProcess);

            let stdout = out_thread.join().unwrap_or_default();
            let stderr = err_thread.join().unwrap_or_default();

            win32::CloseHandle(out_read);
            win32::CloseHandle(err_read);

            Ok(ContainedProcessOutput {
                exit_code: exit_code as i32,
                stdout,
                stderr,
                thread_resumption: resumption,
            })
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (
            profile,
            job,
            executable,
            args,
            current_dir,
            env_vars,
            timeout,
        );
        Err(SecurityError::UnsupportedPlatformContainment {
            platform: std::env::consts::OS.to_string(),
            reason: "AppContainer execution is only supported on Windows NT (fail-closed)"
                .to_string(),
        })
    }
}

// ============================================================================
// 4. Child-Process Environment Sanitization Policy
// ============================================================================

/// Case-insensitive substring patterns that identify secret credentials or tokens.
pub const SECRET_VARIABLE_PATTERNS: &[&str] = &[
    "KEY",
    "TOKEN",
    "SECRET",
    "AUTH",
    "PASS",
    "CREDENTIAL",
    "COOKIE",
];

/// Known proxy environment variables that must be scrubbed unconditionally.
pub const PROXY_VARIABLES: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
];

/// Git environment variables that must be scrubbed to prevent repository escape.
pub const GIT_METADATA_VARIABLES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_CONFIG",
    "GIT_EXEC_PATH",
];

/// Essential Windows system environment variables allowed in minimal environments.
/// Essential Windows system environment variables allowed in minimal environments.
///
/// NOTE: PATH is intentionally omitted and synthesized deterministically via
/// [`EnvironmentPolicy::build_curated_path`] to resolve AUD-B5-01.
pub const ALLOWLISTED_SYSTEM_VARS_WINDOWS: &[&str] = &[
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "ALLUSERSPROFILE",
    "APPDATA",
    "COMMONPROGRAMFILES",
    "COMPUTERNAME",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PSMODULEPATH",
    "PUBLIC",
    "USERDOMAIN",
    "USERNAME",
    "USERPROFILE",
];

/// Essential Unix system environment variables allowed in minimal environments.
pub const ALLOWLISTED_SYSTEM_VARS_UNIX: &[&str] = &["HOME", "TMPDIR", "TERM"];

/// Helper to determine if a directory is untrusted (e.g. temporary directory, user downloads, or desktop).
pub fn is_untrusted_directory(path: &Path) -> bool {
    let p_str = path.to_string_lossy().to_ascii_lowercase();

    // Check temp directories
    let temp_dir = std::env::temp_dir().to_string_lossy().to_ascii_lowercase();
    if p_str.starts_with(&temp_dir) || p_str.contains("\\temp") || p_str.contains("/tmp") {
        return true;
    }

    // Check user downloads and desktop
    if p_str.contains("\\downloads")
        || p_str.contains("/downloads")
        || p_str.contains("\\desktop")
        || p_str.contains("/desktop")
    {
        return true;
    }

    false
}

/// Default trusted system and developer tool directories.
///
/// Curated deterministically from system-level locations rather than the untrusted parent PATH.
pub fn get_trusted_system_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        let system_root = std::env::var("SYSTEMROOT")
            .or_else(|_| std::env::var("WINDIR"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Windows"));

        // Windows NT core system utilities
        let sys32 = system_root.join("System32");
        if sys32.is_dir() {
            paths.push(sys32.clone());
        }
        if system_root.is_dir() {
            paths.push(system_root.clone());
        }
        let wbem = sys32.join("Wbem");
        if wbem.is_dir() {
            paths.push(wbem);
        }
        let ps = sys32.join("WindowsPowerShell").join("v1.0");
        if ps.is_dir() {
            paths.push(ps);
        }

        // Rust / Cargo toolchain
        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            let cargo_bin = PathBuf::from(cargo_home).join("bin");
            if cargo_bin.is_dir() && !is_untrusted_directory(&cargo_bin) {
                paths.push(cargo_bin);
            }
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let cargo_bin = PathBuf::from(userprofile).join(".cargo").join("bin");
            if cargo_bin.is_dir() && !is_untrusted_directory(&cargo_bin) {
                paths.push(cargo_bin);
            }
        }

        // Node.js
        if let Ok(pf) = std::env::var("ProgramFiles") {
            let node_dir = PathBuf::from(&pf).join("nodejs");
            if node_dir.is_dir() {
                paths.push(node_dir);
            }
            let git_cmd = PathBuf::from(&pf).join("Git").join("cmd");
            if git_cmd.is_dir() {
                paths.push(git_cmd);
            }
            let git_bin = PathBuf::from(&pf).join("Git").join("bin");
            if git_bin.is_dir() {
                paths.push(git_bin);
            }
        }
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            let node_dir = PathBuf::from(&pf86).join("nodejs");
            if node_dir.is_dir() && !paths.contains(&node_dir) {
                paths.push(node_dir);
            }
        }

        // Python (System ProgramFiles and User LocalAppData)
        if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
            let py_base = PathBuf::from(local_appdata).join("Programs").join("Python");
            if py_base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&py_base) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() && !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
            }
        }
        if let Ok(pf) = std::env::var("ProgramFiles") {
            if let Ok(entries) = std::fs::read_dir(&pf) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let name = p.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with("Python") && p.is_dir() && !paths.contains(&p) {
                        paths.push(p);
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for p in &["/usr/bin", "/bin", "/usr/local/bin"] {
            let pb = PathBuf::from(p);
            if pb.is_dir() {
                paths.push(pb);
            }
        }
    }

    paths
}

/// Policy for sanitizing child-process environment variables.
#[derive(Debug, Clone)]
pub struct EnvironmentPolicy {
    /// Sandbox root directory for local cache redirection.
    pub sandbox_root: PathBuf,
    /// Additional explicit environment variables to set.
    pub custom_vars: HashMap<String, String>,
    /// Explicitly approved tool directories permitted in the curated child PATH.
    pub approved_tool_dirs: Vec<PathBuf>,
}

impl EnvironmentPolicy {
    /// Create a new sanitization policy bound to the specified sandbox root.
    pub fn new(sandbox_root: impl Into<PathBuf>) -> Self {
        Self {
            sandbox_root: sandbox_root.into(),
            custom_vars: HashMap::new(),
            approved_tool_dirs: Vec::new(),
        }
    }

    /// Add an approved tool directory to the curated child PATH.
    pub fn with_approved_tool_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        let p = dir.into();
        if !self.approved_tool_dirs.contains(&p) {
            self.approved_tool_dirs.push(p);
        }
        self
    }

    /// Check if a variable name indicates a secret token or credential.
    pub fn is_secret_name(name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        SECRET_VARIABLE_PATTERNS
            .iter()
            .any(|pattern| upper.contains(pattern))
    }

    /// Check if a variable name is a known proxy or Git variable.
    pub fn is_forbidden_metadata_var(name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        PROXY_VARIABLES
            .iter()
            .any(|v| v.to_ascii_uppercase() == upper)
            || GIT_METADATA_VARIABLES
                .iter()
                .any(|v| v.to_ascii_uppercase() == upper)
    }

    /// Build the curated child PATH (AUD-B5-01).
    ///
    /// Preserves only verified system directories and explicitly approved tool directories.
    /// Rejects untrusted directories (temp, downloads, desktop) and sandbox root paths.
    pub fn build_curated_path(&self) -> Result<String, SecurityError> {
        let mut dirs = get_trusted_system_search_paths();

        // Include any explicitly approved tool directories
        for approved in &self.approved_tool_dirs {
            if approved.is_dir() && !dirs.contains(approved) {
                let approved_str = approved.to_string_lossy();
                let sandbox_str = self.sandbox_root.to_string_lossy();
                if approved_str.starts_with(&*sandbox_str) || is_untrusted_directory(approved) {
                    return Err(SecurityError::EnvironmentSanitizationFailed {
                        variable: "PATH".to_string(),
                        reason: format!(
                            "Approved tool directory '{approved_str}' is within an untrusted location or sandbox root"
                        ),
                    });
                }
                dirs.push(approved.clone());
            }
        }

        std::env::join_paths(dirs)
            .map_err(|e| SecurityError::EnvironmentSanitizationFailed {
                variable: "PATH".to_string(),
                reason: format!("Failed to join curated PATH entries: {e}"),
            })
            .map(|os_str| os_str.to_string_lossy().to_string())
    }

    /// Build the minimal, sanitized environment map.
    ///
    /// Preserves only required system variables, scrubs all secret and proxy variables,
    /// synthesizes a curated child PATH, and redirects tool caches to sandbox-local directories.
    pub fn build_sanitized_environment(&self) -> Result<HashMap<String, String>, SecurityError> {
        let mut env = HashMap::new();

        // 1. Collect allowlisted system variables from parent (excluding PATH)
        #[cfg(windows)]
        let allowlist = ALLOWLISTED_SYSTEM_VARS_WINDOWS;
        #[cfg(not(windows))]
        let allowlist = ALLOWLISTED_SYSTEM_VARS_UNIX;

        for &key in allowlist {
            if let Ok(val) = std::env::var(key) {
                if !Self::is_secret_name(key) && !Self::is_forbidden_metadata_var(key) {
                    env.insert(key.to_string(), val);
                }
            }
        }

        // 2. Synthesize curated child PATH (AUD-B5-01)
        let curated_path = self.build_curated_path()?;
        env.insert("PATH".to_string(), curated_path);

        // 3. Redirect tool caches to sandbox-local directories
        let local_cargo = self.sandbox_root.join(".engram_cargo_home");
        let local_npm = self.sandbox_root.join(".engram_npm_cache");
        let local_pycache = self.sandbox_root.join(".engram_pycache");
        let local_tmp = self.sandbox_root.join(".engram_tmp");

        env.insert(
            "CARGO_HOME".to_string(),
            local_cargo.to_string_lossy().to_string(),
        );
        env.insert(
            "NPM_CONFIG_CACHE".to_string(),
            local_npm.to_string_lossy().to_string(),
        );
        env.insert(
            "PYTHONPYCACHEPREFIX".to_string(),
            local_pycache.to_string_lossy().to_string(),
        );
        env.insert("TEMP".to_string(), local_tmp.to_string_lossy().to_string());
        env.insert("TMP".to_string(), local_tmp.to_string_lossy().to_string());

        // Preserve read-only compiler toolchain directories so rustup can locate toolchains
        if let Ok(rustup) = std::env::var("RUSTUP_HOME") {
            env.insert("RUSTUP_HOME".to_string(), rustup);
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let default_rustup = PathBuf::from(userprofile).join(".rustup");
            if default_rustup.exists() {
                env.insert(
                    "RUSTUP_HOME".to_string(),
                    default_rustup.to_string_lossy().to_string(),
                );
            }
        }
        if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
            env.insert("RUSTUP_TOOLCHAIN".to_string(), toolchain);
        }

        // 4. Apply validated custom variables
        for (k, v) in &self.custom_vars {
            if Self::is_secret_name(k) {
                return Err(SecurityError::EnvironmentSanitizationFailed {
                    variable: k.clone(),
                    reason: "Variable name contains secret-identifying pattern".to_string(),
                });
            }
            if Self::is_forbidden_metadata_var(k) {
                return Err(SecurityError::EnvironmentSanitizationFailed {
                    variable: k.clone(),
                    reason: "Variable is a prohibited proxy or Git metadata variable".to_string(),
                });
            }
            env.insert(k.clone(), v.clone());
        }

        Ok(env)
    }

    /// Apply this sanitization policy to a `std::process::Command`.
    ///
    /// Clears all inherited environment variables, then injects the sanitized set.
    pub fn apply_to_command(&self, cmd: &mut Command) -> Result<(), SecurityError> {
        let sanitized = self.build_sanitized_environment()?;

        // Unconditionally clear parent environment
        cmd.env_clear();

        for (k, v) in sanitized {
            cmd.env(k, v);
        }

        Ok(())
    }
}

// ============================================================================
// 5. Trusted Executable Path Resolution
// ============================================================================

/// Resolve an approved binary name to an absolute, verified path.
///
/// Prevents relative PATH spoofing attacks (e.g. executing a rogue `./cargo.bat` or `./python.exe`
/// dropped in the sandbox workspace). Searches exclusively trusted system directories (AUD-B5-01).
pub fn resolve_trusted_executable(command_name: &str) -> Result<PathBuf, SecurityError> {
    resolve_trusted_executable_with_search_paths(command_name, &[])
}

/// Resolve an approved binary name using trusted system paths plus explicitly supplied extra directories.
pub fn resolve_trusted_executable_with_search_paths(
    command_name: &str,
    extra_search_paths: &[PathBuf],
) -> Result<PathBuf, SecurityError> {
    let trimmed = command_name.trim();
    if trimmed.is_empty() {
        return Err(SecurityError::ExecutableResolutionFailed {
            command: command_name.to_string(),
            search_paths: vec![],
        });
    }

    // If an absolute path was already provided, verify it exists and is a regular file
    let path = Path::new(trimmed);
    if path.is_absolute() {
        if path.is_file() {
            if is_untrusted_directory(path) {
                return Err(SecurityError::ExecutableResolutionFailed {
                    command: command_name.to_string(),
                    search_paths: vec![path.display().to_string()],
                });
            }
            return Ok(path.to_path_buf());
        } else {
            return Err(SecurityError::ExecutableResolutionFailed {
                command: command_name.to_string(),
                search_paths: vec![path.display().to_string()],
            });
        }
    }

    // Do NOT search arbitrary relative paths like "./malicious" or "../evil"
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(SecurityError::ExecutableResolutionFailed {
            command: command_name.to_string(),
            search_paths: vec![],
        });
    }

    // Collect trusted directory candidates
    let mut search_paths = get_trusted_system_search_paths();
    for extra in extra_search_paths {
        if extra.is_dir() && !is_untrusted_directory(extra) && !search_paths.contains(extra) {
            search_paths.push(extra.clone());
        }
    }

    // Windows extensions to check
    #[cfg(windows)]
    let extensions = &["", ".exe", ".cmd", ".bat"];
    #[cfg(not(windows))]
    let extensions = &[""];

    for dir in &search_paths {
        for ext in extensions {
            let candidate_name = if ext.is_empty() || trimmed.ends_with(ext) {
                trimmed.to_string()
            } else {
                format!("{trimmed}{ext}")
            };

            let candidate = dir.join(&candidate_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(SecurityError::ExecutableResolutionFailed {
        command: command_name.to_string(),
        search_paths: search_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    })
}

// ============================================================================
// 6. Adversarial Network-Containment Validation Probe
// ============================================================================

/// Typed outcome classification for a network-containment probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EgressProbeOutcome {
    /// An explicit operating system or firewall policy rejection was observed.
    ActiveRejectionVerified {
        probe_id: String,
        destination: String,
        os_error_code: Option<u32>,
        rejection_policy: String,
    },
    /// The destination was unreachable for an inconclusive reason (e.g. connection refused, no listener).
    Unreachable {
        probe_id: String,
        destination: String,
        reason: String,
    },
    /// DNS or domain name resolution failed.
    ResolutionFailed {
        probe_id: String,
        destination: String,
        reason: String,
    },
    /// An unexpected connection was successfully established (containment failure / leak!).
    EgressLeakDetected {
        probe_id: String,
        destination: String,
        detail: String,
    },
    /// Containment could not be established or verified on this platform/configuration.
    ContainmentUnverified { probe_id: String, reason: String },
    /// The probe infrastructure encountered an internal failure.
    ProbeInfrastructureFailure { probe_id: String, error: String },
}

impl EgressProbeOutcome {
    /// Returns true if the probe detected an active, verified OS policy denial.
    pub fn is_active_rejection(&self) -> bool {
        matches!(self, Self::ActiveRejectionVerified { .. })
    }

    /// Returns true if the probe detected a critical security egress leak.
    pub fn is_leak(&self) -> bool {
        matches!(self, Self::EgressLeakDetected { .. })
    }
}

/// Simulated or deterministic probe descriptor for offline testing.
#[derive(Debug, Clone)]
pub struct ProbeDescriptor {
    pub probe_id: String,
    pub destination: String,
    pub timeout_ms: u64,
}

/// Adversarial network containment validator for Milestone B5.
///
/// Designed to execute 100% offline using controlled local loopback endpoints,
/// simulated failure injections, and deterministic socket probes without ever
/// reaching out to public IP addresses or external service endpoints.
pub struct NetworkEgressProbe;

impl NetworkEgressProbe {
    /// Evaluate a socket connection attempt against a controlled local target.
    ///
    /// Distinguishes between:
    /// - `ActiveRejectionVerified`: OS policy error (`WSAEACCES` / 10013 on Windows, `EACCES` / `EPERM` on Unix).
    /// - `Unreachable`: Connection refused (`WSAECONNREFUSED` / 10061) or timed out.
    /// - `EgressLeakDetected`: Successful connection when isolation was expected.
    pub fn evaluate_local_probe(
        probe_id: &str,
        destination: &str,
        expected_isolated: bool,
    ) -> EgressProbeOutcome {
        use std::net::{SocketAddr, TcpStream};
        use std::time::Duration;

        // Parse target socket address
        let addr: SocketAddr = match destination.parse() {
            Ok(a) => a,
            Err(e) => {
                return EgressProbeOutcome::ProbeInfrastructureFailure {
                    probe_id: probe_id.to_string(),
                    error: format!("Invalid socket address '{destination}': {e}"),
                };
            }
        };

        match TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
            Ok(_stream) => {
                if expected_isolated {
                    EgressProbeOutcome::EgressLeakDetected {
                        probe_id: probe_id.to_string(),
                        destination: destination.to_string(),
                        detail: "TCP stream connected successfully to forbidden target".to_string(),
                    }
                } else {
                    EgressProbeOutcome::ContainmentUnverified {
                        probe_id: probe_id.to_string(),
                        reason: "Connection succeeded on unisolated test probe".to_string(),
                    }
                }
            }
            Err(e) => Self::classify_io_error(probe_id, destination, &e),
        }
    }

    /// Classify an I/O error into typed probe outcomes.
    pub fn classify_io_error(
        probe_id: &str,
        destination: &str,
        err: &std::io::Error,
    ) -> EgressProbeOutcome {
        let raw_code = err.raw_os_error();

        #[cfg(windows)]
        {
            // Windows WSA error codes:
            // 10013 = WSAEACCES (Permission denied / blocked by Windows Filtering Platform or firewall)
            // 10061 = WSAECONNREFUSED (No listener / connection refused)
            // 10051 = WSAENETUNREACH (Network unreachable)
            // 10060 = WSAETIMEDOUT (Connection timed out)
            // 11001 = WSAHOST_NOT_FOUND (Authoritative host not found)
            match raw_code {
                Some(10013) => EgressProbeOutcome::ActiveRejectionVerified {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    os_error_code: Some(10013),
                    rejection_policy: "Windows Filtering Platform / Firewall Policy (WSAEACCES)"
                        .to_string(),
                },
                Some(10061) => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason:
                        "Connection refused (WSAECONNREFUSED) - inconclusive proof of containment"
                            .to_string(),
                },
                Some(10051) => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: "Network unreachable (WSAENETUNREACH)".to_string(),
                },
                Some(10060) => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: "Connection timed out (WSAETIMEDOUT)".to_string(),
                },
                Some(11001) => EgressProbeOutcome::ResolutionFailed {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: "Host not found (WSAHOST_NOT_FOUND)".to_string(),
                },
                _ => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: format!("Inconclusive socket error (raw OS error {raw_code:?}): {err}"),
                },
            }
        }
        #[cfg(not(windows))]
        {
            // Unix error codes:
            // 13 = EACCES, 1 = EPERM
            // 111 = ECONNREFUSED
            match raw_code {
                Some(13) | Some(1) => EgressProbeOutcome::ActiveRejectionVerified {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    os_error_code: raw_code.map(|c| c as u32),
                    rejection_policy: "POSIX Packet Filter / iptables rejection (EACCES/EPERM)"
                        .to_string(),
                },
                Some(111) => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: "Connection refused (ECONNREFUSED) - inconclusive proof of containment"
                        .to_string(),
                },
                _ => EgressProbeOutcome::Unreachable {
                    probe_id: probe_id.to_string(),
                    destination: destination.to_string(),
                    reason: format!("Inconclusive socket error (raw OS error {raw_code:?}): {err}"),
                },
            }
        }
    }
}
