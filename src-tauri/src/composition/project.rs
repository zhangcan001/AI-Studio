use crate::app_state::ProjectServices;
use crate::application::{
    comfy_preflight_service::ComfyPreflightService,
    comfy_service::ComfyService,
    ports::{
        Clock, ProjectBackupRepository, ProjectCommandCenterRepository, ProjectDirectoryStore,
        ProjectManifestRepository, ProjectRepository,
    },
    production_audit_service::ProductionAuditService,
    project_backup_service::ProjectBackupService,
    project_command_center_service::ProjectCommandCenterService,
    project_manifest_service::ProjectManifestService,
    project_service::ProjectService,
    project_workflow_binding_service::ProjectWorkflowBindingService,
};
use std::{path::PathBuf, sync::Arc};

/// Domain-specific ports and already-created cross-context authorities, never a second pool/queue.
pub struct ProjectContextDependencies {
    pub projects: Arc<dyn ProjectRepository>,
    pub directories: Arc<dyn ProjectDirectoryStore>,
    pub clock: Arc<dyn Clock>,
    pub overview_query: Arc<dyn ProjectCommandCenterRepository>,
    pub manifest_query: Arc<dyn ProjectManifestRepository>,
    pub backup: Arc<dyn ProjectBackupRepository>,
    pub audit: Arc<ProductionAuditService>,
    pub workflow_binding: Arc<ProjectWorkflowBindingService>,
    pub comfy_cache: Option<(Arc<ComfyService>, Arc<ComfyPreflightService>)>,
    pub projects_dir: PathBuf,
    pub cache_dir: PathBuf,
}
impl ProjectServices {
    pub fn build(deps: ProjectContextDependencies) -> Self {
        let mut center = ProjectCommandCenterService::new(deps.overview_query, deps.audit);
        if let Some((comfy, preflight)) = deps.comfy_cache {
            center = center.with_comfy_cache_services(comfy, preflight);
        }
        Self {
            command_center: Arc::new(center),
            project: Arc::new(ProjectService::new(
                deps.projects,
                deps.directories,
                deps.clock,
            )),
            backup: Arc::new(ProjectBackupService::new(
                deps.backup,
                deps.projects_dir,
                deps.cache_dir,
            )),
            manifest: Arc::new(ProjectManifestService::new(deps.manifest_query)),
            workflow_binding: deps.workflow_binding,
        }
    }
}

#[cfg(test)]
mod phase8_tests {
    use super::*;
    use crate::infrastructure::{
        database::*, filesystem::FileSystemProjectDirectoryStore, time::SystemClock,
    };
    #[tokio::test]
    async fn phase8_target5_project_context_reuses_real_sqlite_query_and_binding_ports() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(dir.path().join("projects"))
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join("cache"))
            .await
            .unwrap();
        let pool = initialize(&dir.path().join("project.db")).await.unwrap();
        let projects = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let clock = Arc::new(SystemClock);
        let binding = Arc::new(ProjectWorkflowBindingService::new(
            Arc::new(SqliteProjectWorkflowBindingRepository::new(pool.clone())),
            projects.clone(),
            Arc::new(SqliteWorkflowRuntimeRepository::new(pool.clone())),
            Arc::new(SqliteWorkflowRuntimeStateRepository::new(pool.clone())),
            clock.clone(),
        ));
        let audit = Arc::new(ProductionAuditService::new(Arc::new(
            SqliteProductionAuditRepository::new(pool.clone()),
        )));
        let context = ProjectServices::build(ProjectContextDependencies {
            projects: projects.clone(),
            directories: Arc::new(FileSystemProjectDirectoryStore::new(
                dir.path().join("projects"),
            )),
            clock,
            overview_query: Arc::new(SqliteProjectCommandCenterRepository::new(pool.clone())),
            manifest_query: Arc::new(SqliteProjectManifestRepository::new(pool.clone())),
            backup: Arc::new(SqliteProjectBackupRepository::new(pool.clone())),
            audit,
            workflow_binding: binding.clone(),
            comfy_cache: None,
            projects_dir: dir.path().join("projects"),
            cache_dir: dir.path().join("cache"),
        });
        assert!(Arc::ptr_eq(&binding, &context.workflow_binding));
        let project = context
            .project
            .create("Phase8 isolated context", None)
            .await
            .unwrap();
        assert!(context
            .project
            .list()
            .await
            .unwrap()
            .iter()
            .any(|p| p.id == project.id));
        let view = context.command_center.get(&project.id).await.unwrap();
        assert_eq!(view.project.id, project.id);
        assert_eq!(view.audit.tasks, 0);
        let file = dir.path().join("manifest.json");
        context
            .manifest
            .export(&project.id, file.clone())
            .await
            .unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(file).await.unwrap()).unwrap();
        assert_eq!(manifest["project"]["id"], project.id);
        let bindings = context.workflow_binding.get(&project.id).await.unwrap();
        assert!(bindings.image_default.is_none());
        assert!(bindings.video_default.is_none());
        assert!(bindings.video_mode_overrides.is_empty());
        let backup = dir.path().join("backup.aiarchive");
        context
            .backup
            .export(&project.id, backup.clone())
            .await
            .unwrap();
        assert!(backup.is_file());
        pool.close().await;
    }
}
