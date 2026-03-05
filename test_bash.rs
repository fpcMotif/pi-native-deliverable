use std::process::Command;

fn main() {
    let mut child = Command::new("sh");
    child.arg("-c").arg("echo hello");
    let output = child.output().unwrap();
    println!("out: {:?}", String::from_utf8_lossy(&output.stdout));
}
