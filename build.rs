extern crate shaderc;

use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use shaderc::CompilationArtifact;

const SHADER_EXT: &'static str = "glsl";

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=shaders/*");

    //let out_dir = std::env::var("OUT_DIR")?;
    let shaderbuild_dirpath = "./shaderbuild";
    fs::create_dir_all(shaderbuild_dirpath)?;

    let compiler = shaderc::Compiler::new()
        .ok_or(anyhow!("Failed to create shaderc compiler"))?;
    let options = shaderc::CompileOptions::new()
        .ok_or(anyhow!("Failed to create shaderc options"))?;

    let shaders_dirpath = Path::new("./shaders");
    for entry in fs::read_dir(shaders_dirpath)? {
        let filepath = entry?.path();
        if filepath.is_file()
            && filepath.extension() == Some(SHADER_EXT.as_ref())
        {
            let filestem = filepath.file_stem().unwrap().to_str().unwrap();
            let filename = filepath.file_name().unwrap().to_str().unwrap();

            let (vert_glsl, frag_glsl) = parse_shaderfile(&filepath)?;
            let (vert_spirv, frag_spirv) = compile_shaders(
                &vert_glsl, &frag_glsl, &compiler, &options, filename,
            )?;

            let vert_spv_filepath =
                format!("{}/{}-vert.spv", shaderbuild_dirpath, filestem);
            let mut vert_spv_file = File::create(vert_spv_filepath)?;
            vert_spv_file.write_all(vert_spirv.as_binary_u8())?;

            let frag_spv_filepath =
                format!("{}/{}-frag.spv", shaderbuild_dirpath, filestem);
            let mut frag_spv_file = File::create(frag_spv_filepath)?;
            frag_spv_file.write_all(frag_spirv.as_binary_u8())?;
        }
    }

    Ok(())
}

fn parse_shaderfile(filepath: &PathBuf) -> anyhow::Result<(String, String)> {
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

            return Err(anyhow!("Invalid #shader type specifier: {}", line));
        }

        if let Some(stype) = &shadertype {
            let str_buf = match stype {
                shaderc::ShaderKind::Vertex => Ok(&mut vert_glsl),
                shaderc::ShaderKind::Fragment => Ok(&mut frag_glsl),
                _ => Err(anyhow!("Invalid shadertype")),
            }?;
            str_buf.push_str(&line);
            str_buf.push('\n');
        }
    }

    if vert_glsl.is_empty() {
        Err(anyhow!("No vertex #shader type specifier found"))
    } else if frag_glsl.is_empty() {
        Err(anyhow!("No fragment #shader type specifier found"))
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
) -> anyhow::Result<(CompilationArtifact, CompilationArtifact)> {
    let vert_spirv = compiler.compile_into_spirv(
        &vert_glsl,
        shaderc::ShaderKind::Vertex,
        &filename,
        "main",
        Some(&options),
    )?;

    let frag_spirv = compiler.compile_into_spirv(
        &frag_glsl,
        shaderc::ShaderKind::Fragment,
        &filename,
        "main",
        Some(&options),
    )?;

    Ok((vert_spirv, frag_spirv))
}
