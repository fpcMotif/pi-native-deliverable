use std::process::{Command, Stdio};
fn main() {
    let mut command = Command::new("sh");
    command.arg("-lc").arg("echo hi");
    // command.stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let out = child.wait_with_output().unwrap();
    println!("out: {:?}", out);
}
