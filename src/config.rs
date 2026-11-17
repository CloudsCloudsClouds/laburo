use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub typst: String,
}

impl ProjectConfig {
    pub fn resolve(project: Option<PathBuf>) -> Result<Self> {
        let root = match project {
            Some(p) => p,
            None => find_project_root()?,
        };
        let root = root
            .canonicalize()
            .with_context(|| format!("resolving project root {}", root.display()))?;
        let data_dir = root.join(".laburo");
        let db_path = data_dir.join("laburo.sqlite3");
        let typst = std::env::var("LABURO_TYPST").unwrap_or_else(|_| "typst".to_string());
        Ok(Self {
            root,
            data_dir,
            db_path,
            typst,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(self.data_dir.join("evidence"))?;
        std::fs::create_dir_all(self.data_dir.join("reports"))?;
        std::fs::create_dir_all(self.data_dir.join("tmp"))?;
        Ok(())
    }
}

fn find_project_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".laburo").is_dir()
            || dir.join("Gemfile").is_file()
            || dir.join("Cargo.toml").is_file()
        {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("could not find a project root; pass --project");
        }
    }
}

pub fn init_project(root: &Path) -> Result<ProjectConfig> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving project root {}", root.display()))?;
    let cfg = ProjectConfig {
        root: root.clone(),
        data_dir: root.join(".laburo"),
        db_path: root.join(".laburo").join("laburo.sqlite3"),
        typst: std::env::var("LABURO_TYPST").unwrap_or_else(|_| "typst".to_string()),
    };
    cfg.ensure_dirs()?;
    Ok(cfg)
}
