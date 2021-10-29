use shaderc::{CompileOptions, Compiler, OptimizationLevel, ShaderKind, SourceLanguage::GLSL};

const SHADERS_PATH: &str = "shaders/";

#[allow(clippy::enum_variant_names)]
enum Error {
    DirNotFound(std::io::Error),
    FileNotFound(std::path::PathBuf, std::io::Error),
    WriteError(std::path::PathBuf, std::io::Error),
    NonUtf8FileName(std::ffi::OsString),
    CompilerCreate,
    CompileOptionsCreate,
    CompileError(std::path::PathBuf, shaderc::Error),
}

fn main_handled() -> Result<(), Error> {
    let mut compiler = Compiler::new().ok_or(Error::CompilerCreate)?;
    let mut compile_options = CompileOptions::new().ok_or(Error::CompileOptionsCreate)?;
    compile_options.set_optimization_level(OptimizationLevel::Performance);
    for (file, file_type) in std::fs::read_dir(SHADERS_PATH)
        .map_err(Error::DirNotFound)?
        .filter_map(|f| f.ok().and_then(|f| f.file_type().map(|t| (f, t)).ok()))
    {
        if !file_type.is_file() {
            continue;
        };
        let path = file.path();
        let (kind, lang, out_name) = match &path.file_name().and_then(std::ffi::OsStr::to_str) {
            Some(name) if name.ends_with(".vertex.glsl") => (
                ShaderKind::Vertex,
                GLSL,
                path.with_extension("spirv").file_name().unwrap().to_owned(),
            ),
            Some(name) if name.ends_with(".fragment.glsl") => (
                ShaderKind::Fragment,
                GLSL,
                path.with_extension("spirv").file_name().unwrap().to_owned(),
            ),
            _ => {
                println!(
                    "cargo:warning=build script: unexpected file \"{}\"",
                    path.display()
                );
                continue;
            }
        };
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .file_name()
            .unwrap()
            .to_str()
            .ok_or_else(|| Error::NonUtf8FileName(path.as_os_str().to_owned()))?;
        compile_options.set_source_language(lang);
        if let Ok("debug") = std::env::var("PROFILE").as_ref().map(String::as_str) {
            compile_options.set_generate_debug_info();
        }
        let source =
            std::fs::read_to_string(&path).map_err(|err| Error::FileNotFound(path.clone(), err))?;
        let artifact = compiler
            .compile_into_spirv(&source, kind, name, "entry", Some(&compile_options))
            .map_err(|err| Error::CompileError(path, err))?;
        if artifact.get_num_warnings() > 0 {
            println!(
                "cargo:warning=build script: shader compilation warning \"{}\"",
                artifact.get_warning_messages()
            );
        }
        let mut out_path: std::path::PathBuf = std::env::var("OUT_DIR")
            .expect("build script needs the OUT_DIR env var")
            .into();
        out_path.push(out_name);
        std::fs::write(&out_path, artifact.as_binary_u8())
            .map_err(|err| Error::WriteError(out_path, err))?;
    }
    Ok(())
}

fn main() -> Result<(), ()> {
    use Error::*;
    main_handled().map_err(|err| match err {
        DirNotFound(err) => {
            println!(
                "error: cannot open directory \"{}\" ({})",
                SHADERS_PATH, err
            );
        }
        FileNotFound(path, err) => {
            println!("error: cannot read file \"{}\" ({})", path.display(), err)
        }
        WriteError(path, err) => {
            println!("error: cannot write file \"{}\" ({})", path.display(), err)
        }
        NonUtf8FileName(path) => println!("non utf-8 file name \"{:#?}\"", path),
        CompilerCreate => println!("error: cannot initialize SPIR-V compiler"),
        CompileOptionsCreate => println!("error: cannot initialize SPIR-V compile options"),
        CompileError(path, err) => println!(
            "error: shader compilation error in \"{}\": {}",
            path.display(),
            err
        ),
    })
}
