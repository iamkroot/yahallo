use std::time::Duration;

use dbus::blocking::Connection;
use pamsm::{pam_module, Pam, PamError, PamFlags, PamLibExt, PamServiceModule};

struct YahalloDbus;

impl YahalloDbus {}

impl PamServiceModule for YahalloDbus {
    fn authenticate(pamh: Pam, _flags: PamFlags, _args: Vec<String>) -> PamError {
        let user = match pamh.get_user(None) {
            Ok(Some(u)) => match u.to_str() {
                Ok(s) => s.to_owned(),
                Err(e) => {
                    eprintln!("Error getting user name {e}");
                    return PamError::USER_UNKNOWN;
                }
            },
            Ok(None) => return PamError::USER_UNKNOWN,
            Err(e) => return e,
        };
        let conn = match Connection::new_system() {
            Ok(c) => c,
            Err(e) => {
                // TODO: should log to syslog
                eprintln!("Error opening DBus conn {e}");
                return PamError::SYSTEM_ERR;
            }
        };
        const NAME: &str = "com.iamkroot.yahallo";

        let proxy = conn.with_proxy(NAME, "/", Duration::from_secs(30));
        let (_reply,): (String,) = match proxy.method_call(NAME, "CheckMatch", (user,)) {
            Ok(r) => r,
            Err(e) => {
                if e.name() == Some("org.freedesktop.DBus.Error.Failed"){
                    return PamError::AUTH_ERR;
                }
                eprintln!("Error calling daemon {e}");
                return PamError::SERVICE_ERR;
            }
        };
        // TODO: Verify username
        PamError::SUCCESS
    }
}

pam_module!(YahalloDbus);
