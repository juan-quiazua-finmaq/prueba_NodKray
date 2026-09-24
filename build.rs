fn main() {
    println!("cargo:rerun-if-env-changed=NODKRAY_REPO");
    if let Ok(repo) = std::env::var("NODKRAY_REPO") {
        if !repo.is_empty() {
            println!("cargo:rustc-env=NODKRAY_REPO={repo}");
        }
    }
}
