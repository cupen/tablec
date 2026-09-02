use std::path::Path;

fn main() {
    // The webui frontend (tablec-webui/webui) is a pnpm + Vite project whose
    // build output `dist/` is embedded via include_dir!. dist/ is NOT
    // committed: dependencies come from the npm registry via pnpm-lock.yaml.
    // Keep `cargo build` working without node by creating an empty
    // placeholder when dist hasn't been generated yet.
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("webui/dist");
    if !dist.is_dir() {
        std::fs::create_dir_all(&dist)
            .unwrap_or_else(|e| panic!("failed to create webui/dist placeholder: {e}"));
        println!(
            "cargo:warning=tablec-webui: webui/dist is missing — created an empty placeholder. \
             Run `pnpm build` in tablec-webui/webui to embed the frontend."
        );
    }
    println!("cargo:rerun-if-changed=webui/dist/index.html");
    println!("cargo:rerun-if-changed=webui/dist/assets");
}
