// module.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::metal::parser::{Program, Decl, Parser};

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub program: Program,
    pub exports: HashMap<String, ExportKind>,
}

#[derive(Debug, Clone)]
pub enum ExportKind {
    Struct(Vec<crate::metal::parser::StructField>),
    Const(crate::metal::parser::Type),
}

pub struct ModuleLoader {
    pub loaded: HashMap<String, Module>,  // pub 추가
    search_paths: Vec<PathBuf>,
}

impl ModuleLoader {
    pub fn new(base_path: &Path) -> Self {
        Self {
            loaded: HashMap::new(),
            search_paths: vec![base_path.to_path_buf()],
        }
    }

    pub fn load(&mut self, name: &str) -> Result<&Module, String> {
        if self.loaded.contains_key(name) {
            return Ok(self.loaded.get(name).unwrap());
        }

        let filename = format!("{}.slvt", name);
        let path = self.search_paths.iter()
            .map(|p| p.join(&filename))
            .find(|p| p.exists())
            .ok_or_else(|| format!("module '{}' not found", name))?;

        let src = std::fs::read_to_string(&path)
            .map_err(|e| format!("error reading module '{}': {}", name, e))?;
        let mut parser = Parser::new(&src);
        let program = parser.parse_program()
            .map_err(|e| format!("parse error in module '{}': {}", name, e))?;

        let mut exports = HashMap::new();
        for decl in &program.decls {
            match decl {
                Decl::Struct { name: sname, fields, is_pub, .. } if *is_pub => {
                    exports.insert(sname.clone(), ExportKind::Struct(fields.clone()));
                }
                Decl::Const { name: cname, ty, is_pub, .. } if *is_pub => {
                    exports.insert(cname.clone(), ExportKind::Const(ty.clone()));
                }
                _ => {}
            }
        }

        let module = Module {
            name: name.to_string(),
            program,
            exports,
        };

        self.loaded.insert(name.to_string(), module);
        Ok(self.loaded.get(name).unwrap())
    }

    pub fn get(&self, name: &str) -> Option<&Module> {
        self.loaded.get(name)
    }
}