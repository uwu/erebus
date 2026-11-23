use std::{path::PathBuf, process::ExitCode};

use goblin::Object;

const HOOKED: &[&str] = &["D3D11CreateDevice", "D3D11CreateDeviceAndSwapChain"];

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run --example mkdefs -- <path-to-d3d11.dll>");
        return ExitCode::FAILURE;
    };

    let bytes = std::fs::read(&path).expect("failed to read the d3d11.dll");
    let Object::PE(pe) = Object::parse(&bytes).expect("failed to parse the d3d11.dll") else {
        panic!("{} is not a PE file", path.display());
    };

    let mut forwarders: Vec<&str> = pe
        .exports
        .iter()
        .filter_map(|export| export.name)
        .filter(|name| !HOOKED.contains(name))
        .collect();
    forwarders.sort_unstable();
    forwarders.dedup();

    println!("; Generated from d3d11.dll's export table. Regenerate with:");
    println!(";   cargo run --example mkdefs -- <path-to-d3d11.dll> > d3d11_forwarders.def");
    println!("EXPORTS");
    for name in forwarders {
        println!("    {name}=d3d11_orig.{name}");
    }

    ExitCode::SUCCESS
}
