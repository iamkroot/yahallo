# Howdy in Rust


## Architecture

* Input thread: Init camera, Read frames at 20fps, do resize, histogram (darkness)
  --- Queue 1: (frame matrix) n=10 ---
* FD Thread: Init Face Detector; do face detection on threads
  --- Queue 2: (rectangles,frame matrix) n=10 ---
* Main Thread: Init landmark predictor, encoder; do both


* 
```rs
struct Cam {
  destroying: Option<JoinHandle>
}
impl Cam {
  fn start_stream(&mut self) {
    camera = new camera
    call camera.start
    return Stream{cam, self}
  }
}
``` 
