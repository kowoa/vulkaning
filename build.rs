extern crate shaderc;

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
};

use color_eyre::eyre::{eyre, OptionExt, Result};
use shaderc::CompilationArtifact;

const COMBINED_SHADER_EXT: &str = "combined";
const COMP_SHADER_EXT: &str = "comp";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=shaders/*");

    color_eyre::install()?;

    let shaderbuild_dirpath = std::env::var("SHADER_BUILD_DIR")
        .unwrap_or_else(|_| "./shaderbuild".to_string());
    fs::create_dir_all(shaderbuild_dirpath.clone())?;

    let compiler = shaderc::Compiler::new()
        .ok_or_eyre("Failed to create shaderc compiler")?;
    let options = shaderc::CompileOptions::new()
        .ok_or_eyre("Failed to create shaderc options")?;

    let shaders_dirpath = Path::new("./shaders");
    for entry in fs::read_dir(shaders_dirpath)? {
        let filepath = entry?.path();
        if filepath.is_file() {
            let ext = filepath.extension();
            if ext.is_none() {
                continue;
            }
            let ext = ext.unwrap();
            let filestem = filepath.file_stem().unwrap().to_str().unwrap();
            let filename = filepath.file_name().unwrap().to_str().unwrap();

            if ext == COMBINED_SHADER_EXT {
                let (vert_glsl, frag_glsl) =
                    parse_combined_shaderfile(&filepath)?;
                let vert_spirv = compile_shader(
                    &vert_glsl,
                    shaderc::ShaderKind::Vertex,
                    &compiler,
                    &options,
                    filename,
                )?;
                let frag_spirv = compile_shader(
                    &frag_glsl,
                    shaderc::ShaderKind::Fragment,
                    &compiler,
                    &options,
                    filename,
                )?;

                let vert_spv_filepath =
                    format!("{}/{}-vert.spv", shaderbuild_dirpath, filestem);
                let mut vert_spv_file = File::create(vert_spv_filepath)?;
                vert_spv_file.write_all(vert_spirv.as_binary_u8())?;

                let frag_spv_filepath =
                    format!("{}/{}-frag.spv", shaderbuild_dirpath, filestem);
                let mut frag_spv_file = File::create(frag_spv_filepath)?;
                frag_spv_file.write_all(frag_spirv.as_binary_u8())?;
            } else if ext == COMP_SHADER_EXT {
                let mut file = File::open(&filepath)?;
                let mut comp_glsl = String::new();
                file.read_to_string(&mut comp_glsl)?;

                let comp_spirv = compile_shader(
                    &comp_glsl,
                    shaderc::ShaderKind::Compute,
                    &compiler,
                    &options,
                    filename,
                )?;

                let mut comp_spv_filepath = PathBuf::from(&shaderbuild_dirpath);
                comp_spv_filepath.push(format!("{}-comp.spv", filestem));
                let mut comp_spv_file = File::create(comp_spv_filepath)?;
                comp_spv_file.write_all(comp_spirv.as_binary_u8())?;
            }
        }
    }

    Ok(())
}

fn parse_combined_shaderfile(filepath: &PathBuf) -> Result<(String, String)> {
    let file = File::open(filepath)?;
    let reader = BufReader::new(file);
    let lines = reader.lines();

    let mut vert_glsl = String::new();
    let mut frag_glsl = String::new();
    let mut shadertype = None;

    for line in lines {
        let line = line?;

        if line.trim_start().starts_with("#shader") {
            if let Some(stype) = line.split_whitespace().nth(1) {
                shadertype = match stype {
                    "vertex" => Some(shaderc::ShaderKind::Vertex),
                    "fragment" => Some(shaderc::ShaderKind::Fragment),
                    _ => None,
                };
                continue;
            }

            return Err(eyre!("Invalid #shader type specifier: {}", line));
        }

        if let Some(stype) = &shadertype {
            let str_buf = match stype {
                shaderc::ShaderKind::Vertex => Ok(&mut vert_glsl),
                shaderc::ShaderKind::Fragment => Ok(&mut frag_glsl),
                _ => Err(eyre!("Invalid shadertype")),
            }?;
            str_buf.push_str(&line);
            str_buf.push('\n');
        }
    }

    if vert_glsl.is_empty() {
        Err(eyre!("No vertex #shader type specifier found"))
    } else if frag_glsl.is_empty() {
        Err(eyre!("No fragment #shader type specifier found"))
    } else {
        Ok((vert_glsl, frag_glsl))
    }
}

fn compile_shaders(
    vert_glsl: &str,
    frag_glsl: &str,
    compiler: &shaderc::Compiler,
    options: &shaderc::CompileOptions,
    filename: &str,
) -> Result<(CompilationArtifact, CompilationArtifact)> {
    let vert_spirv = compiler.compile_into_spirv(
        vert_glsl,
        shaderc::ShaderKind::Vertex,
        filename,
        "main",
        Some(options),
    )?;

    let frag_spirv = compiler.compile_into_spirv(
        frag_glsl,
        shaderc::ShaderKind::Fragment,
        filename,
        "main",
        Some(options),
    )?;

    Ok((vert_spirv, frag_spirv))
}

fn compile_shader(
    glsl: &str,
    kind: shaderc::ShaderKind,
    compiler: &shaderc::Compiler,
    options: &shaderc::CompileOptions,
    filename: &str,
) -> Result<CompilationArtifact> {
    Ok(compiler.compile_into_spirv(
        glsl,
        kind,
        filename,
        "main",
        Some(options),
    )?)
}
