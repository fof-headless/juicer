//! Project = a folder on disk that holds everything for one demo.
//!
//! ~/Movies/Juicer/<name>/
//!   scene.json     ← the full scene (elements, keyframes, camera, render)
//!   assets/        ← captured HTML PNGs, imported images
//!   renders/       ← render_frame + render_animation output
//!
//! This is what makes Juicer findable: nothing lands in /tmp anymore. The
//! active project is saved after every mutation so a restart restores state.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::scene::Scene;

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
}

impl Project {
    /// Root folder that holds all projects: ~/Movies/Juicer
    pub fn projects_root() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Path::new(&home).join("Movies").join("Juicer")
    }

    fn sanitize(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
            .collect();
        let trimmed = cleaned.trim();
        if trimmed.is_empty() { "Untitled".to_string() } else { trimmed.to_string() }
    }

    /// Create (or adopt) a project folder by name under the projects root.
    pub fn create(name: &str) -> Result<Self> {
        let name = Self::sanitize(name);
        let root = Self::projects_root().join(&name);
        let p = Project { root, name };
        p.ensure_dirs()?;
        Ok(p)
    }

    /// Open an existing project folder by absolute path.
    pub fn open(path: &str) -> Result<(Self, Option<Scene>)> {
        let root = PathBuf::from(path);
        if !root.exists() {
            anyhow::bail!("project folder does not exist: {path}");
        }
        let name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".into());
        let p = Project { root, name };
        p.ensure_dirs()?;
        let scene = p.load_scene()?;
        Ok((p, scene))
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.assets_dir()).context("creating assets/")?;
        std::fs::create_dir_all(self.renders_dir()).context("creating renders/")?;
        Ok(())
    }

    pub fn assets_dir(&self) -> PathBuf { self.root.join("assets") }
    pub fn renders_dir(&self) -> PathBuf { self.root.join("renders") }
    pub fn scene_path(&self) -> PathBuf { self.root.join("scene.json") }

    /// Write the scene to scene.json.
    pub fn save_scene(&self, scene: &Scene) -> Result<()> {
        let json = serde_json::to_string_pretty(scene)?;
        std::fs::write(self.scene_path(), json).context("writing scene.json")?;
        Ok(())
    }

    /// Load scene.json if present.
    pub fn load_scene(&self) -> Result<Option<Scene>> {
        let path = self.scene_path();
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path).context("reading scene.json")?;
        let scene: Scene = serde_json::from_str(&data).context("parsing scene.json")?;
        Ok(Some(scene))
    }

    /// Copy an external file into assets/ and return the new absolute path.
    /// If the file is already inside assets/, returns it unchanged.
    pub fn import_asset(&self, src: &str) -> Result<String> {
        let src_path = Path::new(src);
        let assets = self.assets_dir();
        if src_path.starts_with(&assets) {
            return Ok(src.to_string());
        }
        let file_name = src_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "asset".into());
        let dest = assets.join(&file_name);
        std::fs::copy(src_path, &dest)
            .with_context(|| format!("copying {src} into assets/"))?;
        Ok(dest.to_string_lossy().to_string())
    }

    /// A fresh path inside assets/ for a given base name + extension.
    pub fn asset_path(&self, base: &str, ext: &str) -> String {
        let safe = Self::sanitize(base).replace(' ', "_");
        self.assets_dir().join(format!("{safe}.{ext}")).to_string_lossy().to_string()
    }

    /// A path inside renders/ for a given base name + extension.
    pub fn render_path(&self, base: &str, ext: &str) -> String {
        let safe = Self::sanitize(base).replace(' ', "_");
        self.renders_dir().join(format!("{safe}.{ext}")).to_string_lossy().to_string()
    }
}
