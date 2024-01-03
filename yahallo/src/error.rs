#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Timeout waiting for frame!")]
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

impl dbus::arg::Arg for Error {
    const ARG_TYPE: dbus::arg::ArgType = dbus::arg::ArgType::Variant;

    fn signature() -> dbus::Signature<'static> {
        unsafe { dbus::Signature::from_slice_unchecked("v\0") }
    }
}

impl dbus::arg::Append for Error {
    fn append(self, i: &mut dbus::arg::IterAppend) {
        self.append_by_ref(i)
    }

    fn append_by_ref(&self, i: &mut dbus::arg::IterAppend) {
        match self {
            Error::Timeout => i.append("Timeout"),
            Error::NoData => i.append("NoData"),
            Error::NoFace => i.append("NoFace"),
            Error::MultipleFaces => i.append("MultipleFaces"),
            Error::TooDark => i.append("TooDark"),
            Error::UnknownUser => i.append("UnknownUser"),
            Error::Other(e) => i.append(e.to_string()),
        }
    }
}

impl<'a> dbus::arg::Get<'a> for Error {
    fn get(i: &mut dbus::arg::Iter) -> Option<Self> {
        let s: String = i.get()?;
        match s.as_str() {
            "Timeout" => Some(Error::Timeout),
            "NoData" => Some(Error::NoData),
            "NoFace" => Some(Error::NoFace),
            "MultipleFaces" => Some(Error::MultipleFaces),
            "TooDark" => Some(Error::TooDark),
            "UnknownUser" => Some(Error::UnknownUser),
            _ => Some(Error::Other(anyhow::anyhow!("{s}"))),
        }
    }
}

pub type YahalloResult<T> = std::result::Result<T, Error>;
