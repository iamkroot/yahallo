# Yahallo

Yet Another Hallo - a Face Recognition integration for Linux.

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
