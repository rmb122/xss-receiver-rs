use std::path::{Path, PathBuf};

use rquickjs::{
    Ctx, Error, Module, Result,
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
};

pub struct StorageModuleLoader {
    root: PathBuf,
}

impl StorageModuleLoader {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        Ok(Self {
            root: std::fs::canonicalize(root)?,
        })
    }
}

impl Resolver for StorageModuleLoader {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        let path = if name.starts_with("./") || name.starts_with("../") {
            Path::new(base).parent().unwrap_or(&self.root).join(name)
        } else {
            self.root.join(name)
        };
        let canonical = std::fs::canonicalize(&path).map_err(|error| {
            Error::new_resolving_message(base, name, format!("could not open file: {error}"))
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(Error::new_resolving_message(
                base,
                name,
                "module is outside the module root",
            ));
        }
        canonical
            .into_os_string()
            .into_string()
            .map_err(|_| Error::new_resolving_message(base, name, "module path must be UTF-8"))
    }
}

impl Loader for StorageModuleLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let path = std::fs::canonicalize(name).map_err(|error| {
            Error::new_loading_message(name, format!("could not open file: {error}"))
        })?;
        if !path.starts_with(&self.root) {
            return Err(Error::new_loading_message(
                name,
                "module is outside the module root",
            ));
        }
        let source = std::fs::read_to_string(path).map_err(|error| {
            Error::new_loading_message(name, format!("could not parse module: {error}"))
        })?;
        Module::declare(ctx.clone(), name, source).map_err(|error| {
            let error = rquickjs::CaughtError::from_error(ctx, error);
            Error::new_loading_message(name, format!("could not parse module: {error}"))
        })
    }
}
