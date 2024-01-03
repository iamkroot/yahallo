use std::time::Duration;

use ctor::ctor;
use dbus::blocking::Connection;
use log::{error, info, warn};
use pamsm::{pam_module, Pam, PamError, PamFlags, PamLibExt, PamServiceModule};
use yahallo::Error;

struct YahalloDbus;

impl YahalloDbus {}

#[ctor]
fn setup_logger() {
    let formatter = syslog::Formatter3164 {
        facility: syslog::Facility::LOG_AUTH,
        ..Default::default()
    };

    let logger = match syslog::unix(formatter) {
        Err(e) => {
            println!("impossible to connect to syslog: {:?}", e);
            return;
        }
        Ok(logger) => logger,
    };
    log::set_boxed_logger(Box::new(syslog::BasicLogger::new(logger)))
        .map(|()| log::set_max_level(log::LevelFilter::Debug))
        .unwrap()
}

impl PamServiceModule for YahalloDbus {
    fn authenticate(pamh: Pam, _flags: PamFlags, _args: Vec<String>) -> PamError {
        info!("starting face match");
        let user = match pamh.get_user(None) {
            Ok(Some(u)) => match u.to_str() {
                Ok(s) => s.to_owned(),
                Err(e) => {
                    error!("Error getting user name {e}");
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
                error!("Error opening DBus conn {e}");
                return PamError::SYSTEM_ERR;
            }
        };
        const NAME: &str = "com.iamkroot.yahallo";

        let proxy = conn.with_proxy(NAME, "/", Duration::from_secs(30));
        let (success, err): (bool, Error) = match proxy.method_call(NAME, "CheckMatch", (user,)) {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Failed DBus call {e}");
                warn!("{msg}");
                if let Err(e2) = pamh.conv(Some(&msg), pamsm::PamMsgStyle::ERROR_MSG) {
                    // error showing pam conv
                    eprintln!("wut {e2}");
                    return PamError::CONV_ERR;
                };
                return PamError::SERVICE_ERR;
            }
        };
        if !success {
            let msg = err.to_string();
            warn!("Error: {msg}");
            if let Err(e2) = pamh.conv(Some(&msg), pamsm::PamMsgStyle::ERROR_MSG) {
                // error showing pam conv
                eprintln!("wut {e2}");
                return PamError::CONV_ERR;
            };
            return PamError::AUTH_ERR;
        }
        // TODO: Verify username
        PamError::SUCCESS
    }
}

pam_module!(YahalloDbus);
