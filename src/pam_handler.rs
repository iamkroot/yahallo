//! Exposes the PAM interface as C functions

use std::ffi::{c_char, c_int, c_void};

// TODO: Import from C header instead of hardcoding here
const PAM_IGNORE: c_int = 25;

type PamHandleT = *mut c_void;

/// Called by PAM when a user needs to be authenticated, for example by running
/// the sudo command
#[no_mangle]
pub extern "C" fn pam_sm_authenticate(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    todo!("auth");
}

/// Called by PAM when a session is started, such as by the su command
#[no_mangle]
pub extern "C" fn pam_sm_open_session(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    todo!("open");
}

#[no_mangle]
pub extern "C" fn pam_sm_acct_mgmt(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_close_session(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_chauthtok(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

#[no_mangle]
pub extern "C" fn pam_sm_setcred(
    _pamh: PamHandleT,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}
