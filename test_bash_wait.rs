use std::process::Command;

fn main() {
    let mut child = Command::new("sh");
    child.arg("-c").arg("echo hello");
    let mut child = child.spawn().unwrap();
    while child.try_wait().unwrap().is_none() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    println!("out: {:?}", String::from_utf8_lossy(&output.stdout));
}
