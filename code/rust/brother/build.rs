// brother/build.rs
use std::{env, fs, path::{Path, PathBuf}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);

    // ──► walk up three levels: brother/ → rust/ → code/ → <repo root>
    let repo_root = crate_dir
        .ancestors().nth(3)   // 0 = self, 1 = rust, 2 = code, 3 = root
        .expect("directory depth assumption broken")
        .to_path_buf();

    let proto_dir      = repo_root.join("contracts/proto");
    let brother_proto  = proto_dir.join("brother/brother.proto");

    // where we want the generated .rs to live:
    let gen_dir = crate_dir.join("src/generated");
    fs::create_dir_all(&gen_dir)?;

    tonic_build::configure()
        .out_dir(&gen_dir)
        .build_client(true)
        .build_server(true)
        .compile_protos(&[&brother_proto], &[&proto_dir])?;

    // Make Cargo re-run this script if the proto changes
    println!("cargo:rerun-if-changed={}", brother_proto.display());
    Ok(())
}
