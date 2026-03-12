with open("crates/pi-session/src/bin_benchmark.rs", "r") as f:
    c = f.read()
c = c.replace(".unwrap()", ".expect(\"benchmark error\")")
with open("crates/pi-session/src/bin_benchmark.rs", "w") as f:
    f.write(c)

with open("crates/pi-session/benches/session_load.rs", "r") as f:
    c = f.read()
c = c.replace(".unwrap()", ".expect(\"benchmark error\")")
with open("crates/pi-session/benches/session_load.rs", "w") as f:
    f.write(c)
