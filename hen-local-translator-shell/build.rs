fn main() {
    println!("cargo:rerun-if-env-changed=HEN_LOCAL_SUPABASE_URL");
    println!("cargo:rerun-if-env-changed=HEN_LOCAL_SUPABASE_ANON_KEY");
    println!("cargo:rerun-if-env-changed=HEN_LOCAL_AUTH_PROVIDER");
    println!("cargo:rerun-if-env-changed=HEN_LOCAL_ACCOUNT_URL");
    tauri_build::build()
}
