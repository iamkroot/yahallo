# Yahallo

Yet Another Hallo - a Face Recognition integration for Linux.

Note: This project is in very early stages (I mean, just look at the [TODOs](#todo)!) I have only tested it on my own setup. Any contribution is appreciated! Proceed with caution.

## Building
Use a recent Rust toolchain to build this project. See https://rustup.rs for install instructions.

Build should be as simple as running `cargo build --release`.
* Note: you might also need the `dlib` library installed on your system.

The build generates three binaries-
1. `yahallo` - The CLI executable that lets you manage faces, etc.
2. `yahallod` - The daemon executable that gets loaded at computer startup and always runs in the background. 
    * It keeps the face recognition models loaded into RAM and helps reduce latency when performing authentication.
    * Runs as a DBus service, so it can be invoked by any application that needs yahallo.
3. `libpam_yahallo.so` - The PAM module that allows things like `sudo` to use yahallo.
    * Note: Ideally, this file would be named `pam_yahallo.so`, but the Rust build tool does not allow it today (see https://github.com/rust-lang/cargo/issues/1970). The best we can do is set `soname` in the build script, and later manually rename the file during installation.

## Installation

* `cp target/release/libpam_yahallo.so /usr/lib/security/pam_yahallo.so` (note the change in name)
* Set owner: `chown root:root /usr/lib/security/pam_yahallo.so`
* Set perms: `chmod 0755 /usr/lib/security/pam_yahallo.so`
* Put the daemon in a secure directory: `cp target/release/yahallod /usr/sbin/`
* Put cli in the usual place: `cp target/release/yahallo /usr/bin/`

To support autostart on boot and to expose the service via DBus:
* Install [`yahallod.service`](res/yahallod.service) to the systemd units dir (usually, `/usr/lib/systemd/system/`)
* Install DBus service unit [com.iamkroot.yahallo.service](res/com.iamkroot.yahallo.service) to `/usr/share/dbus-1/system-services/`
* Install DBus service conf [com.iamkroot.yahallo.conf](res/com.iamkroot.yahallo.conf) to `/usr/share/dbus-1/system.d/`

To install the dlib data models-
1. Download:
    * CNN Face Detector: http://dlib.net/files/shape_predictor_68_face_landmarks.dat.bz2
    * Landmark Predictor: http://dlib.net/files/mmod_human_face_detector.dat.bz2
    * Face Recognition Net: http://dlib.net/files/dlib_face_recognition_resnet_model_v1.dat.bz2
2. Extract the `.dat` files
3. Put the `.dat` in `/etc/yahallo/data/`

### Initial setup
* Use `sudo yahallo add --label $USER` to add your face

### sudo

Add `auth sufficient pam_yahallo.so` to `/etc/pam.d/sudo` before other entries.

This makes `sudo` try yahallo _before_ showing the password input.

Note that this means if the face isn't visible due to some reason, and if your timeout is too large, it will take a long time before the password prompt is shown. I have found that keeping the timeout to something like `2 seconds` provides the best tradeoff. If yahallo can't detect your face in 2 seconds for whatever reason, it likely won't help to keep trying any further; just type your password at this time.

### KDE setup

The goal is to make KDE automatically start the face auth on lock screen.

For now, this is a hack. We hijack the existing support for fingerprint/smartcard modules, routing the request to yahallo instead. I mean, do you _really_ need all 4 of (text,fprint,smartcard,face) methods available simultaneously? Probably not.

The main steps:
1. Open up `/etc/pam.d/kde-smartcard` or `/etc/pam.d/kde-fingerprint` in your editor (with write access)
    * Note: This support for parallel-auth is pretty recent in KDE ([landed Oct 2023](https://github.com/KDE/kscreenlocker/commit/adfae58490b4b2307221fa4e45465948b749937b)) so you might need to upgrade your DE if running an older version.
    * Note: If the file is not present and you _are_ running Plasma 6, maybe your distro has skipped bundling the extra pam conf files. You can find the full contents for Redhat (and adjacent) OSes from [here](https://invent.kde.org/plasma/kscreenlocker/-/merge_requests/163) and create it yourself.
2. Replace any calls to `fprintd.so` (if editing `kde-fingerprint`) or `pam_pkcs11.so` (if editing `kde-smartcard`) with `pam_yahallo.so`
3. ???
4. Profit

Note: I haven't tested it, but the same approach likely works on recent versions of GNOME too. Please create an issue if you confirm this.

## TODO

Running list of features I'd like to implement, ordered by (approximate) priority:
* [ ] Read config from file (`/etc/`)
* [ ] Also support password input in parallel for PAM module
* [ ] DBus Method to reload known faces in the daemon
* [ ] Write an install script (?)
* [ ] Allow changing the config path via CLI
* [ ] `--replace` support for `yahallod`
* [ ] Allow using session bus in `yahallod` (for testing)
* [ ] Benchmark and reduce latency
* [ ] Daemon should watch the known faces file for changes and autoreload
* [ ] Look into other image resizing methods (Linear, Cubic, etc.)
