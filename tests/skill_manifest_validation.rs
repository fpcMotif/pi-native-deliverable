use pi_ext::discover_skills;
use std::fs;

#[test]
fn skill_manifest_requires_description() {
    let tmp = tempfile::tempdir().expect("tmp");
    let skills_dir = tmp.path().join(".pi/skills");
    fs::create_dir_all(&skills_dir).expect("skills dir");

    fs::write(
        skills_dir.join("broken.md"),
        "---\nname: broken\n---\n# Broken",
    )
    .expect("write");

    let result = discover_skills(tmp.path());
    assert!(result.loaded.is_empty());
    assert!(result
        .warnings
        .iter()
        .any(|w| w.message.contains("missing required description")));
}

#[test]
fn skill_manifest_honors_disable_controls() {
    let tmp = tempfile::tempdir().expect("tmp");
    let skills_dir = tmp.path().join(".pi/skills");
    fs::create_dir_all(&skills_dir).expect("skills dir");

    fs::write(
        skills_dir.join("enabled.md"),
        "---\nname: enabled\ndescription: usable\n---\n# Enabled",
    )
    .expect("enabled");
    fs::write(
        skills_dir.join("disabled.md"),
        "---\nname: disabled\ndescription: hidden\ndisabled: true\n---\n# Disabled",
    )
    .expect("disabled");

    let result = discover_skills(tmp.path());
    assert_eq!(result.loaded.len(), 1);
    assert_eq!(result.loaded[0].name, "enabled");
    assert!(result
        .warnings
        .iter()
        .any(|w| w.message.contains("is disabled")));
}
