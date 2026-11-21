use std::process::Command;

const PIN: &str = include_str!("../rune-pin.toml");

fn field(name: &str) -> &'static str {
    let prefix = format!("{name} = \"");
    PIN.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("missing {name} in rune-pin.toml"))
}

fn main() {
    let revision = field("revision");
    let output = Command::new("git")
        .args(["-C", "../rune", "rev-parse", "HEAD"])
        .output()
        .unwrap_or_else(|error| panic!("cannot verify the Rune checkout: {error}"));
    if !output.status.success() {
        panic!("../rune is missing or is not a git repository; run ./scripts/setup-rune.sh from the repository root");
    }
    let actual = String::from_utf8(output.stdout)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if actual != revision {
        panic!("Rune checkout is {actual}, expected {revision}; run ./scripts/setup-rune.sh from the repository root");
    }
    println!("cargo:rerun-if-changed=../rune-pin.toml");
    println!("cargo:rerun-if-changed=../rune/.git/HEAD");
}
