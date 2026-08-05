use crate::application::comfy_service::ComfyService;
use crate::application::generation_service::GenerationService;
use crate::infrastructure::filesystem::AppDataDirs;
use std::sync::Arc;

pub struct AppState {
    pub data_dirs: AppDataDirs,
    pub comfy_service: Arc<ComfyService>,
    pub generation_service: Arc<GenerationService>,
}

impl AppState {
    pub fn new(
        data_dirs: AppDataDirs,
        comfy_service: Arc<ComfyService>,
        generation_service: Arc<GenerationService>,
    ) -> Self {
        Self {
            data_dirs,
            comfy_service,
            generation_service,
        }
    }
}
