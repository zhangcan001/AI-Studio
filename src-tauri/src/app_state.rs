use crate::application::comfy_service::ComfyService;
use crate::infrastructure::filesystem::AppDataDirs;
use std::sync::Arc;

pub struct AppState {
    pub data_dirs: AppDataDirs,
    pub comfy_service: Arc<ComfyService>,
}

impl AppState {
    pub fn new(data_dirs: AppDataDirs, comfy_service: Arc<ComfyService>) -> Self {
        Self {
            data_dirs,
            comfy_service,
        }
    }
}
