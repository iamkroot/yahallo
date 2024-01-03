#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Timeout waiting for face!")]
    Timeout,
    #[error("No models enrolled!")]
    NoData,
    #[error("No face detected!")]
    NoFace,
    #[error("Multiple faces detected!")]
    MultipleFaces,
    #[error("Frame too dark!")]
    TooDark,
    #[error("Unknown user!")]
    UnknownUser,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Wrapper that can be sent over dbus.
#[derive(Debug)]
pub enum DbusResult {
    Success,
    Error(Error),
}

impl dbus::arg::Arg for DbusResult {
    const ARG_TYPE: dbus::arg::ArgType = dbus::arg::ArgType::String;

    fn signature() -> dbus::Signature<'static> {
        unsafe { dbus::Signature::from_slice_unchecked("s\0") }
    }
}

impl dbus::arg::Append for DbusResult {
    fn append(self, i: &mut dbus::arg::IterAppend) {
        self.append_by_ref(i)
    }

    fn append_by_ref(&self, i: &mut dbus::arg::IterAppend) {
        match self {
            DbusResult::Success => i.append("Success"),
            DbusResult::Error(err) => match err {
                Error::Timeout => i.append("Timeout"),
                Error::NoData => i.append("NoData"),
                Error::NoFace => i.append("NoFace"),
                Error::MultipleFaces => i.append("MultipleFaces"),
                Error::TooDark => i.append("TooDark"),
                Error::UnknownUser => i.append("UnknownUser"),
                Error::Other(e) => i.append(e.to_string()),
            },
        }
    }
}

impl<'a> dbus::arg::Get<'a> for DbusResult {
    fn get(i: &mut dbus::arg::Iter) -> Option<Self> {
        let s: String = i.get()?;
        match s.as_str() {
            "Success" => Some(DbusResult::Success),
            "Timeout" => Some(DbusResult::Error(Error::Timeout)),
            "NoData" => Some(DbusResult::Error(Error::NoData)),
            "NoFace" => Some(DbusResult::Error(Error::NoFace)),
            "MultipleFaces" => Some(DbusResult::Error(Error::MultipleFaces)),
            "TooDark" => Some(DbusResult::Error(Error::TooDark)),
            "UnknownUser" => Some(DbusResult::Error(Error::UnknownUser)),
            _ => Some(DbusResult::Error(Error::Other(anyhow::anyhow!(s)))),
        }
    }
}

pub type YahalloResult<T> = std::result::Result<T, Error>;
