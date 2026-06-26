use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let include_dirs = resolve_lsl();

    #[cfg(feature = "bindgen")]
    {
        let header = find_header(&include_dirs, "lsl_c.h").unwrap_or_else(|| {
            panic!(
                "lsl_c.h not found in any resolved include dir: {include_dirs:?}.\n\
                 If liblsl is installed in a non-standard location, make sure pkg-config \
                 can find its .pc file (set PKG_CONFIG_PATH), or on Windows that vcpkg \
                 knows about the 'lsl' port."
            )
        });
        run_bindgen(&header, &include_dirs);
    }

    let _ = include_dirs;
}

#[cfg(all(feature = "system", feature = "bundled"))]
fn resolve_lsl() -> Vec<PathBuf> {
    try_system().unwrap_or_else(build_liblsl)
}

#[cfg(all(feature = "system", not(feature = "bundled")))]
fn resolve_lsl() -> Vec<PathBuf> {
    try_system().unwrap_or_else(|| {
        panic!(
            "liblsl not found via pkg-config (or vcpkg on Windows). Install liblsl's \
             development package, or enable the 'bundled' feature to build it from source."
        )
    })
}

#[cfg(all(not(feature = "system"), feature = "bundled"))]
fn resolve_lsl() -> Vec<PathBuf> {
    build_liblsl()
}

#[cfg(not(any(feature = "system", feature = "bundled")))]
fn resolve_lsl() -> Vec<PathBuf> {
    compile_error!("at least one of the 'system' or 'bundled' features must be enabled")
}

#[cfg(feature = "system")]
fn try_system() -> Option<Vec<PathBuf>> {
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    if let Ok(lib) = pkg_config::Config::new().probe("lsl") {
        return Some(lib.include_paths);
    }

    #[cfg(windows)]
    if let Ok(lib) = vcpkg::find_package("lsl") {
        return Some(lib.include_paths);
    }

    None
}

fn find_header(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.exists())
}

#[cfg(feature = "bundled")]
fn build_liblsl() -> Vec<PathBuf> {
    let target = env::var("TARGET").unwrap();

    let mut cfg = cmake::Config::new("liblsl");
    cfg.define("LSL_NO_FANCY_LIBNAME", "ON")
        .define("LSL_BUILD_STATIC", "ON");
    if target.contains("msvc") {
        let cxx_args = " /nologo /EHsc /MD /GR";
        cfg.define("WIN32", "1")
            .define("_WINDOWS", "1")
            .define("CMAKE_C_FLAGS", cxx_args)
            .define("CMAKE_CXX_FLAGS", cxx_args)
            .define("CMAKE_C_FLAGS_DEBUG", cxx_args)
            .define("CMAKE_CXX_FLAGS_DEBUG", cxx_args)
            .define("CMAKE_C_FLAGS_RELEASE", cxx_args)
            .define("CMAKE_CXX_FLAGS_RELEASE", cxx_args);
    }
    let install_dir = cfg.build();

    let libdir = install_dir.join("lib");
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rustc-link-lib=static=lsl");

    if target.contains("linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if target.contains("windows") {
        println!("cargo:rustc-link-lib=dylib=bcrypt");
    } else {
        println!("cargo:rustc-link-lib=dylib=c++");
    }

    vec![install_dir.join("include")]
}

#[cfg(feature = "bindgen")]
fn run_bindgen(header: &Path, include_dirs: &[PathBuf]) {
    let mut builder = bindgen::Builder::default()
        .header(header.to_str().expect("header path is not valid UTF-8"))
        .allowlist_function("^lsl_.*")
        .allowlist_var("^lsl_.*")
        .allowlist_type("^lsl_.*");

    for dir in include_dirs {
        builder = builder.clang_arg(format!("-I{}", dir.display()));
    }

    let bindings = builder.generate().expect("failed to generate liblsl bindings");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("generated.rs");
    bindings
        .write_to_file(&out)
        .expect("failed to write liblsl bindings");
}
