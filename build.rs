fn main() -> shadow_rs::SdResult<()> {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.lock");
    shadow_rs::new()
}
